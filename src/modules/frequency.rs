use std::{collections::VecDeque, time::Instant};

use eframe::egui;

use crate::analysis::AnalysisFrame;
use crate::help::HoverHelp;

pub const BANDS: usize = 96;

#[derive(Clone, Debug, PartialEq)]
pub struct FrequencySettings {
    pub logarithmic: bool,
    pub min_hz: f32,
    pub max_hz: f32,
    pub gain: f32,
    pub smoothing: f32,
    pub frequency_smoothing: usize,
    pub note_labels: bool,
    pub tuning_hz: f32,
    pub decay: f32,
    pub floor: f32,
    pub ceiling: f32,
}

impl Default for FrequencySettings {
    fn default() -> Self {
        Self {
            logarithmic: true,
            min_hz: 20.0,
            max_hz: 20_000.0,
            gain: 0.0,
            smoothing: 0.72,
            frequency_smoothing: 0,
            note_labels: false,
            tuning_hz: 440.0,
            decay: 36.0,
            floor: -90.0,
            ceiling: 0.0,
        }
    }
}

impl FrequencySettings {
    pub fn same_processing(&self, other: &Self) -> bool {
        self.logarithmic == other.logarithmic
            && self.min_hz == other.min_hz
            && self.max_hz == other.max_hz
            && self.gain == other.gain
            && self.smoothing == other.smoothing
            && self.frequency_smoothing == other.frequency_smoothing
            && self.decay == other.decay
    }

    pub fn range_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.logarithmic, true, "Log").help_text("Space frequencies logarithmically to give bass and treble comparable room. Retained history is redrawn, not cleared.");
            ui.selectable_value(&mut self.logarithmic, false, "Linear").help_text("Space frequencies evenly in Hz. Retained history is redrawn, not cleared.");
        });
        ui.add(
            crate::parameter::Parameter::new(&mut self.min_hz, 10.0..=2000.0, 20.0)
                .bounds(1.0..=(self.max_hz - 1.0) as f64)
                .logarithmic(true)
                .text("Low Hz"),
        )
        .help_text("Lowest displayed frequency in Hz. Changes only this module's view.");
        ui.add(
            crate::parameter::Parameter::new(&mut self.max_hz, 2000.0..=24_000.0, 20_000.0)
                .bounds((self.min_hz + 1.0) as f64..=192_000.0)
                .logarithmic(true)
                .text("High Hz"),
        )
        .help_text("Highest displayed frequency in Hz, limited by the source's sample rate.");
        self.max_hz = self.max_hz.max(self.min_hz + 1.0);
        ui.separator();
        ui.checkbox(&mut self.note_labels, "Note labels").help_text("Label the frequency axis with the nearest equal-tempered note instead of Hz. These are frequency references, not detected notes or the song's key.");
        if self.note_labels {
            ui.add(crate::parameter::Parameter::new(&mut self.tuning_hz, 400.0..=480.0, 440.0).bounds(200.0..=1000.0).text("A4 Hz"))
                .help_text("Tuning reference for note labels. Standard concert tuning is A4 = 440 Hz; this does not change the audio.");
        }
    }

    pub fn response_controls(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().slider_width = ui.spacing().slider_width.min(78.0);
        ui.add(
            crate::parameter::Parameter::new(&mut self.gain, -24.0..=36.0, 0.0)
                .bounds(-96.0..=96.0)
                .text("Gain dB"),
        )
        .help_text(
            "Boost or reduce displayed levels in this module. Does not change audio volume.",
        );
        ui.add(
            crate::parameter::Parameter::new(&mut self.smoothing, 0.0..=0.98, 0.72)
                .text("Time smooth"),
        )
        .help_text("Higher values smooth rapid level changes; lower values respond faster.");
        ui.add(
            crate::parameter::Parameter::new(&mut self.decay, 6.0..=96.0, 36.0)
                .bounds(0.0..=240.0)
                .text("Decay dB/s"),
        )
        .help_text("How quickly displayed levels fall after a peak. Higher values fall faster.");
        ui.separator();
        ui.add(crate::parameter::Parameter::new(&mut self.frequency_smoothing, 0..=8, 0).text("Freq. smooth"))
            .help_text("Blend neighboring displayed frequency bands. 0 is off; higher values soften jagged peaks across frequency, independently of time smoothing. Retained history is reprocessed without clearing it; audio and FFT resolution are unchanged.");
    }

    pub fn level_controls(&mut self, ui: &mut egui::Ui) {
        ui.add(
            crate::parameter::Parameter::new(&mut self.floor, -120.0..=-24.0, -90.0)
                .bounds(-140.0..=(self.ceiling - 1.0) as f64)
                .text("Floor dB"),
        )
        .help_text("Quietest visible signal level. Lower this to reveal quieter detail.");
        ui.add(
            crate::parameter::Parameter::new(&mut self.ceiling, -18.0..=12.0, 0.0)
                .bounds((self.floor + 1.0) as f64..=48.0)
                .text("Ceiling dB"),
        )
        .help_text("Signal level mapped to maximum intensity or height. Does not limit the audio.");
    }

    pub fn frequency(&self, fraction: f32, sample_rate: u32) -> f32 {
        let high = self.max_hz.min(sample_rate as f32 / 2.0).max(1.0);
        let low = self.min_hz.min(high * 0.99).max(0.01);
        if self.logarithmic {
            low * (high / low).powf(fraction)
        } else {
            low + (high - low) * fraction
        }
    }

    pub fn intensity(&self, db: f32) -> f32 {
        ((db - self.floor) / (self.ceiling - self.floor)).clamp(0.0, 1.0)
    }

    pub fn axis_label(&self, fraction: f32, sample_rate: u32) -> String {
        let hz = self.frequency(fraction, sample_rate);
        if self.note_labels {
            note_label(hz, self.tuning_hz)
        } else {
            super::frequency_label(hz)
        }
    }
}

