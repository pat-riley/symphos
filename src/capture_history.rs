//! Renderer-independent, bounded frequency capture. Owned by the analysis lane.
use crate::analysis::AnalysisFrame;
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

pub struct SpectralFrame {
    pub sequence: u64,
    pub time: Instant,
    pub sample_rate: u32,
    pub fft_size: usize,
    pub magnitudes: Arc<[f32]>,
}

#[derive(Default)]
pub struct CaptureHistory {
    epoch: u64,
    rows: VecDeque<Arc<SpectralFrame>>,
}

impl CaptureHistory {
    pub fn reset(&mut self, epoch: u64) {
        self.epoch = epoch;
        self.rows.clear();
    }
    pub fn push(&mut self, frame: &AnalysisFrame, now: Instant) {
        if frame.capture_epoch != self.epoch || frame.bins.is_empty() {
            return;
        }
        if self.rows.back().is_some_and(|row| {
            (row.sample_rate, row.fft_size) != (frame.sample_rate, frame.fft_size)
        }) {
            self.rows.clear();
        }
        while self
            .rows
            .front()
            .is_some_and(|row| now.saturating_duration_since(row.time) > Duration::from_secs(30))
        {
            self.rows.pop_front();
        }
        if self.rows.back().is_some_and(|row| {
            frame.sequence <= row.sequence
                || now.saturating_duration_since(row.time).as_secs_f64() < 1.0 / 30.0
        }) {
            return;
        }
        self.rows.push_back(Arc::new(SpectralFrame {
            sequence: frame.sequence,
            time: now,
            sample_rate: frame.sample_rate,
            fft_size: frame.fft_size,
            magnitudes: frame.bins.iter().map(|bin| bin.magnitude).collect(),
        }));
        while self.rows.len() > 901 {
            self.rows.pop_front();
        }
    }
    pub fn since(&self, sequence: u64) -> Vec<Arc<SpectralFrame>> {
        let start = self.rows.partition_point(|row| row.sequence <= sequence);
        self.rows.iter().skip(start).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_real_time_history_without_any_ui_frames_and_bounds_memory() {
        let mut history = CaptureHistory::default();
        let now = Instant::now();
        let mut frame = AnalysisFrame {
            bins: vec![crate::analysis::FrequencyBin {
                magnitude: 0.5,
                ..Default::default()
            }],
            ..Default::default()
        };
        for i in 0..1500 {
            frame.sequence = i + 1;
            history.push(&frame, now + Duration::from_millis(i * 34));
        }
        let rows = history.since(0);
        assert!(rows.len() > 800 && rows.len() <= 901);
        assert_eq!(rows.last().unwrap().sequence, 1500);
        assert!(rows.last().unwrap().time.duration_since(rows[0].time) <= Duration::from_secs(30));
        assert_eq!(history.since(1498).len(), 2);
    }
    #[test]
    fn old_source_packets_cannot_reenter_a_reset_archive() {
        let mut history = CaptureHistory::default();
        let frame = AnalysisFrame {
            sequence: 1,
            bins: vec![Default::default()],
            ..Default::default()
        };
        history.reset(1);
        history.push(&frame, Instant::now());
        assert!(history.since(0).is_empty());
    }
}
