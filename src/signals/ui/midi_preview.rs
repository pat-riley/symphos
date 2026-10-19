//! Input previews retain MIDI units and polarity, independently of signal shaping.
use crate::{
    help::HoverHelp,
    midi::{Binding, Message, Snapshot},
};
use egui::{self, Color32, Rect, Stroke, Vec2};
use std::collections::VecDeque;

const SECONDS: f64 = 4.0;
const SAMPLE_INTERVAL: f64 = 1.0 / 30.0;
const CAPACITY: usize = 122;
#[derive(Clone)]
struct Frame {
    time: f64,
    value: f32,
    notes: Vec<(u8, f32)>,
}
pub(super) struct History {
    binding: Binding,
    frames: VecDeque<Frame>,
    last_serial: u64,
    peaks: [f32; 128],
}
impl History {
    pub fn new(binding: &Binding, serial: u64) -> Self {
        Self {
            binding: binding.clone(),
            frames: VecDeque::new(),
            last_serial: serial,
            peaks: [0.0; 128],
        }
    }
    pub fn update(&mut self, binding: &Binding, snapshot: &Snapshot, time: f64) {
        if self.binding != *binding || self.frames.back().is_some_and(|f| time < f.time) {
            *self = Self::new(binding, snapshot.serial);
        }
        // Retain short pad hits even if note-off arrived before the next UI sample.
        for event in snapshot
            .events
            .iter()
            .filter(|e| e.serial > self.last_serial && binding.matches_event(e))
        {
            if note_message(binding.message)
                && let Some(note) = event.number
            {
                self.peaks[usize::from(note)] =
                    self.peaks[usize::from(note)].max(if binding.message == Message::NoteGate {
                        1.0
                    } else {
                        f32::from(event.value) / 127.0
                    });
            }
        }
        self.last_serial = snapshot.serial;
        if self
            .frames
            .back()
            .is_some_and(|f| time - f.time < SAMPLE_INTERVAL)
        {
            return;
        }
        let levels = snapshot.note_levels(binding);
        let notes = levels
            .iter()
            .zip(&mut self.peaks)
            .enumerate()
            .filter_map(|(number, (&level, peak))| {
                let value = level.max(*peak);
                *peak = 0.0;
                (value > 0.0).then_some((number as u8, value))
            })
            .collect();
        self.frames.push_back(Frame {
            time,
            value: snapshot.value(binding),
            notes,
        });
        while self.frames.len() > CAPACITY
            || self.frames.front().is_some_and(|f| time - f.time > SECONDS)
        {
            self.frames.pop_front();
        }
    }
}
fn note_message(message: Message) -> bool {
    matches!(
        message,
        Message::NoteVelocity | Message::NoteGate | Message::PolyPressure
    )
}
fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    format!(
        "{}{}",
        NAMES[usize::from(note) % 12],
        i16::from(note) / 12 - 1
    )
}
fn title(binding: &Binding) -> String {
    match binding.message {
        Message::ControlChange => format!("CC {}", binding.number.unwrap_or(1)),
        Message::NoteVelocity => "Velocity".into(),
        Message::NoteGate => "Note gate".into(),
        Message::PitchBend => "Pitch bend".into(),
        Message::ChannelPressure => "Aftertouch".into(),
        Message::PolyPressure => "Note pressure".into(),
    }
}
fn formatted_value(message: Message, value: f32, notes: &[f32; 128]) -> String {
    if message == Message::NoteGate {
        format!("{} held", notes.iter().filter(|&&v| v > 0.0).count())
    } else if message == Message::PitchBend {
        format!("{:+.0}", value * if value < 0.0 { 8192.0 } else { 8191.0 })
    } else {
        format!("{:.0}", value.clamp(0.0, 1.0) * 127.0)
    }
}