fn note_label(hz: f32, tuning_hz: f32) -> String {
    const NOTES: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    let midi = (69.0 + 12.0 * (hz.max(0.01) / tuning_hz).log2()).round() as i32;
    format!(
        "{}{}",
        NOTES[midi.rem_euclid(12) as usize],
        midi.div_euclid(12) - 1
    )
}

/// Triangular, edge-normalized smoothing of power, not dB values. A constant
/// spectrum stays constant and silence cannot pull a neighboring peak to -∞.
fn smooth_frequency(levels: &[f32; BANDS], radius: usize) -> [f32; BANDS] {
    if radius == 0 {
        return *levels;
    }
    let radius = radius.min(8);
    let power = levels.map(|db| 10.0_f32.powf(db / 10.0));
    std::array::from_fn(|i| {
        let mut sum = 0.0;
        let mut weight = 0.0;
        for (j, value) in power
            .iter()
            .enumerate()
            .take((i + radius + 1).min(BANDS))
            .skip(i.saturating_sub(radius))
        {
            let w = (radius + 1 - i.abs_diff(j)) as f32;
            sum += value * w;
            weight += w;
        }
        (10.0 * (sum / weight).max(1.0e-12).log10()).clamp(-120.0, 48.0)
    })
}

pub struct FrequencyData {
    pub settings: FrequencySettings,
    pub levels: [f32; BANDS],
    sequence: u64,
    last_update: Option<Instant>,
    signature: Option<(u32, usize, bool, u32, u32)>,
}

impl Default for FrequencyData {
    fn default() -> Self {
        Self {
            settings: FrequencySettings::default(),
            levels: [-120.0; BANDS],
            sequence: 0,
            last_update: None,
            signature: None,
        }
    }
}

impl FrequencyData {
    pub fn clear(&mut self) {
        self.levels.fill(-120.0);
        self.last_update = None;
        self.sequence = 0;
        self.signature = None;
    }

    pub fn update(&mut self, frame: &AnalysisFrame, now: Instant) -> (bool, bool) {
        self.update_magnitudes(
            frame.bins.len(),
            |i| frame.bins[i].magnitude,
            frame.sample_rate,
            frame.fft_size,
            frame.sequence,
            now,
        )
    }

