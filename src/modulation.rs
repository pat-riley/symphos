//! Capture-independent musical controls. Settings are saved; envelopes and meters are not.
use crate::analysis::{AnalysisFrame, FrequencyBin};
use serde::{Deserialize, Serialize};

mod ui;

pub const MAX_LAYERS: usize = 12;
const MAX_ROUTES: usize = 24;

#[derive(Clone, Debug)]
pub struct AudioFeatures {
    frequencies: Vec<f32>,
    powers: Vec<f64>,
    pub spectrum: [f32; 96],
    rms: f32,
    balance: f32,
}
impl AudioFeatures {
    pub fn new(bins: &[FrequencyBin], rms: f32, stereo: [f32; 2]) -> Self {
        let mut bins: Vec<_> = bins
            .iter()
            .filter(|b| b.frequency_hz.is_finite() && b.magnitude.is_finite() && b.magnitude >= 0.0)
            .collect();
        bins.sort_by(|a, b| a.frequency_hz.total_cmp(&b.frequency_hz));
        let mut powers = vec![0.0];
        for b in &bins {
            powers.push(powers.last().copied().unwrap_or(0.0) + f64::from(b.magnitude).powi(2));
        }
        let sum = stereo[0] + stereo[1];
        let mut features = Self {
            frequencies: bins.iter().map(|b| b.frequency_hz).collect(),
            powers,
            spectrum: [0.0; 96],
            rms: finite(rms, 0.0).max(0.0),
            balance: if sum.is_finite() && sum > 0.0001 {
                ((stereo[1] - stereo[0]) / sum).clamp(-1.0, 1.0)
            } else {
                0.0
            },
        };
        features.spectrum = std::array::from_fn(|i| {
            let (lo, hi) = band_range(20.0, 20_000.0, i, 96);
            ((20.0 * features.band(lo, hi).max(1e-9).log10() + 90.0) / 90.0).clamp(0.0, 1.0)
        });
        features
    }
    pub fn band(&self, low: f32, high: f32) -> f32 {
        let a = self.frequencies.partition_point(|f| *f < low);
        let b = self
            .frequencies
            .partition_point(|f| *f < high || (high >= 20_000.0 && *f == high));
        ((self.powers[b] - self.powers[a]).max(0.0).sqrt()) as f32
    }
}
// Logarithmic subdivisions keep equal musical width when layer counts change.
pub fn band_range(low: f32, high: f32, index: usize, count: usize) -> (f32, f32) {
    let edge = |i: usize| low * (high / low).powf(i as f32 / count.max(1) as f32);
    (
        if index == 0 { low } else { edge(index) },
        if index + 1 == count {
            high
        } else {
            edge(index + 1)
        },
    )
}
fn finite(v: f32, default: f32) -> f32 {
    if v.is_finite() { v } else { default }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    Motion,
    Expansion,
    Swirl,
    Turbulence,
    Brightness,
    Size,
    Steering,
    LayerBend,
    LayerRipple,
    LayerShimmer,
}
impl Target {
    pub fn label(self) -> &'static str {
        match self {
            Self::Motion => "Motion speed",
            Self::Expansion => "Expansion",
            Self::Swirl => "Swirling flow",
            Self::Turbulence => "Turbulence",
            Self::Brightness => "Brightness",
            Self::Size => "Particle size",
            Self::Steering => "Horizontal steering",
            Self::LayerBend => "Layer bend",
            Self::LayerRipple => "Layer ripples",
            Self::LayerShimmer => "Layer shimmer",
        }
    }
    fn layered(self) -> bool {
        matches!(
            self,
            Self::LayerBend | Self::LayerRipple | Self::LayerShimmer
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visualizer {
    Flux,
    Veroni,
    Strata,
}
impl Visualizer {
    fn targets(self) -> &'static [Target] {
        use Target::*;
        match self {
            Self::Flux | Self::Veroni => &[
                Motion, Expansion, Swirl, Turbulence, Brightness, Size, Steering,
            ],
            Self::Strata => &[
                Motion,
                LayerBend,
                LayerRipple,
                LayerShimmer,
                Brightness,
                Size,
            ],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Band,
    FullMix,
    Stereo,
}
impl Source {
    fn label(self) -> &'static str {
        match self {
            Self::Band => "Frequency band",
            Self::FullMix => "Full mix",
            Self::Stereo => "Stereo balance",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    Follow,
    Gate,
    Transient,
}
impl Response {
    fn label(self) -> &'static str {
        match self {
            Self::Follow => "Level follower",
            Self::Gate => "Gated ADSR",
            Self::Transient => "Transient ADSR",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Envelope {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}
impl Default for Envelope {
    fn default() -> Self {
        Self {
            attack: 40.0,
            decay: 180.0,
            sustain: 0.65,
            release: 600.0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Route {
    pub id: u64,
    pub enabled: bool,
    pub source: Source,
    pub target: Target,
    pub low: f32,
    pub high: f32,
    pub response: Response,
    pub amount: f32,
    pub floor_db: f32,
    pub gain_db: f32,
    pub curve: f32,
    pub threshold: f32,
    pub hold_ms: f32,
    pub envelope: Envelope,
    pub output_min: f32,
    pub output_max: f32,
    /// Zero distributes the band over every layer; positive values select a single layer (bottom = 1).
    pub layer: usize,
}
impl Default for Route {
    fn default() -> Self {
        Self {
            id: 1,
            enabled: true,
            source: Source::Band,
            target: Target::Expansion,
            low: 20.0,
            high: 250.0,
            response: Response::Follow,
            amount: 1.0,
            floor_db: -66.0,
            gain_db: 0.0,
            curve: 2.0,
            threshold: 0.25,
            hold_ms: 120.0,
            envelope: Envelope::default(),
            output_min: 0.0,
            output_max: 1.0,
            layer: 0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModulationSettings {
    pub ambient: f32,
    pub amount: f32,
    pub adaptive: bool,
    pub routes: Vec<Route>,
}
impl Default for ModulationSettings {
    fn default() -> Self {
        Self {
            ambient: 0.25,
            amount: 1.0,
            adaptive: false,
            routes: Vec::new(),
        }
    }
}
impl ModulationSettings {
    pub fn for_visualizer(kind: Visualizer) -> Self {
        use Target::*;
        let mut settings = Self::default();
        let entries: &[(Target, f32, f32, f32)] = match kind {
            Visualizer::Flux | Visualizer::Veroni => &[
                (Expansion, 20.0, 250.0, 1.0),
                (Swirl, 250.0, 4000.0, 1.0),
                (Turbulence, 4000.0, 20000.0, 1.0),
                (Brightness, 20.0, 20000.0, 1.0),
                (Size, 250.0, 20000.0, 0.7),
                (Motion, 20.0, 20000.0, 1.0),
            ],
            Visualizer::Strata => &[
                (LayerBend, 20.0, 20000.0, 0.9),
                (LayerRipple, 20.0, 20000.0, 0.7),
                (LayerShimmer, 20.0, 20000.0, 0.6),
                (Brightness, 20.0, 20000.0, 1.0),
                (Size, 4000.0, 20000.0, 0.6),
            ],
        };
        for &(target, low, high, amount) in entries {
            settings.routes.push(Route {
                id: settings.routes.len() as u64 + 1,
                target,
                low,
                high,
                amount,
                ..Route::default()
            });
        }
        if kind != Visualizer::Strata {
            settings.routes.push(Route {
                id: 7,
                source: Source::Stereo,
                target: Steering,
                amount: 0.35,
                ..Route::default()
            });
        }
        settings
    }
    pub fn sanitize(&mut self, kind: Visualizer) {
        self.ambient = finite(self.ambient, 0.25).clamp(0.0, 1.0);
        self.amount = finite(self.amount, 1.0).clamp(0.0, 4.0);
        self.routes.truncate(MAX_ROUTES);
        let mut ids = std::collections::HashSet::new();
        for (index, r) in self.routes.iter_mut().enumerate() {
            if r.id == 0 || !ids.insert(r.id) {
                r.id = index as u64 + 1;
                while !ids.insert(r.id) {
                    r.id = r.id.wrapping_add(1).max(1);
                }
            }
            if !kind.targets().contains(&r.target) {
                r.target = kind.targets()[0];
            }
            r.low = finite(r.low, 20.0).clamp(20.0, 19980.0);
            r.high = finite(r.high, 20000.0).clamp(r.low * 1.001, 20000.0);
            r.amount = finite(r.amount, 1.0).clamp(-3.0, 3.0);
            r.floor_db = finite(r.floor_db, -66.0).clamp(-90.0, -12.0);
            r.gain_db = finite(r.gain_db, 0.0).clamp(-24.0, 24.0);
            r.curve = finite(r.curve, 2.0).clamp(0.25, 4.0);
            r.threshold = finite(r.threshold, 0.25).clamp(0.01, 0.99);
            r.hold_ms = finite(r.hold_ms, 120.0).clamp(20.0, 2000.0);
            r.envelope.attack = finite(r.envelope.attack, 40.0).clamp(1.0, 2000.0);
            r.envelope.decay = finite(r.envelope.decay, 180.0).clamp(1.0, 3000.0);
            r.envelope.sustain = finite(r.envelope.sustain, 0.65).clamp(0.0, 1.0);
            r.envelope.release = finite(r.envelope.release, 600.0).clamp(1.0, 5000.0);
            r.output_min = finite(r.output_min, 0.0).clamp(0.0, 1.0);
            r.output_max = finite(r.output_max, 1.0).clamp(r.output_min, 1.0);
            r.layer = r.layer.min(MAX_LAYERS);
        }
    }
    pub fn migrate(
        kind: Visualizer,
        sensitivity: f32,
        floor: f32,
        attack: f32,
        release: f32,
    ) -> Self {
        let mut s = Self::for_visualizer(kind);
        s.amount = sensitivity;
        s.ambient = 1.0;
        for r in &mut s.routes {
            r.floor_db = floor;
            r.envelope.attack = attack;
            r.envelope.release = release;
        }
        s
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}
#[derive(Clone, Debug, Default)]
struct Voice {
    value: f32,
    stage: Stage,
    elapsed: f32,
    start: f32,
    gate: bool,
    previous: f32,
    slow: f32,
    hold: f32,
    cooldown: f32,
    peak: f32,
    pub input: f32,
    pub output: f32,
}
impl Voice {
    fn stage(&mut self, stage: Stage) {
        self.stage = stage;
        self.elapsed = 0.0;
        self.start = self.value;
    }
    fn advance(&mut self, amplitude: f32, sign: f32, r: &Route, adaptive: bool, dt: f32) {
        let db = 20.0 * amplitude.max(1e-9).log10() + r.gain_db;
        self.peak = (self.peak - dt * 3.0).max(db).max(-42.0);
        let ceiling = if adaptive {
            self.peak.max(r.floor_db + 12.0)
        } else {
            -6.0
        };
        let input = ((db - r.floor_db) / (ceiling - r.floor_db))
            .clamp(0.0, 1.0)
            .powf(r.curve);
        self.input = input;
        if r.response == Response::Follow {
            let ms = if input > self.value {
                r.envelope.attack
            } else {
                r.envelope.release
            };
            self.value += (input - self.value) * (1.0 - (-dt * 1000.0 / ms).exp());
        } else {
            self.hold = (self.hold - dt).max(0.0);
            self.cooldown = (self.cooldown - dt).max(0.0);
            let gate = if r.response == Response::Transient {
                if input > r.threshold
                    && input - self.slow > r.threshold * 0.5
                    && input > self.previous + 0.005
                    && self.cooldown == 0.0
                {
                    self.hold = r.hold_ms / 1000.0;
                    self.cooldown = (r.hold_ms / 1000.0).max(0.08);
                    self.stage(Stage::Attack);
                }
                self.hold > 0.0
            } else {
                input >= r.threshold * if self.gate { 0.8 } else { 1.0 }
            };
            if gate && !self.gate {
                self.stage(Stage::Attack);
            }
            if !gate && self.gate {
                self.stage(Stage::Release);
            }
            self.gate = gate;
            let mut remaining = dt;
            // Consume stage boundaries exactly, independent of rendering frame rate.
            for _ in 0..4 {
                let (duration, end, next) = match self.stage {
                    Stage::Attack => (r.envelope.attack / 1000.0, 1.0, Stage::Decay),
                    Stage::Decay => (
                        r.envelope.decay / 1000.0,
                        r.envelope.sustain,
                        Stage::Sustain,
                    ),
                    Stage::Release => (r.envelope.release / 1000.0, 0.0, Stage::Idle),
                    Stage::Sustain => {
                        self.value = r.envelope.sustain;
                        break;
                    }
                    Stage::Idle => {
                        self.value = 0.0;
                        break;
                    }
                };
                let step = remaining.min((duration - self.elapsed).max(0.0));
                self.elapsed += step;
                remaining -= step;
                self.value =
                    self.start + (end - self.start) * (self.elapsed / duration).clamp(0.0, 1.0);
                if self.elapsed + 1e-6 >= duration {
                    self.value = end;
                    self.stage(next);
                } else {
                    break;
                }
                if remaining <= 0.0 {
                    break;
                }
            }
        }
        self.previous = input;
        self.slow += (input - self.slow) * (1.0 - (-dt / 0.15).exp());
        self.output = (r.output_min + self.value * (r.output_max - r.output_min)) * r.amount * sign;
    }
}
struct RouteState {
    id: u64,
    signature: (Source, Target, Response, u32, u32, usize),
    voices: Vec<Voice>,
}
#[derive(Default)]
pub struct Modulator {
    states: Vec<RouteState>,
    epoch: Option<(u64, u32, usize)>,
    outputs: [f32; 10],
    layers: [[f32; MAX_LAYERS]; 3],
    selected: Option<u64>,
    envelope_span: Option<f32>,
}
impl Modulator {
    pub fn select_target(&mut self, target: Target, settings: &ModulationSettings) {
        self.selected = settings
            .routes
            .iter()
            .find(|r| r.target == target)
            .map(|r| r.id);
    }
    pub fn input_for(&self, target: Target, settings: &ModulationSettings) -> f32 {
        settings
            .routes
            .iter()
            .filter(|r| r.enabled && r.target == target)
            .filter_map(|r| {
                self.states.iter().find(|s| s.id == r.id).map(|s| {
                    s.voices
                        .iter()
                        .map(|v| v.input * r.amount.abs())
                        .fold(0.0, f32::max)
                })
            })
            .fold(0.0, f32::max)
            .mul_add(settings.amount, 0.0)
            .clamp(0.0, 4.0)
    }
    pub fn value(&self, target: Target) -> f32 {
        self.outputs[target as usize]
    }
    pub fn layer(&self, target: Target, i: usize) -> f32 {
        self.layers[target as usize - Target::LayerBend as usize][i.min(MAX_LAYERS - 1)]
    }
    pub fn advance(
        &mut self,
        frame: &AnalysisFrame,
        live: bool,
        settings: &ModulationSettings,
        dt: f32,
        layers: usize,
    ) {
        if dt <= 0.0 {
            return;
        }
        let epoch = (frame.capture_epoch, frame.sample_rate, frame.fft_size);
        if self.epoch != Some(epoch) {
            self.states.clear();
            self.epoch = Some(epoch);
        }
        let fallback;
        let features = if let Some(features) = &frame.features {
            features.as_ref()
        } else {
            fallback = AudioFeatures::new(&frame.bins, frame.mix_rms, frame.rms);
            &fallback
        };
        self.outputs = [0.0; 10];
        self.layers = [[0.0; MAX_LAYERS]; 3];
        self.states
            .retain(|s| settings.routes.iter().any(|r| r.enabled && r.id == s.id));
        for r in settings
            .routes
            .iter()
            .filter(|r| r.enabled)
            .take(MAX_ROUTES)
        {
            let count = if r.target.layered() && r.layer == 0 {
                layers.clamp(1, MAX_LAYERS)
            } else {
                1
            };
            let signature = (
                r.source,
                r.target,
                r.response,
                r.low.to_bits(),
                r.high.to_bits(),
                r.layer,
            );
            let index = self
                .states
                .iter()
                .position(|s| s.id == r.id)
                .unwrap_or_else(|| {
                    self.states.push(RouteState {
                        id: r.id,
                        signature,
                        voices: vec![
                            Voice {
                                peak: -42.0,
                                ..Voice::default()
                            };
                            count
                        ],
                    });
                    self.states.len() - 1
                });
            let state = &mut self.states[index];
            if state.signature != signature || state.voices.len() != count {
                state.signature = signature;
                state.voices = vec![
                    Voice {
                        peak: -42.0,
                        ..Voice::default()
                    };
                    count
                ];
            }
            for (i, voice) in state.voices.iter_mut().enumerate() {
                let (low, high) = band_range(r.low, r.high, i, count);
                let amplitude = if !live {
                    0.0
                } else {
                    match r.source {
                        Source::Band => features.band(low, high),
                        Source::FullMix => features.rms,
                        Source::Stereo => features.rms * features.balance.abs(),
                    }
                };
                let sign = if r.source == Source::Stereo {
                    features.balance.signum()
                } else {
                    1.0
                };
                voice.advance(amplitude, sign, r, settings.adaptive, dt.min(0.1));
                let output = voice.output * settings.amount;
                if r.target.layered() {
                    let layer = if r.layer == 0 {
                        i
                    } else {
                        r.layer.saturating_sub(1)
                    };
                    if layer < layers.min(MAX_LAYERS) {
                        self.layers[r.target as usize - Target::LayerBend as usize][layer] +=
                            output;
                    }
                } else {
                    self.outputs[r.target as usize] += output;
                }
            }
        }
        for v in self
            .outputs
            .iter_mut()
            .chain(self.layers.iter_mut().flatten())
        {
            *v = v.clamp(-4.0, 4.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(hz: f32, magnitude: f32) -> AnalysisFrame {
        AnalysisFrame {
            bins: vec![FrequencyBin {
                frequency_hz: hz,
                magnitude,
                dbfs: -10.0,
            }],
            mix_rms: magnitude,
            rms: [magnitude; 2],
            ..Default::default()
        }
    }
    fn one_route(target: Target) -> ModulationSettings {
        ModulationSettings {
            routes: vec![Route {
                target,
                ..Route::default()
            }],
            ..Default::default()
        }
    }
    fn settle(m: &mut Modulator, f: &AnalysisFrame, s: &ModulationSettings) {
        for _ in 0..240 {
            m.advance(f, true, s, 1.0 / 120.0, 7);
        }
    }
    #[test]
    fn exact_bands_reject_invalid_bins_and_keep_quiet_highs_after_loud_bass() {
        let bins = [
            FrequencyBin {
                frequency_hz: 80.0,
                magnitude: 1.0,
                dbfs: 0.0,
            },
            FrequencyBin {
                frequency_hz: 250.0,
                magnitude: 0.5,
                dbfs: 0.0,
            },
            FrequencyBin {
                frequency_hz: 8000.0,
                magnitude: 1e-4,
                dbfs: 0.0,
            },
            FrequencyBin {
                frequency_hz: f32::NAN,
                magnitude: 1.0,
                dbfs: 0.0,
            },
            FrequencyBin {
                frequency_hz: 9000.0,
                magnitude: f32::INFINITY,
                dbfs: 0.0,
            },
        ];
        let f = AudioFeatures::new(&bins, 0.5, [0.5; 2]);
        assert_eq!(f.band(20.0, 250.0), 1.0);
        assert_eq!(f.band(250.0, 4000.0), 0.5);
        assert!((f.band(4000.0, 20000.0) - 1e-4).abs() < 1e-7);
        assert_eq!(f.band(100.0, 200.0), 0.0);
    }
    #[test]
    fn routing_changes_only_its_destination_and_caches_analysis_features() {
        let mut s = one_route(Target::Brightness);
        let mut m = Modulator::default();
        let mut f = tone(80.0, 0.3);
        f.features = Some(std::sync::Arc::new(AudioFeatures::new(
            &f.bins, f.mix_rms, f.rms,
        )));
        f.bins.clear(); // cached snapshot supplies identical controls without another spectrum scan
        settle(&mut m, &f, &s);
        assert!(m.value(Target::Brightness) > 0.8);
        for t in [
            Target::Expansion,
            Target::Motion,
            Target::Swirl,
            Target::Size,
        ] {
            assert_eq!(m.value(t), 0.0);
        }
        s.routes[0].target = Target::Expansion;
        settle(&mut m, &f, &s);
        assert_eq!(m.value(Target::Brightness), 0.0);
        assert!(m.value(Target::Expansion) > 0.8);
        s.routes[0].enabled = false;
        m.advance(&f, true, &s, 0.01, 7);
        assert_eq!(m.value(Target::Expansion), 0.0);
        assert!(m.states.is_empty());
    }
    #[test]
    fn follower_is_frame_rate_independent_and_silence_is_not_auto_amplified() {
        let mut s = one_route(Target::Brightness);
        s.adaptive = true;
        let mut fast = Modulator::default();
        let mut slow = Modulator::default();
        let frame = tone(80.0, 0.01);
        for _ in 0..120 {
            fast.advance(&frame, true, &s, 1.0 / 120.0, 7);
        }
        for _ in 0..30 {
            slow.advance(&frame, true, &s, 1.0 / 30.0, 7);
        }
        assert!((fast.value(Target::Brightness) - slow.value(Target::Brightness)).abs() < 1e-5);
        assert!(fast.value(Target::Brightness) > 0.9);
        for _ in 0..1200 {
            fast.advance(&AnalysisFrame::default(), true, &s, 1.0 / 120.0, 7);
        }
        assert!(fast.value(Target::Brightness) < 1e-5);
        let mut silent = Modulator::default();
        settle(&mut silent, &tone(80.0, 1e-8), &s);
        assert_eq!(silent.value(Target::Brightness), 0.0);
    }
    #[test]
    fn gated_adsr_visits_each_stage_and_releases_from_current_level() {
        let mut r = Route {
            response: Response::Gate,
            envelope: Envelope {
                attack: 100.0,
                decay: 200.0,
                sustain: 0.4,
                release: 300.0,
            },
            ..Route::default()
        };
        let mut v = Voice::default();
        v.advance(0.5, 1.0, &r, false, 0.05);
        assert!((v.value - 0.5).abs() < 1e-5);
        v.advance(0.5, 1.0, &r, false, 0.05);
        assert_eq!(v.stage, Stage::Decay);
        assert_eq!(v.value, 1.0);
        v.advance(0.5, 1.0, &r, false, 0.2);
        assert_eq!(v.stage, Stage::Sustain);
        assert!((v.value - 0.4).abs() < 1e-5);
        v.advance(0.0, 1.0, &r, false, 0.15);
        assert_eq!(v.stage, Stage::Release);
        assert!((v.value - 0.2).abs() < 1e-5);
        v.advance(0.0, 1.0, &r, false, 0.15);
        assert_eq!(v.stage, Stage::Idle);
        assert_eq!(v.value, 0.0);
        r.envelope.attack = 1000.0;
        v.advance(0.5, 1.0, &r, false, 0.1);
        v.advance(0.0, 1.0, &r, false, 0.15);
        assert!((v.value - 0.05).abs() < 1e-5);
    }
    #[test]
    fn transient_pulses_retrigger_but_sustained_audio_does_not() {
        let r = Route {
            response: Response::Transient,
            envelope: Envelope {
                attack: 10.0,
                decay: 20.0,
                sustain: 0.5,
                release: 100.0,
            },
            hold_ms: 50.0,
            ..Route::default()
        };
        let mut v = Voice::default();
        v.advance(0.5, 1.0, &r, false, 0.01);
        assert_eq!(v.value, 1.0);
        for _ in 0..100 {
            v.advance(0.5, 1.0, &r, false, 0.01);
        }
        assert_eq!(v.value, 0.0);
        for _ in 0..100 {
            v.advance(0.0, 1.0, &r, false, 0.01);
        }
        v.advance(0.5, 1.0, &r, false, 0.01);
        assert_eq!(v.value, 1.0);
    }
    #[test]
    fn each_layer_gets_its_own_band_envelope_and_count_changes_clear_old_voices() {
        let mut s = one_route(Target::LayerBend);
        s.routes[0].high = 20000.0;
        for count in 2..=12 {
            for layer in 0..count {
                let (a, b) = band_range(20.0, 20000.0, layer, count);
                let f = tone((a * b).sqrt(), 0.5);
                let mut m = Modulator::default();
                for _ in 0..120 {
                    m.advance(&f, true, &s, 1.0 / 120.0, count);
                }
                for i in 0..count {
                    assert_eq!(m.layer(Target::LayerBend, i) > 0.9, i == layer);
                }
            }
        }
        let mut m = Modulator::default();
        settle(&mut m, &tone(80.0, 0.5), &s);
        assert!(m.layer(Target::LayerBend, 1) > 0.9);
        m.advance(&tone(80.0, 0.5), true, &s, 0.01, 12);
        assert_eq!(m.layer(Target::LayerBend, 1), 0.0);
        assert!(m.layer(Target::LayerBend, 2) > 0.0);
        s.routes[0].layer = 4;
        settle(&mut m, &tone(80.0, 0.5), &s);
        assert!(m.layer(Target::LayerBend, 3) > 0.9);
        assert_eq!(m.layer(Target::LayerBend, 2), 0.0);
    }
    #[test]
    fn source_changes_reset_envelopes_and_route_ids_survive_deletion() {
        let mut s = one_route(Target::Brightness);
        let mut m = Modulator::default();
        let mut f = tone(80.0, 0.5);
        settle(&mut m, &f, &s);
        assert!(m.value(Target::Brightness) > 0.9);
        f.capture_epoch += 1;
        f.bins.clear();
        f.mix_rms = 0.0;
        m.advance(&f, true, &s, 0.01, 7);
        assert_eq!(m.value(Target::Brightness), 0.0);
        s.routes.push(Route {
            id: 2,
            target: Target::Size,
            ..Route::default()
        });
        settle(&mut m, &tone(80.0, 0.5), &s);
        s.routes.remove(0);
        m.advance(&tone(80.0, 0.5), true, &s, 0.01, 7);
        assert_eq!(m.value(Target::Brightness), 0.0);
        assert!(m.value(Target::Size) > 0.9);
        s.routes[0].low = 4000.0;
        s.routes[0].high = 20000.0;
        m.advance(&tone(80.0, 0.5), true, &s, 0.01, 7);
        assert_eq!(m.value(Target::Size), 0.0);
    }
    #[test]
    fn amounts_bounds_and_serialization_do_not_include_runtime() {
        let mut s = one_route(Target::Size);
        let mut m = Modulator::default();
        s.routes[0].amount = -2.0;
        s.routes[0].output_min = 0.2;
        s.routes[0].output_max = 0.8;
        settle(&mut m, &tone(80.0, 1.0), &s);
        assert!((m.value(Target::Size) + 1.6).abs() < 1e-4);
        let json = serde_json::to_string(&s).unwrap();
        let saved: ModulationSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(saved, s);
        assert!(!json.contains("voices") && !json.contains("spectrum"));
        s.routes[0].low = f32::NAN;
        s.routes[0].high = -1.0;
        s.routes[0].envelope.attack = f32::INFINITY;
        s.routes.push(s.routes[0].clone());
        s.sanitize(Visualizer::Flux);
        assert!(s.routes[0].low.is_finite() && s.routes[0].high > s.routes[0].low);
        assert_ne!(s.routes[0].id, s.routes[1].id);
        assert!(s.routes[0].envelope.attack.is_finite());
    }
    #[test]
    #[ignore = "manual cached-feature timing probe"]
    fn feature_cache_timing() {
        for fft_size in [8192, 16384] {
            let bins: Vec<_> = (0..=fft_size / 2)
                .map(|i| FrequencyBin {
                    frequency_hz: i as f32 * 48_000.0 / fft_size as f32,
                    magnitude: 0.01 + (i as f32 * 0.1).sin().abs() * 0.01,
                    dbfs: -40.0,
                })
                .collect();
            let start = std::time::Instant::now();
            for _ in 0..1000 {
                std::hint::black_box(AudioFeatures::new(
                    std::hint::black_box(&bins),
                    0.1,
                    [0.1, 0.09],
                ));
            }
            let build = start.elapsed().as_secs_f64() * 1000.0 / 1000.0;
            let features = AudioFeatures::new(&bins, 0.1, [0.1, 0.09]);
            let start = std::time::Instant::now();
            for _ in 0..1000 {
                for i in 0..24 {
                    let (low, high) = band_range(20.0, 20_000.0, i, 24);
                    std::hint::black_box(features.band(low, high));
                }
            }
            eprintln!(
                "AudioFeatures FFT {fft_size}: {build:.3} ms/build; 24 cached band queries {:.3} us",
                start.elapsed().as_secs_f64() * 1_000_000.0 / 1000.0
            );
        }
    }
}