pub(super) fn draw(
    ui: &egui::Ui,
    rect: Rect,
    binding: &Binding,
    snapshot: &Snapshot,
    history: Option<&History>,
    time: f64,
    color: Color32,
) {
    let help = match binding.message {
        Message::PitchBend => {
            "MIDI pitch bend\nThe center line is the wheel’s resting position (0). Up is positive, down is negative, with the full 14-bit range −8192…+8191. The preview shows incoming MIDI before normalization, smoothing, mute and visual influence; In/Out meters below show the processed signal."
        }
        Message::ControlChange => {
            "MIDI controller preview\nStepped history of the selected CC, in its native 0–127 range. Values hold between messages. Configure the controller number, input and channel in the inspector or use Learn MIDI. The preview is before signal shaping; Out below shows processed influence."
        }
        Message::NoteVelocity | Message::NoteGate => {
            "MIDI note preview\nA miniature piano roll shows the matching notes over the last four seconds. Colored keys are held notes; brighter notes indicate stronger velocity (gates are on/off). Sustain is honored. MIDI note 60 is labeled C4. Short strikes remain visible in the history after release. Use the inspector to select Any note or a particular pitch."
        }
        _ => {
            "MIDI pressure preview\nPressure is shown in its native 0–127 range. Channel pressure shows a stepped control history; note pressure shows separate pitches with brighter marks for stronger aftertouch. The preview shows input before channel shaping; Out below shows processed influence."
        }
    };
    ui.interact(
        rect,
        ui.id().with("midi-input-preview"),
        egui::Sense::hover(),
    )
    .help_text(help);
    let painter = ui.painter_at(rect.shrink(1.0));
    let inner = rect.shrink(6.0);
    let value = snapshot.value(binding);
    let notes = snapshot.note_levels(binding);
    let weak = ui.visuals().weak_text_color();
    let available = snapshot.available(binding);
    let ink = if available { color } else { weak };
    painter.text(
        inner.left_top(),
        egui::Align2::LEFT_TOP,
        title(binding),
        egui::FontId::proportional(10.0),
        weak,
    );
    painter.text(
        inner.right_top(),
        egui::Align2::RIGHT_TOP,
        formatted_value(binding.message, value, &notes),
        egui::FontId::monospace(10.0),
        ink,
    );
    if inner.height() < 44.0 {
        return;
    }
    let plot = Rect::from_min_max(
        inner.min + Vec2::new(0.0, 17.0),
        inner.max - Vec2::new(0.0, 15.0),
    );
    let history = history.filter(|h| h.binding == *binding);
    if note_message(binding.message) {
        piano_roll(ui, plot, &notes, history, time, ink);
    } else {
        control_history(
            ui,
            plot,
            value,
            binding.message == Message::PitchBend,
            history,
            time,
            ink,
        );
    }
    let channel = binding
        .channel
        .map_or("All ch".into(), |c| format!("Ch {c}"));
    let detail = if !available {
        "Input unavailable".into()
    } else if note_message(binding.message) {
        binding
            .number
            .map_or("Any note".into(), |n| format!("{} · {n}", note_name(n)))
    } else if binding.message == Message::PitchBend {
        "− / 0 / +".into()
    } else {
        "0–127".into()
    };
    painter.text(
        inner.left_bottom(),
        egui::Align2::LEFT_BOTTOM,
        channel,
        egui::FontId::proportional(9.0),
        weak,
    );
    painter.text(
        inner.right_bottom(),
        egui::Align2::RIGHT_BOTTOM,
        detail,
        egui::FontId::proportional(9.0),
        weak,
    );
}
fn x(time: f64, now: f64, plot: Rect) -> f32 {
    plot.right() - ((now - time) / SECONDS).clamp(0.0, 1.0) as f32 * plot.width()
}
fn control_history(
    ui: &egui::Ui,
    plot: Rect,
    value: f32,
    bipolar: bool,
    history: Option<&History>,
    now: f64,
    color: Color32,
) {
    let painter = ui.painter_at(plot.expand(1.0));
    let y = |v: f32| {
        if bipolar {
            plot.center().y - v.clamp(-1.0, 1.0) * plot.height() * 0.5
        } else {
            plot.bottom() - v.clamp(0.0, 1.0) * plot.height()
        }
    };
    let weak = ui.visuals().weak_text_color();
    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
        painter.hline(
            plot.x_range(),
            plot.top() + fraction * plot.height(),
            Stroke::new(0.5, weak.gamma_multiply(0.12)),
        );
    }
    painter.hline(
        plot.x_range(),
        y(0.0),
        Stroke::new(
            if bipolar { 1.0 } else { 0.5 },
            weak.gamma_multiply(if bipolar { 0.45 } else { 0.2 }),
        ),
    );
    let mut points = Vec::new();
    if let Some(history) = history {
        for frame in &history.frames {
            let point = egui::pos2(x(frame.time, now, plot), y(frame.value));
            if let Some(last) = points.last().copied() {
                let last: egui::Pos2 = last;
                points.push(egui::pos2(point.x, last.y));
            }
            points.push(point);
        }
    }
    if points.is_empty() {
        points.push(egui::pos2(plot.left(), y(value)));
    } else if points.len() == 1 {
        points.insert(0, egui::pos2(plot.left(), points[0].y));
    }
    if let Some(last) = points.last().copied() {
        points.push(egui::pos2(plot.right(), last.y));
    }
    points.push(egui::pos2(plot.right(), y(value)));
    painter.add(egui::Shape::line(points, Stroke::new(1.5, color)));
    let current = Rect::from_two_pos(
        egui::pos2(plot.right() - 3.0, y(0.0)),
        egui::pos2(plot.right(), y(value)),
    );
    painter.rect_filled(current, 0.0, color.gamma_multiply(0.25));
    painter.circle_filled(egui::pos2(plot.right() - 1.5, y(value)), 2.2, color);
}
fn note_range(notes: &[f32; 128], history: Option<&History>) -> (u8, u8) {
    let mut low = 127_i16;
    let mut high = 0_i16;
    let mut observe = |note: u8| {
        low = low.min(i16::from(note));
        high = high.max(i16::from(note));
    };
    for (note, &value) in notes.iter().enumerate() {
        if value > 0.0 {
            observe(note as u8);
        }
    }
    if let Some(history) = history {
        for frame in &history.frames {
            for &(note, _) in &frame.notes {
                observe(note);
            }
        }
    }
    if low > high {
        return (48, 72);
    }
    let pad = ((12 - (high - low + 1)).max(0) + 1) / 2 + 1;
    ((low - pad).max(0) as u8, (high + pad).min(127) as u8)
}
fn piano_roll(
    ui: &egui::Ui,
    plot: Rect,
    notes: &[f32; 128],
    history: Option<&History>,
    now: f64,
    color: Color32,
) {
    let painter = ui.painter_at(plot);
    let (low, high) = note_range(notes, history);
    let row = plot.height() / f32::from(high - low + 1);
    let timeline = Rect::from_min_max(plot.min + Vec2::new(12.0, 0.0), plot.max);
    let note_rect = |note: u8, left: f32, right: f32| {
        let top = plot.bottom() - f32::from(note - low + 1) * row;
        Rect::from_min_max(
            egui::pos2(left, top + row * 0.1),
            egui::pos2(right, top + row * 0.9),
        )
    };
    for note in low..=high {
        let key = note_rect(note, plot.left(), plot.left() + 9.0);
        let black = matches!(note % 12, 1 | 3 | 6 | 8 | 10);
        let active = notes[usize::from(note)] > 0.0;
        painter.rect_filled(
            key,
            0.0,
            if active {
                color
            } else {
                ui.visuals()
                    .text_color()
                    .gamma_multiply(if black { 0.07 } else { 0.22 })
            },
        );
        if note % 12 == 0 {
            painter.hline(
                timeline.x_range(),
                key.bottom(),
                Stroke::new(0.5, ui.visuals().weak_text_color().gamma_multiply(0.12)),
            );
        }
    }
    if let Some(history) = history {
        for (i, frame) in history.frames.iter().enumerate() {
            let left = x(frame.time, now, timeline);
            let end = history.frames.get(i + 1).map_or(now, |f| f.time);
            let right = x(end, now, timeline).max(left + 1.5).min(timeline.right());
            for &(note, level) in &frame.notes {
                painter.rect_filled(
                    note_rect(note, left, right),
                    0.0,
                    color.gamma_multiply(0.2 + 0.8 * level),
                );
            }
        }
    }
    for (note, &level) in notes.iter().enumerate() {
        if level > 0.0 {
            painter.rect_filled(
                note_rect(note as u8, timeline.right() - 3.0, timeline.right()),
                0.0,
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn note_preview_keeps_polyphony_and_channel_filters() {
        let snapshot = crate::midi::test_snapshot(&[0x90, 60, 127, 64, 64, 0x91, 67, 32]);
        let mut binding = Binding {
            message: Message::NoteVelocity,
            number: None,
            ..Default::default()
        };
        let levels = snapshot.note_levels(&binding);
        assert_eq!(levels[60], 1.0);
        assert_eq!(levels[64], 64.0 / 127.0);
        assert_eq!(levels[67], 32.0 / 127.0);
        binding.channel = Some(2);
        let levels = snapshot.note_levels(&binding);
        assert_eq!(levels[60], 0.0);
        assert_eq!(levels[64], 0.0);
        assert_eq!(levels[67], 32.0 / 127.0);
        binding.number = Some(60);
        assert_eq!(snapshot.note_levels(&binding), [0.0; 128]);
    }
    #[test]
    fn quick_pad_hits_survive_release_and_rebinding_clears_old_pitches() {
        let binding = Binding {
            message: Message::NoteGate,
            number: None,
            ..Default::default()
        };
        let before = crate::midi::test_snapshot(&[]);
        let after = crate::midi::test_snapshot(&[0x90, 60, 100, 0x80, 60, 0]);
        assert_eq!(after.note_levels(&binding), [0.0; 128]);
        let mut history = History::new(&binding, 0);
        history.update(&binding, &before, 0.0);
        history.update(&binding, &after, 0.005);
        history.update(&binding, &after, 0.04);
        assert_eq!(history.frames.back().unwrap().notes, vec![(60, 1.0)]);
        history.update(&binding, &after, 0.08);
        assert!(history.frames.back().unwrap().notes.is_empty());
        let next = Binding {
            message: Message::PolyPressure,
            number: Some(72),
            ..binding
        };
        history.update(&next, &after, 0.12);
        assert_eq!(history.frames.len(), 1);
        assert!(history.frames[0].notes.is_empty());
    }
    #[test]
    fn message_previews_render_native_units_center_and_extreme_notes_at_all_sizes() {
        let ctx = egui::Context::default();
        for (message, bytes, expected) in [
            (Message::ControlChange, vec![0xb0, 1, 96], "96"),
            (Message::PitchBend, vec![0xe0, 0, 64], "+0"),
            (Message::NoteVelocity, vec![0x90, 0, 127, 127, 96], "127"),
            (Message::NoteGate, vec![0x90, 60, 127, 64, 96], "2 held"),
            (Message::ChannelPressure, vec![0xd0, 77], "77"),
            (
                Message::PolyPressure,
                vec![0x90, 60, 127, 0xa0, 60, 80],
                "80",
            ),
        ] {
            let snapshot = crate::midi::test_snapshot(&bytes);
            let binding = Binding {
                message,
                number: if message == Message::ControlChange {
                    Some(1)
                } else {
                    None
                },
                ..Default::default()
            };
            let mut history = History::new(&binding, 0);
            history.update(&binding, &snapshot, 1.0);
            for height in [20.0, 60.0, 160.0, 320.0] {
                let rect = Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(138.0, height));
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(200.0, 400.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        draw(
                            ui,
                            rect,
                            &binding,
                            &snapshot,
                            Some(&history),
                            1.0,
                            Color32::LIGHT_BLUE,
                        )
                    },
                );
                assert!(output.shapes.iter().any(
                    |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text==expected)
                ));
                let meshes =
                    ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
                for clipped in meshes {
                    if let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive {
                        assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
                    }
                }
                output.textures_delta.clear();
            }
        }
        let plot = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(120.0, 100.0));
        assert_eq!(
            formatted_value(Message::PitchBend, -1.0, &[0.0; 128]),
            "-8192"
        );
        assert_eq!(
            formatted_value(Message::PitchBend, 1.0, &[0.0; 128]),
            "+8191"
        );
        assert_eq!(x(0.0, 4.0, plot), plot.left());
        assert_eq!(x(4.0, 4.0, plot), plot.right());
    }
}
