use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use arc_swap::ArcSwap;
use realfft::num_complex::Complex32;
use realfft::{RealFftPlanner, RealToComplex};
use rtrb::Consumer;

pub type StereoSample = [f32; 2];

pub const FFT_SIZES: [usize; 6] = [512, 1024, 2048, 4096, 8192, 16384];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowFunction {
    Hann,
    Hamming,
    BlackmanHarris,
    Rectangular,
}

impl WindowFunction {
    pub const ALL: [Self; 4] = [
        Self::Hann,
        Self::Hamming,
        Self::BlackmanHarris,
        Self::Rectangular,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Hann => "Hann",
            Self::Hamming => "Hamming",
            Self::BlackmanHarris => "Blackman-Harris",
            Self::Rectangular => "Rectangular",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelMode {
    StereoMix,
    Left,
    Right,
}

impl ChannelMode {
    pub const ALL: [Self; 3] = [Self::StereoMix, Self::Left, Self::Right];

    pub const fn label(self) -> &'static str {
        match self {
            Self::StereoMix => "Stereo mix",
            Self::Left => "Left",
            Self::Right => "Right",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnalysisSettings {
    pub fft_size: usize,
    pub window: WindowFunction,
    pub channel_mode: ChannelMode,
    pub smoothing: f32,
    pub gain_db: f32,
    pub min_frequency: f32,
    pub max_frequency: f32,
    pub decay_db_per_second: f32,
    pub analysis_fps: u32,
}

impl Default for AnalysisSettings {
    fn default() -> Self {
        Self {
            fft_size: 4096,
            window: WindowFunction::Hann,
            channel_mode: ChannelMode::StereoMix,
            smoothing: 0.72,
            gain_db: 0.0,
            min_frequency: 20.0,
            max_frequency: 20_000.0,
            decay_db_per_second: 36.0,
            analysis_fps: 60,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FrequencyBin {
    pub frequency_hz: f32,
    pub magnitude: f32,
    pub dbfs: f32,
}

#[derive(Clone, Debug, Default)]
pub struct SpectrumBand {
    pub center_hz: f32,
    pub dbfs: f32,
}

#[derive(Clone, Debug)]
pub struct AnalysisFrame {
    pub sequence: u64,
    pub sample_rate: u32,
    pub channels: u32,
    pub fft_size: usize,
    pub latency_ms: f32,
    pub waveform_left: Vec<f32>,
    pub waveform_right: Vec<f32>,
    pub bins: Vec<FrequencyBin>,
    pub log_bands: Vec<SpectrumBand>,
    pub peak: [f32; 2],
    pub rms: [f32; 2],
    pub mix_peak: f32,
    pub mix_rms: f32,
    pub dominant_frequency_hz: f32,
    pub dominant_note: String,
    pub spectral_centroid_hz: f32,
    pub spectral_rolloff_hz: f32,
    pub spectral_flatness: f32,
    pub zero_crossing_rate: f32,
    pub crest_factor_db: f32,
    pub bpm: f32,
    pub bpm_confidence: f32,
    pub dropped_samples: u64,
}

impl Default for AnalysisFrame {
    fn default() -> Self {
        Self {
            sequence: 0,
            sample_rate: 48_000,
            channels: 2,
            fft_size: 4096,
            latency_ms: 0.0,
            waveform_left: vec![0.0; 512],
            waveform_right: vec![0.0; 512],
            bins: Vec::new(),
            log_bands: Vec::new(),
            peak: [0.0; 2],
            rms: [0.0; 2],
            mix_peak: 0.0,
            mix_rms: 0.0,
            dominant_frequency_hz: 0.0,
            dominant_note: "—".into(),
            spectral_centroid_hz: 0.0,
            spectral_rolloff_hz: 0.0,
            spectral_flatness: 0.0,
            zero_crossing_rate: 0.0,
            crest_factor_db: 0.0,
            bpm: 0.0,
            bpm_confidence: 0.0,
            dropped_samples: 0,
        }
    }
}

pub struct AnalysisRuntime {
    pub snapshot: Arc<ArcSwap<AnalysisFrame>>,
    pub settings: Arc<Mutex<AnalysisSettings>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl AnalysisRuntime {
    pub fn start(
        consumer: Consumer<StereoSample>,
        sample_rate: Arc<std::sync::atomic::AtomicU32>,
        channels: Arc<std::sync::atomic::AtomicU32>,
        dropped_samples: Arc<std::sync::atomic::AtomicU64>,
    ) -> Self {
        let snapshot = Arc::new(ArcSwap::from_pointee(AnalysisFrame::default()));
        let settings = Arc::new(Mutex::new(AnalysisSettings::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = {
            let snapshot = snapshot.clone();
            let settings = settings.clone();
            let stop = stop.clone();
            thread::Builder::new()
                .name("symphos-analysis".into())
                .spawn(move || {
                    run_analysis(
                        consumer,
                        snapshot,
                        settings,
                        sample_rate,
                        channels,
                        dropped_samples,
                        stop,
                    );
                })
                .expect("failed to start analysis thread")
        };

        Self {
            snapshot,
            settings,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for AnalysisRuntime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Analyzer {
    size: usize,
    fft: Arc<dyn RealToComplex<f32>>,
    input: Vec<f32>,
    output: Vec<Complex32>,
    window: Vec<f32>,
    smoothed_db: Vec<f32>,
    previous_magnitudes: Vec<f32>,
    flux_history: VecDeque<f32>,
    last_window: WindowFunction,
}

impl Analyzer {
    fn new(size: usize, window: WindowFunction) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(size);
        let input = fft.make_input_vec();
        let output = fft.make_output_vec();
        let bins = size / 2 + 1;
        Self {
            size,
            fft,
            input,
            output,
            window: build_window(size, window),
            smoothed_db: vec![-120.0; bins],
            previous_magnitudes: vec![0.0; bins],
            flux_history: VecDeque::with_capacity(1024),
            last_window: window,
        }
    }

    fn ensure_window(&mut self, window: WindowFunction) {
        if window != self.last_window {
            self.window = build_window(self.size, window);
            self.last_window = window;
        }
    }

    #[allow(clippy::too_many_arguments)] // The capture metadata is kept explicit at this hot boundary.
    fn analyze(
        &mut self,
        ring: &[StereoSample],
        write_cursor: usize,
        sample_rate: u32,
        channels: u32,
        settings: &AnalysisSettings,
        sequence: u64,
        dropped_samples: u64,
    ) -> AnalysisFrame {
        self.ensure_window(settings.window);
        let mut peak = [0.0_f32; 2];
        let mut squares = [0.0_f64; 2];
        let mut mix_peak = 0.0_f32;
        let mut mix_squares = 0.0_f64;
        let mut zero_crossings = 0_u32;
        let mut previous = 0.0_f32;
        let mut waveform_left = Vec::with_capacity(512);
        let mut waveform_right = Vec::with_capacity(512);
        let waveform_stride = (self.size / 512).max(1);

        let mean = (0..self.size)
            .map(|i| {
                let sample = ring[(write_cursor + ring.len() - self.size + i) % ring.len()];
                selected_sample(sample, settings.channel_mode)
            })
            .sum::<f32>()
            / self.size as f32;

        for i in 0..self.size {
            let sample = ring[(write_cursor + ring.len() - self.size + i) % ring.len()];
            peak[0] = peak[0].max(sample[0].abs());
            peak[1] = peak[1].max(sample[1].abs());
            squares[0] += f64::from(sample[0] * sample[0]);
            squares[1] += f64::from(sample[1] * sample[1]);
            let mix = (sample[0] + sample[1]) * 0.5;
            mix_peak = mix_peak.max(mix.abs());
            mix_squares += f64::from(mix) * f64::from(mix);

            let mono = selected_sample(sample, settings.channel_mode) - mean;
            if i > 0 && mono.signum() != previous.signum() {
                zero_crossings += 1;
            }
            previous = mono;
            self.input[i] = mono * self.window[i];

            if i % waveform_stride == 0 && waveform_left.len() < 512 {
                waveform_left.push(sample[0]);
                waveform_right.push(sample[1]);
            }
        }

        if self.fft.process(&mut self.input, &mut self.output).is_err() {
            return AnalysisFrame::default();
        }

        let window_sum = self.window.iter().sum::<f32>().max(f32::EPSILON);
        let bin_width = sample_rate as f32 / self.size as f32;
        let attack = 1.0 - settings.smoothing.clamp(0.0, 0.98);
        let release =
            (settings.decay_db_per_second / settings.analysis_fps.max(1) as f32).max(0.05);
        let mut bins = Vec::with_capacity(self.output.len());
        let mut magnitudes = Vec::with_capacity(self.output.len());
        let mut weighted_frequency = 0.0_f32;
        let mut magnitude_sum = 0.0_f32;
        let mut log_sum = 0.0_f32;
        let mut dominant = (0.0_f32, 0.0_f32);

        for (index, value) in self.output.iter().enumerate() {
            let frequency = index as f32 * bin_width;
            let edge_scale = if index == 0 || index + 1 == self.output.len() {
                1.0
            } else {
                2.0
            };
            let magnitude = value.norm() * edge_scale / window_sum;
            let raw_db = 20.0 * magnitude.max(1.0e-7).log10() + settings.gain_db;
            let smoothed = if raw_db >= self.smoothed_db[index] {
                self.smoothed_db[index] + (raw_db - self.smoothed_db[index]) * attack
            } else {
                (self.smoothed_db[index] - release).max(raw_db)
            };
            self.smoothed_db[index] = smoothed.clamp(-120.0, 12.0);
            magnitudes.push(magnitude);
            bins.push(FrequencyBin {
                frequency_hz: frequency,
                magnitude,
                dbfs: self.smoothed_db[index],
            });

            if frequency >= settings.min_frequency && frequency <= settings.max_frequency {
                weighted_frequency += frequency * magnitude;
                magnitude_sum += magnitude;
                log_sum += magnitude.max(1.0e-12).ln();
                if magnitude > dominant.1 {
                    dominant = (frequency, magnitude);
                }
            }
        }

        let spectral_centroid_hz = weighted_frequency / magnitude_sum.max(1.0e-12);
        let spectral_flatness = {
            let active_bins =
                ((settings.max_frequency - settings.min_frequency) / bin_width).max(1.0);
            (log_sum / active_bins).exp() / (magnitude_sum / active_bins).max(1.0e-12)
        }
        .clamp(0.0, 1.0);
        let spectral_rolloff_hz = rolloff_frequency(
            &magnitudes,
            bin_width,
            settings.min_frequency,
            settings.max_frequency,
        );
        let log_bands = make_log_bands(&bins, settings.min_frequency, settings.max_frequency, 96);

        let flux = magnitudes
            .iter()
            .zip(&self.previous_magnitudes)
            .map(|(now, before)| (now - before).max(0.0))
            .sum::<f32>()
            / magnitude_sum.max(1.0e-12);
        self.previous_magnitudes.copy_from_slice(&magnitudes);
        self.flux_history.push_back(flux);
        let max_flux_samples = (settings.analysis_fps * 10) as usize;
        while self.flux_history.len() > max_flux_samples {
            self.flux_history.pop_front();
        }
        let (bpm, bpm_confidence) = estimate_bpm(&self.flux_history, settings.analysis_fps);

        let rms = [
            (squares[0] / self.size as f64).sqrt() as f32,
            (squares[1] / self.size as f64).sqrt() as f32,
        ];
        let mono_peak = peak[0].max(peak[1]);
        let mono_rms = ((rms[0] * rms[0] + rms[1] * rms[1]) * 0.5).sqrt();

        AnalysisFrame {
            sequence,
            sample_rate,
            channels,
            fft_size: self.size,
            latency_ms: self.size as f32 * 1000.0 / sample_rate.max(1) as f32,
            waveform_left,
            waveform_right,
            bins,
            log_bands,
            peak,
            rms,
            mix_peak,
            mix_rms: (mix_squares / self.size as f64).sqrt() as f32,
            dominant_frequency_hz: dominant.0,
            dominant_note: frequency_to_note(dominant.0),
            spectral_centroid_hz,
            spectral_rolloff_hz,
            spectral_flatness,
            zero_crossing_rate: zero_crossings as f32 / self.size.saturating_sub(1).max(1) as f32,
            crest_factor_db: 20.0 * (mono_peak / mono_rms.max(1.0e-7)).max(1.0).log10(),
            bpm,
            bpm_confidence,
            dropped_samples,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_analysis(
    mut consumer: Consumer<StereoSample>,
    snapshot: Arc<ArcSwap<AnalysisFrame>>,
    settings: Arc<Mutex<AnalysisSettings>>,
    sample_rate: Arc<std::sync::atomic::AtomicU32>,
    channels: Arc<std::sync::atomic::AtomicU32>,
    dropped_samples: Arc<std::sync::atomic::AtomicU64>,
    stop: Arc<AtomicBool>,
) {
    let mut ring = vec![[0.0; 2]; FFT_SIZES[FFT_SIZES.len() - 1]];
    let mut write_cursor = 0_usize;
    let mut available = 0_usize;
    let mut since_analysis = 0_usize;
    let mut sequence = 0_u64;
    let mut analyzer = Analyzer::new(4096, WindowFunction::Hann);

    while !stop.load(Ordering::Acquire) {
        let mut received = 0_usize;
        while let Ok(sample) = consumer.pop() {
            ring[write_cursor] = sample;
            write_cursor = (write_cursor + 1) % ring.len();
            available = (available + 1).min(ring.len());
            since_analysis += 1;
            received += 1;
        }

        let current = settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if analyzer.size != current.fft_size {
            analyzer = Analyzer::new(current.fft_size, current.window);
            since_analysis = current.fft_size;
        }
        let rate = sample_rate.load(Ordering::Relaxed).max(1);
        let hop = ((rate / current.analysis_fps.max(1)) as usize)
            .clamp((current.fft_size / 16).max(1), current.fft_size / 2);

        if available >= current.fft_size && since_analysis >= hop {
            since_analysis %= hop;
            sequence += 1;
            let frame = analyzer.analyze(
                &ring,
                write_cursor,
                rate,
                channels.load(Ordering::Relaxed),
                &current,
                sequence,
                dropped_samples.load(Ordering::Relaxed),
            );
            snapshot.store(Arc::new(frame));
        }

        if received == 0 {
            thread::sleep(Duration::from_millis(1));
        }
    }
}

fn selected_sample(sample: StereoSample, mode: ChannelMode) -> f32 {
    match mode {
        ChannelMode::StereoMix => (sample[0] + sample[1]) * 0.5,
        ChannelMode::Left => sample[0],
        ChannelMode::Right => sample[1],
    }
}

fn build_window(size: usize, kind: WindowFunction) -> Vec<f32> {
    let denominator = size.saturating_sub(1).max(1) as f32;
    (0..size)
        .map(|i| {
            let phase = 2.0 * PI * i as f32 / denominator;
            match kind {
                WindowFunction::Hann => 0.5 - 0.5 * phase.cos(),
                WindowFunction::Hamming => 0.54 - 0.46 * phase.cos(),
                WindowFunction::BlackmanHarris => {
                    0.35875 - 0.48829 * phase.cos() + 0.14128 * (2.0 * phase).cos()
                        - 0.01168 * (3.0 * phase).cos()
                }
                WindowFunction::Rectangular => 1.0,
            }
        })
        .collect()
}

fn make_log_bands(
    bins: &[FrequencyBin],
    min_frequency: f32,
    max_frequency: f32,
    count: usize,
) -> Vec<SpectrumBand> {
    if bins.len() < 2 || min_frequency >= max_frequency {
        return Vec::new();
    }
    let ratio = (max_frequency / min_frequency.max(1.0)).powf(1.0 / count as f32);
    (0..count)
        .map(|band| {
            let low = min_frequency * ratio.powf(band as f32);
            let high = min_frequency * ratio.powf((band + 1) as f32);
            let mut db = -120.0_f32;
            for bin in bins {
                if bin.frequency_hz >= low && bin.frequency_hz < high {
                    db = db.max(bin.dbfs);
                }
            }
            SpectrumBand {
                center_hz: (low * high).sqrt(),
                dbfs: db,
            }
        })
        .collect()
}

fn rolloff_frequency(magnitudes: &[f32], bin_width: f32, min: f32, max: f32) -> f32 {
    let start = (min / bin_width).floor().max(0.0) as usize;
    let end = ((max / bin_width).ceil() as usize).min(magnitudes.len());
    let total = magnitudes[start.min(end)..end].iter().sum::<f32>();
    let target = total * 0.85;
    let mut sum = 0.0;
    for (index, magnitude) in magnitudes.iter().enumerate().take(end).skip(start) {
        sum += magnitude;
        if sum >= target {
            return index as f32 * bin_width;
        }
    }
    0.0
}

fn frequency_to_note(frequency: f32) -> String {
    if !frequency.is_finite() || frequency < 16.0 {
        return "—".into();
    }
    const NOTES: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    let midi = (69.0 + 12.0 * (frequency / 440.0).log2()).round() as i32;
    let note = NOTES[midi.rem_euclid(12) as usize];
    let octave = midi.div_euclid(12) - 1;
    format!("{note}{octave}")
}

fn estimate_bpm(flux: &VecDeque<f32>, fps: u32) -> (f32, f32) {
    if flux.len() < (fps * 4) as usize || fps == 0 {
        return (0.0, 0.0);
    }
    let mean = flux.iter().sum::<f32>() / flux.len() as f32;
    let centered: Vec<f32> = flux.iter().map(|v| (v - mean).max(0.0)).collect();
    let mut best = (0_usize, 0.0_f32);
    let mut score_sum = 0.0_f32;
    let min_lag = ((60.0 / 200.0) * fps as f32).round() as usize;
    let max_lag = fps as usize;
    for lag in min_lag.max(1)..=max_lag.min(centered.len() / 2) {
        let score = centered[lag..]
            .iter()
            .zip(&centered[..centered.len() - lag])
            .map(|(a, b)| a * b)
            .sum::<f32>();
        score_sum += score;
        if score > best.1 {
            best = (lag, score);
        }
    }
    if best.0 == 0 || best.1 <= f32::EPSILON {
        return (0.0, 0.0);
    }
    let bpm = 60.0 * fps as f32 / best.0 as f32;
    let candidate_count = max_lag.saturating_sub(min_lag).max(1) as f32;
    let confidence = (best.1 / (score_sum / candidate_count).max(1.0e-12) / 6.0).clamp(0.0, 1.0);
    (bpm, confidence)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_meters_measure_summed_samples_including_phase_cancellation() {
        let size = 1024;
        let settings = AnalysisSettings {
            fft_size: size,
            ..Default::default()
        };
        let mut analyzer = Analyzer::new(size, settings.window);
        for (sample, expected) in [([0.75, -0.75], 0.0), ([0.6, 0.6], 0.6), ([1.0, 0.0], 0.5)] {
            let frame = analyzer.analyze(&vec![sample; size], 0, 48_000, 2, &settings, 1, 0);
            assert!((frame.mix_rms - expected).abs() < 1.0e-6);
            assert!((frame.mix_peak - expected).abs() < 1.0e-6);
            assert!((frame.rms[0] - sample[0].abs()).abs() < 1.0e-6);
            assert!((frame.rms[1] - sample[1].abs()).abs() < 1.0e-6);
        }
    }

    #[test]
    fn note_conversion_is_correct() {
        assert_eq!(frequency_to_note(440.0), "A4");
        assert_eq!(frequency_to_note(261.63), "C4");
    }

    #[test]
    fn windows_have_expected_shape() {
        let hann = build_window(128, WindowFunction::Hann);
        assert!(hann[0].abs() < 1.0e-5);
        assert!(hann[127].abs() < 1.0e-5);
        assert!(hann[64] > 0.99);
    }

    #[test]
    fn log_bands_cover_requested_count() {
        let bins: Vec<_> = (0..1000)
            .map(|i| FrequencyBin {
                frequency_hz: i as f32 * 24.0,
                magnitude: 0.1,
                dbfs: -20.0,
            })
            .collect();
        let bands = make_log_bands(&bins, 20.0, 20_000.0, 96);
        assert_eq!(bands.len(), 96);
        assert!(bands.windows(2).all(|w| w[0].center_hz < w[1].center_hz));
    }

    #[test]
    fn analyzer_finds_known_sine_and_level() {
        let sample_rate = 48_000_u32;
        let size = 4096_usize;
        let frequency = 440.0_f32;
        let ring: Vec<StereoSample> = (0..size)
            .map(|index| {
                let sample = (2.0 * PI * frequency * index as f32 / sample_rate as f32).sin();
                [sample, sample]
            })
            .collect();
        let settings = AnalysisSettings {
            fft_size: size,
            ..Default::default()
        };
        let mut analyzer = Analyzer::new(size, settings.window);
        let frame = analyzer.analyze(&ring, 0, sample_rate, 2, &settings, 1, 0);
        let bin_width = sample_rate as f32 / size as f32;

        assert!((frame.dominant_frequency_hz - frequency).abs() <= bin_width);
        assert_eq!(frame.dominant_note, "A4");
        assert!((frame.rms[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01);
        assert!(frame.peak[0] > 0.99);
    }

    #[test]
    fn tempo_estimator_tracks_regular_onsets() {
        let fps = 60_u32;
        let flux: VecDeque<f32> = (0..fps * 6)
            .map(|frame| if frame % 30 == 0 { 1.0 } else { 0.0 })
            .collect();
        let (bpm, confidence) = estimate_bpm(&flux, fps);
        assert!((bpm - 120.0).abs() < 0.1);
        assert!(confidence > 0.0);
    }
}
