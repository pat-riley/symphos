//! Distinct readouts for the audio feed and the final modulation value.
use super::*;

pub(super) const MIN_STRIP_HEIGHT: f32 = 384.0;
pub(super) const FOOTER_HEIGHT: f32 = 110.0;

fn header(ui: &egui::Ui, rect: Rect, title: &str, destination: &str, value: &str) -> Rect {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    let mut job = egui::text::LayoutJob::simple(
        title.into(),
        egui::FontId::proportional(10.0),
        ui.visuals().text_color(),
        (rect.width() - 52.0).max(1.0),
    );
    job.wrap.max_rows = 1;
    let galley = painter.layout_job(job);
    painter.galley(
        rect.min + egui::vec2(5.0, 4.0),
        galley,
        ui.visuals().text_color(),
    );
    painter.text(
        rect.right_top() + egui::vec2(-5.0, 5.0),
        egui::Align2::RIGHT_TOP,
        value,
        egui::FontId::monospace(9.0),
        ui.visuals().weak_text_color(),
    );
    painter.text(
        rect.min + egui::vec2(5.0, 17.0),
        egui::Align2::LEFT_TOP,
        destination,
        egui::FontId::proportional(9.0),
        ui.visuals().weak_text_color(),
    );
    Rect::from_min_max(
        rect.min + egui::vec2(5.0, 30.0),
        rect.max - egui::vec2(5.0, 5.0),
    )
}

pub(super) fn audio(
    ui: &mut egui::Ui,
    rect: Rect,
    frame: Option<&AnalysisFrame>,
    color: egui::Color32,
    available: bool,
) {
    let db = frame.map_or_else(
        || "No input".into(),
        |frame| {
            if frame.mix_rms > 1e-7 {
                format!("{:.0} dB", 20.0 * frame.mix_rms.log10())
            } else {
                "−∞ dB".into()
            }
        },
    );
    let plot = header(
        ui,
        rect,
        "Audio output",
        if available {
            "To visualizers"
        } else {
            "No audio · control source"
        },
        if available { &db } else { "—" },
    );
    if available && plot.height() > 8.0 {
        audio_channel::spectrum_bars(ui, plot, frame, color);
    }
    ui.interact(rect, ui.id().with("audio-output-help"), egui::Sense::hover())
        .help_text("Waveform and spectrum supplied to visualizers that select this channel under Driven by. Audio gain, filters, channel routing and audio gates affect this feed. Detection, normalization and modulation shaping do not replace it. No speaker playback is changed.");
}

pub(super) fn modulation(
    ui: &mut egui::Ui,
    rect: Rect,
    d: &Definition,
    sample: &Sample,
    history: Option<&history::History>,
    color: egui::Color32,
) {
    let active = d.has_control_output();
    let value = if active {
        format!("{:.0}%", sample.value * 100.0)
    } else {
        "Off".into()
    };
    let plot = header(
        ui,
        rect,
        "Modulation output",
        if active {
            "To macros & parameters"
        } else {
            "No detector · audio continues"
        },
        &value,
    );
    if active && plot.height() > 8.0 {
        let signed = d.kind() == OutputKind::Bipolar;
        let high = history
            .into_iter()
            .flat_map(|h| h.points())
            .map(|p| p[1].abs())
            .fold(1.0_f32, f32::max)
            .max(sample.value.abs());
        let low = if signed { -high } else { 0.0 };
        let y = |value: f32| {
            plot.bottom() - ((value - low) / (high - low)).clamp(0.0, 1.0) * plot.height()
        };
        let painter = ui.painter_at(plot);
        for value in [low, (low + high) * 0.5, high] {
            painter.hline(
                plot.x_range(),
                y(value),
                egui::Stroke::new(0.5, ui.visuals().weak_text_color().gamma_multiply(0.18)),
            );
        }
        // Keep the signed center visible: centered balance is zero, not silence in the audio feed.
        painter.hline(
            plot.x_range(),
            y(0.0),
            egui::Stroke::new(0.7, ui.visuals().weak_text_color().gamma_multiply(0.35)),
        );
        if let Some(history) = history {
            let points: Vec<_> = history
                .points()
                .map(|[x, value]| egui::pos2(plot.left() + x * plot.width(), y(value)))
                .collect();
            if points.len() > 1 {
                painter.add(egui::Shape::line(points, egui::Stroke::new(1.7, color)));
            }
        }
    }
    ui.interact(rect, ui.id().with("modulation-output-help"), egui::Sense::hover())
        .help_text("Final control value supplied to macros and parameter mappings, after detection, response, normalization, extra control processors and modulation amount. Stereo balance can be zero while audio is active. An audio-only channel needs a detector to create modulation.");
}

pub(super) fn layout(bounds: Rect, audio: bool, modulation: bool) -> (Rect, Rect) {
    let gap = 8.0;
    let audio_height = if !audio {
        32.0
    } else if !modulation {
        bounds.height() - 40.0
    } else {
        (bounds.height() - gap) * 0.5
    };
    let audio_rect = Rect::from_min_max(
        bounds.min,
        egui::pos2(bounds.right(), bounds.top() + audio_height),
    );
    let modulation_rect = Rect::from_min_max(
        egui::pos2(bounds.left(), audio_rect.bottom() + gap),
        bounds.max,
    );
    (audio_rect, modulation_rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_outputs_show_audio_when_balance_is_zero_and_explicit_missing_branches() {
        let ctx = egui::Context::default();
        let mut panel = Panel::default();
        panel.act(StripAction::AddGuided(
            crate::signals::guided::Kind::Balance,
            0,
        ));
        let mut d = panel.bank.channels[0].clone();
        let color = channel_color(&d);
        let frame = AnalysisFrame {
            mix_rms: 0.5,
            ..Default::default()
        };
        let sample = Sample {
            value: 0.0,
            ..Default::default()
        };
        for input in [Input::System, Input::Lfo] {
            d.input = input;
            for modulation_available in [true, false] {
                if !modulation_available {
                    d.inserts.as_mut().unwrap().stages.clear();
                }
                // LFO is inherently a control source, even without detector inserts.
                let active = d.has_control_output();
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let bounds =
                        Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(140.0, 160.0));
                    let (a, m) = layout(bounds, input == Input::System, active);
                    assert!(bounds.contains_rect(a) && bounds.contains_rect(m));
                    assert!(a.bottom() + 8.0 <= m.top());
                    audio(ui, a, Some(&frame), color, input == Input::System);
                    modulation(ui, m, &d, &sample, None, color);
                });
                out.textures_delta.clear();
                let labels: Vec<_> = out
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) => Some(t.galley.job.text.as_str()),
                        _ => None,
                    })
                    .collect();
                for label in [
                    "Audio output",
                    "Modulation output",
                    if input == Input::System {
                        "-6 dB"
                    } else {
                        "No audio · control source"
                    },
                    if active {
                        "0%"
                    } else {
                        "No detector · audio continues"
                    },
                ] {
                    assert!(labels.contains(&label), "missing {label}: {labels:?}");
                }
                for s in &out.shapes {
                    if let egui::Shape::Text(t) = &s.shape {
                        assert!(
                            !t.galley.elided,
                            "output headings must remain readable: {}",
                            t.galley.job.text
                        );
                    }
                }
            }
        }
    }
}
