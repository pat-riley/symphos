use std::{collections::VecDeque, time::Instant};

use eframe::egui;

use crate::analysis::AnalysisFrame;

pub const BANDS: usize = 96;

#[derive(Clone, Debug, PartialEq)]
pub struct FrequencySettings {
    pub logarithmic: bool,
    pub min_hz: f32,
    pub max_hz: f32,
    pub gain: f32,
    pub smoothing: f32,
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
            decay: 36.0,
            floor: -90.0,
            ceiling: 0.0,
        }
    }
}

impl FrequencySettings {
    pub fn range_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.logarithmic, true, "Log");
            ui.selectable_value(&mut self.logarithmic, false, "Linear");
        });
        ui.add(
            egui::Slider::new(&mut self.min_hz, 10.0..=2000.0)
                .logarithmic(true)
                .text("Low Hz"),
        );
        ui.add(
            egui::Slider::new(&mut self.max_hz, 2000.0..=24_000.0)
                .logarithmic(true)
                .text("High Hz"),
        );
        self.max_hz = self.max_hz.max(self.min_hz + 1.0);
    }

    pub fn response_controls(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Slider::new(&mut self.gain, -24.0..=36.0).text("Gain dB"));
        ui.add(egui::Slider::new(&mut self.smoothing, 0.0..=0.98).text("Smooth"));
        ui.add(egui::Slider::new(&mut self.decay, 6.0..=96.0).text("Decay dB/s"));
    }

    pub fn level_controls(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Slider::new(&mut self.floor, -120.0..=-24.0).text("Floor dB"));
        ui.add(egui::Slider::new(&mut self.ceiling, -18.0..=12.0).text("Ceiling dB"));
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
        for i in 0..BANDS {
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
            let db =
                (20.0 * magnitude.max(1.0e-7).log10() + self.settings.gain).clamp(-120.0, 48.0);
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
        let processing_changed = self.applied_settings.as_ref().is_none_or(|previous| {
            previous.logarithmic != self.data.settings.logarithmic
                || previous.min_hz != self.data.settings.min_hz
                || previous.max_hz != self.data.settings.max_hz
                || previous.gain != self.data.settings.gain
                || previous.smoothing != self.data.settings.smoothing
                || previous.decay != self.data.settings.decay
        });
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
