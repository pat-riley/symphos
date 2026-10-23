//! Typed, scene-owned insert chains. All DSP and detector clocks live on the Signals worker.
use super::*;
use std::collections::VecDeque;

pub const MAX_INSERTS: usize = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Data {
    Audio,
    Control,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Detector {
    Level,
    Band,
    Bass,
    Hits,
    Change,
    Balance,
    Beat,
}
impl Detector {
    pub fn label(self) -> &'static str {
        match self {
            Self::Level => "Overall intensity",
            Self::Band => "Band energy",
            Self::Bass => "Bass energy",
            Self::Hits => "Bass hits",
            Self::Change => "Spectral change",
            Self::Balance => "Stereo balance",
            Self::Beat => "Beat detection",
        }
    }
    pub fn method(self) -> Method {
        match self {
            Self::Level => Method::Intensity,
            Self::Band | Self::Bass => Method::Band,
            Self::Hits | Self::Beat => Method::BassHits,
            Self::Change => Method::SpectralChange,
            Self::Balance => Method::Balance,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Detection {
    pub detector: Detector,
    pub response: Route,
    pub adaptive: bool,
}
impl Detection {
    pub fn new(detector: Detector) -> Self {
        let mut d = Definition::recipe(1, detector.method());
        if matches!(detector, Detector::Bass | Detector::Beat) {
            d.response.low = 30.0;
            d.response.high = 180.0;
        }
        if !matches!(detector, Detector::Hits | Detector::Beat) {
            d.response.curve = 1.0;
            d.response.envelope.attack = 15.0;
            d.response.envelope.release = 180.0;
        }
        Self {
            detector,
            response: d.response,
            adaptive: true,
        }
    }
    fn valid(&self) -> bool {
        let mut d = Definition::recipe(1, self.detector.method());
        d.response = self.response.clone();
        d.adaptive = self.adaptive;
        let bank = Bank {
            next_id: 2,
            channels: vec![d],
            ..Default::default()
        };
        bank.valid()
            && self.response.amount == 1.0
            && self.response.output_min == 0.0
            && self.response.output_max == 1.0
            && self.response.signal.is_none()
            && !self.response.invert
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeatOutput {
    Pulse,
    Phase,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    AudioGain {
        db: f32,
    },
    Filter {
        low: f32,
        high: f32,
    },
    HighPass {
        hz: f32,
    },
    LowPass {
        hz: f32,
    },
    AudioGate {
        db: f32,
    },
    Channels {
        channels: audio::Channels,
    },
    Detect(Detection),
    Beat {
        sensitivity: f32,
        manual: bool,
        bpm: f32,
        output: BeatOutput,
    },
    Gain {
        gain: f32,
    },
    Normalize {
        low: f32,
        high: f32,
        automatic: bool,
    },
    Smooth {
        ms: f32,
    },
    Envelope {
        attack: f32,
        release: f32,
    },
    Threshold {
        threshold: f32,
        hysteresis: f32,
    },
    Shape(shaping::Shaping),
    Invert,
    Quantize {
        steps: u32,
    },
}
impl Effect {
    pub fn label(&self) -> &'static str {
        match self {
            Self::AudioGain { .. } => "Audio gain",
            Self::Filter { .. } => "Frequency filter",
            Self::HighPass { .. } => "High-pass",
            Self::LowPass { .. } => "Low-pass",
            Self::AudioGate { .. } => "Audio gate",
            Self::Channels { .. } => "Audio channels",
            Self::Detect(d) => d.detector.label(),
            Self::Beat { .. } => "Beat detection",
            Self::Gain { .. } => "Control gain",
            Self::Normalize { .. } => "Normalize",
            Self::Smooth { .. } => "Smooth",
            Self::Envelope { .. } => "Envelope",
            Self::Threshold { .. } => "Threshold",
            Self::Shape(_) => "Curve",
            Self::Invert => "Invert",
            Self::Quantize { .. } => "Steps",
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
            Self::AudioGain { .. } => {
                "Scale the audio waveform before detection. The spectrum and measurements are recalculated from the same audio. This does not change speaker volume."
            }
            Self::Filter { .. } | Self::HighPass { .. } | Self::LowPass { .. } => {
                "Second-order Butterworth filters remove unwanted frequencies before the next insert. Low and high cutoffs define a pass band. The visualizer audio feed follows these filters."
            }
            Self::AudioGate { .. } => {
                "Silence audio whose windowed RMS falls below the threshold. Useful for rejecting background noise before detection; this is a hard gate."
            }
            Self::Channels { .. } => {
                "Choose stereo, left, right, mid or side before other audio inserts. Mid cancels opposite-polarity material; side isolates differences between channels."
            }
            Self::Detect(d) => match d.detector {
                Detector::Level => {
                    "Convert full-mix RMS into a stable 0–1 curve. Attack follows rising energy and release lets it fall smoothly. Sensitivity boosts quiet material; adaptive scaling follows recent level."
                }
                Detector::Band | Detector::Bass => {
                    "Measure summed FFT power within the selected frequency range, then follow its energy with a smooth envelope. Bass energy starts at 30–180 Hz. This measures low-frequency energy, not an isolated instrument."
                }
                Detector::Hits => {
                    "Detect sudden rises in bass energy using a slow baseline, threshold and retrigger protection. Produces hit events and an envelope; sustained bass is not a continuous stream of hits."
                }
                Detector::Change => {
                    "Measure positive spectral flux: new energy across 96 logarithmic bands per second. Attacks and changing textures respond strongly; a steady tone settles toward zero."
                }
                Detector::Balance => {
                    "Convert relative left/right RMS into a signed control curve: left is negative, right positive. Quiet audio is gated so silence cannot create a misleading balance."
                }
                Detector::Beat => "Estimate rhythmic onsets and timing from bass transients.",
            },
            Self::Beat { .. } => {
                "Detect low-frequency onsets with adaptive flux and a refractory interval. Tempo is the median of recent consistent beat intervals; confidence stays low until enough evidence exists. Pulse follows detected hits; phase predicts between confident beats. Manual mode provides a stable BPM clock, not generated audio."
            }
            Self::Gain { .. } => {
                "Multiply the incoming control curve. Unlike audio gain this changes only modulation, leaving the channel's waveform and spectrum intact."
            }
            Self::Normalize { .. } => {
                "Map an input range onto 0–1 and clamp outside it. For example, 0.50–0.55 can fill the full output range. Auto follows slowly relaxing observed bounds; disable Auto to enter exact limits."
            }
            Self::Smooth { .. } => {
                "A time-based one-pole low-pass filter softens rapid changes. The amount is a time constant in milliseconds; zero bypasses it. Its response is independent of display FPS."
            }
            Self::Envelope { .. } => {
                "Follow a curve with separate exponential rise and fall times. A fast attack and slower release preserve hits while reducing flicker. Signed inputs retain their direction."
            }
            Self::Threshold { .. } => {
                "Turn a curve into an on/off gate. Hysteresis keeps it from chattering near the threshold: it turns off only after falling below the lower boundary."
            }
            Self::Shape(_) => {
                "Bend a control curve without changing its timing. Curve adjusts sensitivity, dead zone suppresses small values, and steps produces discrete levels. Signed curves retain their center."
            }
            Self::Invert => {
                "Reverse the control curve: 0–1 becomes 1–0; a signed curve changes polarity. Audio remains unchanged."
            }
            Self::Quantize { .. } => {
                "Round a control value into equally spaced levels. Useful for MIDI knobs, stepped animation and deliberate rhythmic jumps. This operates on decoded control values, not MIDI messages."
            }
        }
    }
    pub fn input(&self) -> Data {
        if matches!(
            self,
            Self::AudioGain { .. }
                | Self::Filter { .. }
                | Self::HighPass { .. }
                | Self::LowPass { .. }
                | Self::AudioGate { .. }
                | Self::Channels { .. }
                | Self::Detect(_)
                | Self::Beat { .. }
        ) {
            Data::Audio
        } else {
            Data::Control
        }
    }
    pub fn output(&self) -> Data {
        if matches!(self, Self::Detect(_) | Self::Beat { .. }) {
            Data::Control
        } else {
            self.input()
        }
    }
    pub fn valid(&self) -> bool {
        let range = |v: f32, a, b| v.is_finite() && (a..=b).contains(&v);
        match self {
            Self::AudioGain { db } => range(*db, -48.0, 36.0),
            Self::Filter { low, high } => {
                range(*low, 20.0, 19999.0) && range(*high, 21.0, 20000.0) && low < high
            }
            Self::HighPass { hz } | Self::LowPass { hz } => range(*hz, 20.0, 20000.0),
            Self::AudioGate { db } => range(*db, -100.0, 0.0),
            Self::Channels { .. } | Self::Invert => true,
            Self::Detect(d) => d.valid(),
            Self::Beat {
                sensitivity, bpm, ..
            } => range(*sensitivity, 0.5, 4.0) && range(*bpm, 40.0, 240.0),
            Self::Gain { gain } => range(*gain, 0.0, 8.0),
            Self::Normalize { low, high, .. } => {
                range(*low, -8.0, 8.0) && range(*high, -8.0, 8.0) && high - low >= 0.00001
            }
            Self::Smooth { ms } => range(*ms, 0.0, 5000.0),
            Self::Envelope { attack, release } => {
                range(*attack, 1.0, 2000.0) && range(*release, 1.0, 5000.0)
            }
            Self::Threshold {
                threshold,
                hysteresis,
            } => range(*threshold, -1.0, 1.0) && range(*hysteresis, 0.0, 0.5),
            Self::Shape(s) => s.valid(),
            Self::Quantize { steps } => (2..=128).contains(steps),
        }
    }
    pub fn choices(data: Data) -> Vec<Self> {
        match data {
            Data::Audio => vec![
                Self::AudioGain { db: 0.0 },
                Self::Filter {
                    low: 30.0,
                    high: 180.0,
                },
                Self::HighPass { hz: 80.0 },
                Self::LowPass { hz: 4000.0 },
                Self::AudioGate { db: -60.0 },
                Self::Channels {
                    channels: audio::Channels::Stereo,
                },
                Self::Detect(Detection::new(Detector::Level)),
                Self::Detect(Detection::new(Detector::Bass)),
                Self::Detect(Detection::new(Detector::Band)),
                Self::Detect(Detection::new(Detector::Hits)),
                Self::Detect(Detection::new(Detector::Change)),
                Self::Detect(Detection::new(Detector::Balance)),
                Self::Beat {
                    sensitivity: 1.5,
                    manual: false,
                    bpm: 120.0,
                    output: BeatOutput::Pulse,
                },
            ],
            Data::Control => vec![
                Self::Normalize {
                    low: 0.0,
                    high: 1.0,
                    automatic: false,
                },
                Self::Envelope {
                    attack: 15.0,
                    release: 200.0,
                },
                Self::Smooth { ms: 100.0 },
                Self::Gain { gain: 1.0 },
                Self::Threshold {
                    threshold: 0.5,
                    hysteresis: 0.05,
                },
                Self::Shape(shaping::Shaping {
                    enabled: true,
                    ..Default::default()
                }),
                Self::Invert,
                Self::Quantize { steps: 8 },
            ],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Insert {
    pub id: u64,
    pub enabled: bool,
    pub effect: Effect,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chain {
    pub stages: Vec<Insert>,
    pub next_id: u64,
}
impl Default for Chain {
    fn default() -> Self {
        Self {
            stages: Vec::new(),
            next_id: 1,
        }
    }
}
impl Chain {
    pub fn data_at(&self, source: Input, index: usize) -> Data {
        self.stages.iter().take(index).fold(
            if source == Input::System {
                Data::Audio
            } else {
                Data::Control
            },
            |_, s| s.effect.output(),
        )
    }
    pub fn valid(&self, source: Input) -> bool {
        let mut data = if source == Input::System {
            Data::Audio
        } else {
            Data::Control
        };
        let mut ids = std::collections::HashSet::new();
        self.stages.len() <= MAX_INSERTS
            && self.next_id > 0
            && self.stages.iter().all(|s| {
                let valid = s.id > 0
                    && s.id < self.next_id
                    && ids.insert(s.id)
                    && s.effect.valid()
                    && s.effect.input() == data;
                data = s.effect.output();
                valid
            })
    }
    pub fn add(&mut self, source: Input, index: usize, effect: Effect) -> Option<u64> {
        if self.stages.len() >= MAX_INSERTS || index > self.stages.len() || self.next_id == u64::MAX
        {
            return None;
        }
        let id = self.next_id;
        self.stages.insert(
            index,
            Insert {
                id,
                enabled: true,
                effect,
            },
        );
        self.next_id += 1;
        if !self.valid(source) {
            self.stages.remove(index);
            self.next_id -= 1;
            None
        } else {
            Some(id)
        }
    }
    pub fn move_to(&mut self, source: Input, id: u64, boundary: usize) -> bool {
        let Some(from) = self.stages.iter().position(|s| s.id == id) else {
            return false;
        };
        if boundary > self.stages.len() {
            return false;
        }
        let to = boundary - usize::from(from < boundary);
        let stage = self.stages.remove(from);
        self.stages.insert(to, stage);
        if self.valid(source) {
            true
        } else {
            let stage = self.stages.remove(to);
            self.stages.insert(from, stage);
            false
        }
    }
    pub fn remove(&mut self, id: u64) {
        if let Some(index) = self.stages.iter().position(|s| s.id == id) {
            if self.stages[index].effect.output() != self.stages[index].effect.input() {
                self.stages.truncate(index);
            } else {
                self.stages.remove(index);
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Point {
    pub time: f64,
    pub input: f32,
    pub output: f32,
}
#[derive(Clone, Debug, Default)]
pub struct Monitor {
    pub id: u64,
    pub input: f32,
    pub output: f32,
    pub bpm: f32,
    pub phase: f32,
    pub confidence: f32,
    pub history: Arc<[Point]>,
}
#[derive(Default)]
struct StageState {
    voice: Voice,
    previous: Option<[f32; 96]>,
    spectral: f32,
    detector_signature: Option<(Method, Response, u32, u32)>,
    smoother: OutputSmoother,
    gate: bool,
    bounds: Option<(f32, f32)>,
    audio: audio::Inputs,
    history: VecDeque<Point>,
    monitor: Monitor,
    beat: BeatState,
    input_revision: Option<u64>,
}
#[derive(Default)]
struct BeatState {
    previous: f32,
    baseline: f32,
    last_hit: Option<f64>,
    intervals: VecDeque<f32>,
    period: f32,
    confidence: f32,
    pulse: f32,
    phase: f32,
    manual_beat: i64,
    last_analysis: Option<f64>,
}
impl BeatState {
    #[allow(clippy::too_many_arguments)]
    fn step(
        &mut self,
        energy: f32,
        fresh: bool,
        dt: f32,
        time: f64,
        sensitivity: f32,
        manual: bool,
        bpm: f32,
    ) -> (bool, f32, f32, f32) {
        let mut hit = false;
        if manual {
            self.period = 60.0 / bpm;
            self.confidence = 1.0;
            self.phase = (time / self.period as f64).rem_euclid(1.0) as f32;
            let beat = (time / self.period as f64).floor() as i64;
            hit = beat != self.manual_beat;
            self.manual_beat = beat;
        } else {
            if fresh {
                let novelty = (energy - self.previous).max(0.0);
                self.previous = energy;
                let threshold = (self.baseline * sensitivity).max(0.002);
                if novelty > threshold
                    && energy > 0.004
                    && self.last_hit.is_none_or(|t| time - t > 0.24)
                {
                    hit = true;
                    if let Some(last) = self.last_hit {
                        let interval = (time - last) as f32;
                        if (0.25..=1.5).contains(&interval) {
                            self.intervals.push_back(interval);
                            while self.intervals.len() > 8 {
                                self.intervals.pop_front();
                            }
                        } else {
                            self.intervals.clear();
                        }
                    }
                    self.last_hit = Some(time);
                    if self.intervals.len() >= 3 {
                        let mut periods: Vec<_> = self.intervals.iter().copied().collect();
                        periods.sort_by(f32::total_cmp);
                        self.period = periods[periods.len() / 2];
                        let agreement = periods
                            .iter()
                            .filter(|p| (**p - self.period).abs() < self.period * 0.15)
                            .count() as f32
                            / periods.len() as f32;
                        self.confidence = if agreement >= 0.7 { agreement } else { 0.0 };
                    }
                }
                let analysis_dt = self
                    .last_analysis
                    .map_or(dt, |last| (time - last).clamp(0.0, 0.1) as f32);
                self.last_analysis = Some(time);
                self.baseline += (novelty - self.baseline) * (1.0 - (-analysis_dt / 0.25).exp());
            }
            let age = self.last_hit.map_or(f64::INFINITY, |t| time - t);
            if self.period > 0.0 && age < (self.period * 2.5) as f64 && self.confidence > 0.0 {
                self.phase = (age / self.period as f64).rem_euclid(1.0) as f32;
            } else {
                self.phase = 0.0;
                self.confidence = 0.0;
            }
        }
        self.pulse = if hit {
            1.0
        } else {
            self.pulse * (-dt / 0.09).exp()
        };
        (
            hit,
            self.pulse,
            self.phase,
            if self.period > 0.0 {
                60.0 / self.period
            } else {
                0.0
            },
        )
    }
}
#[derive(Default)]
pub struct Runtime {
    states: BTreeMap<u64, StageState>,
    order: Vec<u64>,
    epoch: Option<(u64, u32, usize)>,
    source: Option<(audio::Connection, u64)>,
    audio_signature: Vec<Insert>,
    audio_cache: Option<Arc<audio::Feed>>,
    silent_audio: audio::Inputs,
}
pub struct Result {
    pub sample: Sample,
    pub audio: Option<Arc<audio::Feed>>,
    pub monitors: Vec<Monitor>,
}
impl Runtime {
    #[allow(clippy::too_many_arguments)]
    pub fn process(
        &mut self,
        d: &Definition,
        feed: Option<Arc<audio::Feed>>,
        source: &AnalysisFrame,
        base: &Sample,
        time: f64,
        dt: f32,
        fresh: bool,
        frame_dt: f32,
        next_event: &mut u64,
    ) -> Result {
        let chain = d.inserts.as_ref().unwrap();
        let order: Vec<_> = chain.stages.iter().map(|s| s.id).collect();
        let epoch = if d.input == Input::System {
            (source.capture_epoch, source.sample_rate, source.fft_size)
        } else {
            (0, 0, 0)
        };
        let source_key = (d.audio_input, feed.as_ref().map_or(0, |f| f.revision));
        let source_changed = self.epoch != Some(epoch) || self.source != Some(source_key);
        if self.order != order || source_changed {
            self.states.clear();
            self.order = order;
            self.epoch = Some(epoch);
            self.source = Some(source_key);
        }
        let signature: Vec<_> = chain
            .stages
            .iter()
            .take_while(|s| s.effect.output() == Data::Audio)
            .cloned()
            .map(|mut insert| {
                // Keep history while adjusting gain; topology/bypass and other
                // audio-processing changes still invalidate incompatible rows.
                if let Effect::AudioGain { db } = &mut insert.effect {
                    *db = 0.0;
                }
                insert
            })
            .collect();
        if source_changed || self.audio_signature != signature {
            self.audio_cache = None;
            self.audio_signature = signature;
        }
        let mut audio = feed;
        let mut value = base.raw;
        let mut signed = d.input == Input::Lfo && d.lfo.bipolar
            || d.input == Input::Midi && d.midi.message == crate::midi::Message::PitchBend;
        let mut data = if d.input == Input::System {
            Data::Audio
        } else {
            Data::Control
        };
        let mut control_available = d.input != Input::System && base.status == Status::Live;
        let mut event = base.event;
        let mut raw = value;
        let mut monitors = Vec::new();
        for insert in &chain.stages {
            let state = self.states.entry(insert.id).or_default();
            if base.status != Status::Live || (data == Data::Control && !control_available) {
                state.voice = Voice::default();
                state.smoother = OutputSmoother::default();
                state.previous = None;
                state.beat = BeatState::default();
            }
            let frame = audio.as_ref().map_or(source, |f| f.frame.as_ref());
            let input = if data == Data::Audio {
                if base.status == Status::Live {
                    frame.mix_rms
                } else {
                    0.0
                }
            } else {
                value
            };
            if insert.enabled
                && base.status == Status::Live
                && (data == Data::Audio || control_available)
            {
                match &insert.effect {
                    Effect::AudioGain { .. }
                    | Effect::Filter { .. }
                    | Effect::HighPass { .. }
                    | Effect::LowPass { .. }
                    | Effect::AudioGate { .. }
                    | Effect::Channels { .. } => {
                        let mut bus = audio::InputBus {
                            id: insert.id,
                            ..Default::default()
                        };
                        match &insert.effect {
                            Effect::AudioGain { db } => bus.gain_db = *db,
                            Effect::Filter { low, high } => {
                                bus.high_pass = true;
                                bus.low_hz = *low;
                                bus.low_pass = true;
                                bus.high_hz = *high;
                            }
                            Effect::HighPass { hz } => {
                                bus.high_pass = true;
                                bus.low_hz = *hz;
                            }
                            Effect::LowPass { hz } => {
                                bus.low_pass = true;
                                bus.high_hz = *hz;
                            }
                            Effect::AudioGate { db } => {
                                bus.gate = true;
                                bus.gate_db = *db;
                            }
                            Effect::Channels { channels } => bus.channels = *channels,
                            _ => {}
                        }
                        // Upstream insert edits can change audio without advancing capture sequence.
                        let input_revision = audio.as_ref().map_or(0, |f| f.revision);
                        if state.input_revision != Some(input_revision) {
                            state.audio.clear();
                            state.input_revision = Some(input_revision);
                        }
                        audio = state.audio.process(&[bus], frame).remove(&insert.id);
                    }
                    Effect::Detect(config) => {
                        control_available = true;
                        let sig = (
                            config.detector.method(),
                            config.response.response,
                            config.response.low.to_bits(),
                            config.response.high.to_bits(),
                        );
                        if state.detector_signature != Some(sig) {
                            state.voice = Voice::default();
                            state.previous = None;
                            state.detector_signature = Some(sig);
                        }
                        let fallback;
                        let features = if let Some(f) = frame.features.as_deref() {
                            f
                        } else {
                            fallback = AudioFeatures::new(&frame.bins, frame.mix_rms, frame.rms);
                            &fallback
                        };
                        if fresh && config.detector == Detector::Change {
                            state.spectral = state.previous.map_or(0.0, |old| {
                                features
                                    .spectrum
                                    .iter()
                                    .zip(old)
                                    .map(|(a, b)| (a - b).max(0.0))
                                    .sum::<f32>()
                                    / 96.0
                                    / frame_dt
                            });
                            state.previous = Some(features.spectrum);
                        }
                        let amplitude = match config.detector {
                            Detector::Level => features.rms,
                            Detector::Band | Detector::Bass | Detector::Hits | Detector::Beat => {
                                features.band(config.response.low, config.response.high)
                            }
                            Detector::Change => state.spectral,
                            Detector::Balance => features.rms * features.balance.abs(),
                        };
                        let sign = if config.detector == Detector::Balance {
                            features.balance.signum()
                        } else {
                            1.0
                        };
                        state
                            .voice
                            .advance(amplitude, sign, &config.response, config.adaptive, dt);
                        value = state.voice.output;
                        raw = value;
                        signed = config.detector == Detector::Balance;
                        if state.voice.fired && fresh {
                            *next_event = next_event.wrapping_add(1).max(1);
                            event = Event {
                                id: *next_event,
                                time,
                                strength: state.voice.input * d.influence,
                            };
                        }
                    }
                    Effect::Beat {
                        sensitivity,
                        manual,
                        bpm,
                        output,
                    } => {
                        control_available = true;
                        let energy = frame.features.as_ref().map_or_else(
                            || {
                                AudioFeatures::new(&frame.bins, frame.mix_rms, frame.rms)
                                    .band(30.0, 220.0)
                            },
                            |f| f.band(30.0, 220.0),
                        );
                        let (hit, pulse, phase, tempo) =
                            state
                                .beat
                                .step(energy, fresh, dt, time, *sensitivity, *manual, *bpm);
                        value = if *output == BeatOutput::Pulse {
                            pulse
                        } else {
                            phase
                        };
                        raw = value;
                        signed = false;
                        state.monitor.bpm = tempo;
                        state.monitor.phase = phase;
                        state.monitor.confidence = state.beat.confidence;
                        if hit {
                            *next_event = next_event.wrapping_add(1).max(1);
                            event = Event {
                                id: *next_event,
                                time,
                                strength: 1.0,
                            };
                        }
                    }
                    Effect::Gain { gain } => value *= gain,
                    Effect::Normalize {
                        low,
                        high,
                        automatic,
                    } => {
                        let (lo, hi) = if *automatic {
                            let (mut lo, mut hi) = state.bounds.unwrap_or((value, value));
                            let relax = 1.0 - (-dt / 4.0).exp();
                            lo += (value - lo) * relax;
                            hi += (value - hi) * relax;
                            lo = lo.min(value);
                            hi = hi.max(value);
                            state.bounds = Some((lo, hi));
                            (lo, hi)
                        } else {
                            (*low, *high)
                        };
                        value = if hi - lo > 0.0001 {
                            ((value - lo) / (hi - lo)).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        signed = false;
                    }
                    Effect::Smooth { ms } => value = state.smoother.process(value, *ms, dt),
                    Effect::Envelope { attack, release } => {
                        let previous = state.smoother.value.unwrap_or(0.0);
                        value = state.smoother.process(
                            value,
                            if value.abs() > previous.abs() {
                                *attack
                            } else {
                                *release
                            },
                            dt,
                        );
                    }
                    Effect::Threshold {
                        threshold,
                        hysteresis,
                    } => {
                        state.gate = if state.gate {
                            value >= threshold - hysteresis
                        } else {
                            value >= *threshold
                        };
                        value = if state.gate { 1.0 } else { 0.0 };
                        signed = false;
                    }
                    Effect::Shape(shape) => value = shape.map(value, signed),
                    Effect::Invert => value = if signed { -value } else { 1.0 - value },
                    Effect::Quantize { steps } => {
                        let n = (*steps - 1) as f32;
                        value = (value * n).round() / n;
                    }
                }
            } else if insert.effect.input() != insert.effect.output() {
                control_available = false;
                value = 0.0;
                raw = 0.0;
                signed = false;
                state.voice = Voice::default();
            }
            data = insert.effect.output();
            if data == Data::Control && !control_available {
                value = 0.0;
                raw = 0.0;
            }
            let output = if data == Data::Audio {
                audio.as_ref().map_or(0.0, |f| f.frame.mix_rms)
            } else {
                value
            };
            state.monitor.id = insert.id;
            state.monitor.input = input;
            state.monitor.output = output;
            if state
                .history
                .back()
                .is_none_or(|p| time - p.time >= 1.0 / 60.0 - 1e-6)
            {
                state.history.push_back(Point {
                    time,
                    input,
                    output,
                });
                while state.history.len() > 121 {
                    state.history.pop_front();
                }
                state.monitor.history = state.history.iter().copied().collect::<Vec<_>>().into();
            }
            monitors.push(state.monitor.clone());
        }
        if d.input == Input::System
            && !self.audio_signature.is_empty()
            && let Some(feed) = &audio
        {
            static NEXT_REVISION: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(1);
            let cached = self.audio_cache.as_ref();
            let same_frame = cached.is_some_and(|old| Arc::ptr_eq(&old.frame, &feed.frame));
            if !same_frame {
                let (history, revision) = cached.map_or_else(
                    || {
                        let mut history = crate::capture_history::CaptureHistory::default();
                        history.reset(source.capture_epoch);
                        (
                            Arc::new(std::sync::Mutex::new(history)),
                            NEXT_REVISION.fetch_add(1, Ordering::Relaxed),
                        )
                    },
                    |old| (old.history.clone(), old.revision),
                );
                history
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(&feed.frame, Instant::now());
                self.audio_cache = Some(Arc::new(audio::Feed {
                    frame: feed.frame.clone(),
                    history,
                    revision,
                }));
            }
            audio = self.audio_cache.clone();
        }
        if d.input == Input::System && base.status != Status::Live {
            audio = self
                .silent_audio
                .process(
                    &[audio::InputBus {
                        id: d.id,
                        enabled: false,
                        ..Default::default()
                    }],
                    source,
                )
                .remove(&d.id);
        }
        let mut sample = base.clone();
        sample.raw = raw;
        sample.event = event;
        if d.input == Input::System {
            let rms = audio.as_ref().map_or(0.0, |f| f.frame.mix_rms);
            sample.level = if rms > 1e-9 {
                ((20.0 * rms.log10() + 72.0) / 66.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
        }
        if base.status == Status::Live {
            if !control_available {
                sample.event = Event::default();
            }
            sample.value = if data == Data::Control && control_available {
                value * d.influence
            } else {
                0.0
            };
            sample.input = if d.input == Input::System {
                audio.as_ref().map_or(0.0, |f| f.frame.mix_rms)
            } else {
                base.input
            };
            sample.availability = if data == Data::Control && control_available {
                1.0
            } else {
                0.0
            };
        }
        Result {
            sample,
            audio,
            monitors,
        }
    }
}

#[cfg(test)]
mod tests;