    fn update_magnitudes(
        &mut self,
        count: usize,
        magnitude_at: impl Fn(usize) -> f32,
        sample_rate: u32,
        fft_size: usize,
        sequence: u64,
        now: Instant,
    ) -> (bool, bool) {
        let signature = (
            sample_rate,
            fft_size,
            self.settings.logarithmic,
            self.settings.min_hz.to_bits(),
            self.settings.max_hz.to_bits(),
        );
        let reset = self.signature != Some(signature);
        if reset {
            self.clear();
            self.signature = Some(signature);
        }
        if sequence == self.sequence || count == 0 {
            return (false, reset);
        }
        let dt = self
            .last_update
            .map_or(1.0 / 60.0, |last| now.duration_since(last).as_secs_f32())
            .min(0.25);
        let attack = 1.0 - self.settings.smoothing.powf(dt * 60.0);
        let bins_per_hz = fft_size as f32 / sample_rate.max(1) as f32;
        let mut targets = [-120.0; BANDS];
        for (i, target) in targets.iter_mut().enumerate() {
            let low = self
                .settings
                .frequency(i as f32 / BANDS as f32, sample_rate);
            let high = self
                .settings
                .frequency((i + 1) as f32 / BANDS as f32, sample_rate);
            let start = ((low * bins_per_hz).ceil() as usize).min(count);
            let end = ((high * bins_per_hz).ceil() as usize).min(count);
            let magnitude = if start < end {
                (start..end).map(&magnitude_at).fold(0.0_f32, f32::max)
            } else {
                let index = (((low + high) * 0.5 * bins_per_hz).round() as usize).min(count - 1);
                magnitude_at(index)
            };
            *target =
                (20.0 * magnitude.max(1.0e-7).log10() + self.settings.gain).clamp(-120.0, 48.0);
        }
        for (i, db) in smooth_frequency(&targets, self.settings.frequency_smoothing)
            .into_iter()
            .enumerate()
        {
            self.levels[i] = if db > self.levels[i] {
                self.levels[i] + (db - self.levels[i]) * attack
            } else {
                (self.levels[i] - self.settings.decay * dt).max(db)
            };
        }
        self.sequence = sequence;
        self.last_update = Some(now);
        (true, reset)
    }
}

pub struct HistoryRow {
    pub time: Instant,
    pub levels: [f32; BANDS],
    pub magnitudes: Vec<f32>,
    pub sequence: u64,
}

#[derive(Default)]
pub struct History {
    pub data: FrequencyData,
    pub rows: VecDeque<HistoryRow>,
    format: Option<(u32, usize)>,
    applied_settings: Option<FrequencySettings>,
    last_sequence: u64,
}

impl History {
    pub fn clear(&mut self) {
        self.data.clear();
        self.rows.clear();
        self.format = None;
        self.applied_settings = None;
        self.last_sequence = 0;
    }

    pub fn update(&mut self, frame: &AnalysisFrame, now: Instant, live: bool) {
        while self
            .rows
            .front()
            .is_some_and(|row| now.duration_since(row.time).as_secs_f32() > 30.0)
        {
            self.rows.pop_front();
        }
        if live
            && !frame.bins.is_empty()
            && self.format != Some((frame.sample_rate, frame.fft_size))
        {
            self.clear();
            self.format = Some((frame.sample_rate, frame.fft_size));
        }
        // Replay retained raw magnitudes when display processing changes. Merely
        // relabeling old bands would incorrectly move their frequencies.
        let processing_changed = self
            .applied_settings
            .as_ref()
            .is_none_or(|previous| !previous.same_processing(&self.data.settings));
        if processing_changed {
            self.data.clear();
            if let Some((sample_rate, fft_size)) = self.format {
                for row in &mut self.rows {
                    self.data.update_magnitudes(
                        row.magnitudes.len(),
                        |i| row.magnitudes[i],
                        sample_rate,
                        fft_size,
                        row.sequence,
                        row.time,
                    );
                    row.levels = self.data.levels;
                }
            }
            self.applied_settings = Some(self.data.settings.clone());
        }
        if live
            && frame.sequence != self.last_sequence
            && self
                .rows
                .back()
                .is_none_or(|row| now.duration_since(row.time).as_secs_f32() >= 1.0 / 30.0)
        {
            let (changed, _) = self.data.update(frame, now);
            if changed {
                self.last_sequence = frame.sequence;
                self.rows.push_back(HistoryRow {
                    time: now,
                    levels: self.data.levels,
                    magnitudes: frame.bins.iter().map(|bin| bin.magnitude).collect(),
                    sequence: frame.sequence,
                });
            }
        }
        while self.rows.len() > 901 {
            self.rows.pop_front();
        }
    }

    pub fn sample(&self, now: Instant, age: f32) -> Option<&[f32; BANDS]> {
        let target = now.checked_sub(std::time::Duration::from_secs_f32(age.max(0.0)))?;
        let index = self.rows.partition_point(|row| row.time <= target);
        let row = self.rows.get(index.saturating_sub(1))?;
        // Leave actual gaps blank instead of stretching a stale snapshot over time.
        let distance = if target >= row.time {
            target.duration_since(row.time)
        } else {
            row.time.duration_since(target)
        };
        (distance.as_secs_f32() < 0.12).then_some(&row.levels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::FrequencyBin;
    use std::time::Duration;

    #[test]
    fn frequency_smoothing_is_bounded_symmetric_and_preserves_flat_spectra() {
        let mut spike = [-120.0; BANDS];
        spike[48] = 0.0;
        assert_eq!(smooth_frequency(&spike, 0), spike);
        for radius in 1..=8 {
            let smooth = smooth_frequency(&spike, radius);
            assert!(smooth[48] < 0.0 && smooth[48] > -12.0);
            assert!(smooth[47] > -120.0 && smooth[47] < smooth[48]);
            for offset in 1..=radius {
                assert_eq!(smooth[48 - offset], smooth[48 + offset]);
            }
            assert_eq!(smooth[48 + radius + 1], -120.0);
            for db in [-120.0, -45.0, 0.0, 48.0] {
                assert!(
                    smooth_frequency(&[db; BANDS], radius)
                        .iter()
                        .all(|value| value.is_finite() && (*value - db).abs() < 0.001)
                );
            }
        }
    }

    #[test]
    fn note_labels_cover_octaves_accidentals_and_custom_tuning() {
        for (hz, label) in [
            (440.0, "A4"),
            (880.0, "A5"),
            (261.6256, "C4"),
            (277.1826, "C♯4"),
            (27.5, "A0"),
        ] {
            assert_eq!(note_label(hz, 440.0), label);
        }
        assert_eq!(note_label(432.0, 432.0), "A4");
        let mut settings = FrequencySettings {
            min_hz: 440.0,
            ..FrequencySettings::default()
        };
        assert_eq!(settings.axis_label(0.0, 48000), "440");
        settings.note_labels = true;
        assert_eq!(settings.axis_label(0.0, 48000), "A4");
    }

    #[test]
    fn spatial_smoothing_replays_retained_audio_but_labels_do_not_reprocess_it() {
        let mut history = History::default();
        history.data.settings.smoothing = 0.0;
        let mut tone = frame(1);
        for bin in &mut tone.bins {
            bin.magnitude = 1.0e-7;
        }
        tone.bins[85].magnitude = 1.0;
        let now = Instant::now();
        history.update(&tone, now, true);
        let original = history.rows[0].levels;
        let magnitudes = history.rows[0].magnitudes.clone();
        history.data.settings.frequency_smoothing = 4;
        history.update(&tone, now, false);
        assert_ne!(history.rows[0].levels, original);
        assert_eq!(history.rows.len(), 1);
        assert_eq!(history.rows[0].time, now);
        assert_eq!(history.rows[0].magnitudes, magnitudes);
        let smooth = history.rows[0].levels;
        history.data.settings.note_labels = true;
        history.data.settings.tuning_hz = 432.0;
        history.update(&tone, now, false);
        assert_eq!(history.rows[0].levels, smooth);
        history.data.settings.frequency_smoothing = 0;
        history.update(&tone, now, false);
        assert_eq!(history.rows[0].levels, original);
    }

    fn frame(sequence: u64) -> AnalysisFrame {
        AnalysisFrame {
            sequence,
            bins: (0..=2048)
                .map(|i| FrequencyBin {
                    frequency_hz: i as f32 * 48000.0 / 4096.0,
                    magnitude: 0.5,
                    dbfs: -100.0,
                })
                .collect(),
            ..AnalysisFrame::default()
        }
    }

    #[test]
    fn independent_gain_and_range_ignore_shared_display_smoothing() {
        let mut a = FrequencyData::default();
        let mut b = FrequencyData::default();
        a.settings.smoothing = 0.0;
        b.settings.smoothing = 0.0;
        b.settings.gain = 12.0;
        b.settings.min_hz = 200.0;
        a.update(&frame(1), Instant::now());
        b.update(&frame(1), Instant::now());
        assert!((a.levels[20] + 6.0206).abs() < 0.01);
        assert!((b.levels[20] - a.levels[20] - 12.0).abs() < 0.01);
        assert_eq!(a.settings.min_hz, 20.0);
    }

    #[test]
    fn history_uses_elapsed_time_and_expires_stale_audio() {
        let mut history = History::default();
        let start = Instant::now();
        history.update(&frame(1), start, true);
        history.update(&frame(1), start + Duration::from_secs(1), true);
        assert_eq!(history.rows.len(), 1);
        assert!(
            history
                .sample(start + Duration::from_secs(1), 0.0)
                .is_none()
        );
        assert!(
            history
                .sample(start + Duration::from_secs(1), 1.0)
                .is_some()
        );
        history.update(&frame(1), start + Duration::from_secs(31), false);
        assert!(history.rows.is_empty());
    }

    #[test]
    fn display_changes_reprocess_history_without_changing_its_timestamps() {
        let mut history = History::default();
        let start = Instant::now();
        let first = frame(1);
        let mut second = frame(2);
        for bin in &mut second.bins {
            bin.magnitude = 0.1;
        }
        let second_time = start + Duration::from_millis(50);
        history.update(&first, start, true);
        history.update(&second, second_time, true);
        history.data.settings.logarithmic = false;
        history.data.settings.min_hz = 500.0;
        history.data.settings.max_hz = 12000.0;
        history.data.settings.gain = 12.0;
        history.data.settings.smoothing = 0.2;
        history.data.settings.decay = 90.0;
        history.update(&second, second_time, false);
        assert_eq!(history.rows.len(), 2);
        assert_eq!(history.rows[0].time, start);
        assert_eq!(history.rows[1].time, second_time);
        let mut reference = FrequencyData {
            settings: history.data.settings.clone(),
            ..FrequencyData::default()
        };
        reference.update(&first, start);
        assert_eq!(history.rows[0].levels, reference.levels);
        reference.update(&second, second_time);
        assert_eq!(history.rows[1].levels, reference.levels);
        assert!(history.data.settings.frequency(1.0, 22050) <= 11025.0);
    }

    #[test]
    fn expanding_range_recovers_old_frequencies_outside_original_view() {
        let mut history = History::default();
        history.data.settings.smoothing = 0.0;
        history.data.settings.max_hz = 2000.0;
        let mut tone = frame(1);
        for bin in &mut tone.bins {
            bin.magnitude = 1.0e-7;
        }
        tone.bins[853].magnitude = 1.0; // Approximately 10 kHz.
        let now = Instant::now();
        history.update(&tone, now, true);
        assert!(history.rows[0].levels.iter().all(|db| *db <= -119.0));
        history.data.settings.max_hz = 20000.0;
        history.update(&tone, now, true);
        assert_eq!(history.rows.len(), 1);
        assert!(history.rows[0].levels.iter().any(|db| *db > -1.0));
    }

    #[test]
    fn changing_controls_does_not_revive_expired_audio() {
        let mut history = History::default();
        let start = Instant::now();
        history.update(&frame(1), start, true);
        history.data.settings.gain = 6.0;
        history.update(&frame(1), start + Duration::from_secs(31), true);
        assert!(history.rows.is_empty());
    }

    #[test]
    fn capture_format_changes_still_clear_incompatible_history() {
        let mut history = History::default();
        let start = Instant::now();
        history.update(&frame(1), start, true);
        let mut next = frame(2);
        next.sample_rate = 44100;
        history.update(&next, start + Duration::from_millis(50), true);
        assert_eq!(history.rows.len(), 1);
        assert_eq!(history.rows[0].sequence, 2);
    }
}
