//! Reusable musical drivers. The worker owns detector state; views only consume snapshots.
use crate::{
    analysis::AnalysisFrame,
    modulation::{AudioFeatures, ModulationSettings, Response, Route, Source, Voice},
};
use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
pub mod shaping;
pub mod ui;

pub const CHANNEL_COLORS: [[u8; 3]; 8] = [
    [35, 214, 240],
    [74, 151, 255],
    [230, 85, 220],
    [255, 199, 55],
    [45, 223, 148],
    [255, 131, 65],
    [161, 116, 255],
    [183, 224, 65],
];
pub const MAX_SIGNALS: usize = 512;
pub const STALE_SECONDS: f64 = 0.5;
pub const FADE_SECONDS: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Input {
    #[default]
    System,
    Lfo,
    Midi,
    Osc,
    Ableton,
    File,
}
impl Input {
    pub const ALL: [Self; 6] = [
        Self::System,
        Self::Lfo,
        Self::Midi,
        Self::Osc,
        Self::Ableton,
        Self::File,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Main audio",
            Self::Lfo => "LFO",
            Self::Midi => "MIDI",
            Self::Osc => "OSC · planned",
            Self::Ableton => "Ableton · planned",
            Self::File => "Audio file · planned",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    Intensity,
    Band,
    BassHits,
    SpectralChange,
    Balance,
    Lfo,
    Midi,
}
impl Method {
    pub const ALL: [Self; 7] = [
        Self::Intensity,
        Self::Band,
        Self::BassHits,
        Self::SpectralChange,
        Self::Balance,
        Self::Lfo,
        Self::Midi,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Intensity => "Overall intensity",
            Self::Band => "Band energy",
            Self::BassHits => "Bass hits",
            Self::SpectralChange => "Spectral change",
            Self::Balance => "Stereo balance",
            Self::Lfo => "LFO",
            Self::Midi => "MIDI control",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Midi => {
                "MIDI controller, note, pitch bend or pressure from an enabled input. Configure the message in the MIDI mapping section. Values hold between messages; disconnected inputs fade to zero."
            }
            Self::Lfo => "Free-running oscillator, independent of the audio device. Rate is in Hz.",
            Self::Intensity => "Full-mix RMS level / shaped intensity.",
            Self::Band => "Energy within the selected band / shaped intensity.",
            Self::BassHits => {
                "Estimated low-band onsets from a mix, not isolated kick drums. Produces hit events and an envelope."
            }
            Self::SpectralChange => {
                "Mean positive change in 96 logarithmic spectral levels, per second. Responds to new spectral energy, not sustained tones."
            }
            Self::Balance => {
                "Signed left/right balance, gated by mix level. Left is negative; right is positive."
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Waveform {
    #[default]
    Sine,
    Triangle,
    Saw,
    Square,
}
impl Waveform {
    pub const ALL: [Self; 4] = [Self::Sine, Self::Triangle, Self::Saw, Self::Square];
    pub fn label(self) -> &'static str {
        match self {
            Self::Sine => "Sine",
            Self::Triangle => "Triangle",
            Self::Saw => "Saw",
            Self::Square => "Square",
        }
    }
    fn value(self, phase: f64) -> f32 {
        let p = phase.rem_euclid(1.0) as f32;
        match self {
            Self::Sine => (p * std::f32::consts::TAU).sin(),
            Self::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
            Self::Saw => 2.0 * p - 1.0,
            Self::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lfo {
    pub waveform: Waveform,
    pub rate: f32,
    pub phase: f32,
    pub depth: f32,
    pub bipolar: bool,
}
impl Default for Lfo {
    fn default() -> Self {
        Self {
            waveform: Waveform::Sine,
            rate: 0.25,
            phase: 0.0,
            depth: 1.0,
            bipolar: true,
        }
    }
}
impl Lfo {
    fn valid(&self) -> bool {
        (0.01..=20.0).contains(&self.rate)
            && (0.0..=1.0).contains(&self.phase)
            && (0.0..=1.0).contains(&self.depth)
    }
    fn sanitize(&mut self) {
        let bound = |v: f32, lo: f32, hi: f32, fallback| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                fallback
            }
        };
        self.rate = bound(self.rate, 0.01, 20.0, 0.25);
        self.phase = bound(self.phase, 0.0, 1.0, 0.0);
        self.depth = bound(self.depth, 0.0, 1.0, 1.0);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputKind {
    Unipolar,
    Bipolar,
    EventsAndEnvelope,
}
/// Optional calibration of a detector/envelope's output before its influence fader.
/// Saved source configuration; live learning and observed samples are not saved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Normalization {
    pub enabled: bool,
    pub low: f32,
    pub high: f32,
}
impl Default for Normalization {
    fn default() -> Self {
        Self {
            enabled: false,
            low: 0.0,
            high: 1.0,
        }
    }
}
impl Normalization {
    pub fn valid(&self) -> bool {
        self.low.is_finite()
            && self.high.is_finite()
            && (-1.0..=1.0).contains(&self.low)
            && (-1.0..=1.0).contains(&self.high)
            && self.high - self.low >= 0.0001 - 1e-7
    }
    fn sanitize(&mut self) {
        if !self.low.is_finite() {
            self.low = 0.0;
        }
        if !self.high.is_finite() {
            self.high = 1.0;
        }
        self.low = self.low.clamp(-1.0, 0.9999);
        self.high = self.high.clamp(self.low + 0.0001, 1.0);
    }
    pub fn map(&self, raw: f32) -> f32 {
        if self.enabled {
            ((raw - self.low) / (self.high - self.low).max(f32::EPSILON)).clamp(0.0, 1.0)
        } else {
            raw
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Definition {
    pub id: u64,
    pub name: String,
    pub input: Input,
    pub method: Method,
    pub enabled: bool,
    #[serde(default)]
    pub solo: bool,
    pub influence: f32,
    pub adaptive: bool,
    #[serde(default)]
    pub normalization: Normalization,
    /// Final output lag, after calibration/influence. Zero bypasses smoothing.
    #[serde(default)]
    pub smoothing_ms: f32,
    #[serde(default)]
    pub shaping: shaping::Shaping,
    // The response is shared; mapping amount/range/target are not used here.
    pub response: Route,
    #[serde(default)]
    pub lfo: Lfo,
    #[serde(default)]
    pub midi: crate::midi::Binding,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
}
impl Definition {
    pub fn color_rgb(&self) -> [u8; 3] {
        self.color
            .unwrap_or(CHANNEL_COLORS[(self.id.saturating_sub(1) % 8) as usize])
    }
    pub fn recipe(id: u64, method: Method) -> Self {
        let mut response = Route::default();
        if method == Method::BassHits {
            response.response = Response::Transient;
            response.envelope.attack = 5.0;
            response.envelope.release = 250.0;
            response.high = 180.0;
        }
        if method == Method::Intensity {
            response.source = Source::FullMix;
        }
        if method == Method::Balance {
            response.source = Source::Stereo;
        }
        Self {
            id,
            name: method.label().into(),
            input: if method == Method::Midi {
                Input::Midi
            } else if method == Method::Lfo {
                Input::Lfo
            } else {
                Input::System
            },
            method,
            enabled: true,
            solo: false,
            influence: 1.0,
            adaptive: false,
            normalization: Normalization::default(),
            smoothing_ms: 0.0,
            shaping: Default::default(),
            response,
            lfo: Lfo::default(),
            midi: Default::default(),
            color: None,
        }
    }
    pub fn sanitize(&mut self) {
        self.lfo.sanitize();
        self.midi.sanitize();
        if self.input == Input::Midi || self.method == Method::Midi {
            self.input = Input::Midi;
            self.method = Method::Midi;
        }
        self.normalization.sanitize();
        self.shaping.sanitize();
        if !self.smoothing_ms.is_finite() {
            self.smoothing_ms = 0.0;
        }
        self.smoothing_ms = self.smoothing_ms.clamp(0.0, 5000.0);
        if self.method == Method::Lfo {
            self.input = Input::Lfo;
        }
        let mut settings = ModulationSettings {
            routes: vec![self.response.clone()],
            ..Default::default()
        };
        settings.sanitize(crate::modulation::Visualizer::Flux);
        self.response = settings.routes.remove(0);
        self.response.signal = None;
        self.response.amount = 1.0;
        self.response.output_min = 0.0;
        self.response.output_max = 1.0;
        if self.method == Method::BassHits {
            self.response.response = Response::Transient;
        }
        if !self.influence.is_finite() {
            self.influence = 1.0;
        }
        self.influence = self.influence.clamp(0.0, 2.0);
    }
    pub fn kind(&self) -> OutputKind {
        if self.input == Input::Midi {
            if self.midi.message == crate::midi::Message::PitchBend && !self.normalization.enabled {
                OutputKind::Bipolar
            } else {
                OutputKind::Unipolar
            }
        } else if self.normalization.enabled {
            if self.input != Input::Lfo && self.response.response == Response::Transient {
                OutputKind::EventsAndEnvelope
            } else {
                OutputKind::Unipolar
            }
        } else if self.input == Input::Lfo {
            if self.lfo.bipolar {
                OutputKind::Bipolar
            } else {
                OutputKind::Unipolar
            }
        } else if self.method == Method::Balance {
            OutputKind::Bipolar
        } else if self.response.response == Response::Transient {
            OutputKind::EventsAndEnvelope
        } else {
            OutputKind::Unipolar
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bank {
    pub slots: Vec<Option<u64>>,
    pub channels: Vec<Definition>,
    pub next_id: u64,
}
impl Default for Bank {
    fn default() -> Self {
        Self {
            channels: Vec::new(),
            slots: Vec::new(),
            next_id: 1,
        }
    }
}
impl Bank {
    pub fn sync_slots(&mut self) {
        // Older projects contain vacant mixer slots. Preserve occupied order,
        // but show only actual channels and append newly created sources.
        self.slots
            .retain(|slot| slot.is_some_and(|id| self.channels.iter().any(|d| d.id == id)));
        for d in &self.channels {
            if !self.slots.contains(&Some(d.id)) {
                self.slots.push(Some(d.id));
            }
        }
    }
    /// Insert at a boundary in the current slot order (0 = before the first slot).
    /// Account for the source slot disappearing when moving toward the right.
    pub fn insert_channel(&mut self, id: u64, boundary: usize) {
        if let Some(from) = self.slots.iter().position(|s| *s == Some(id))
            && boundary <= self.slots.len()
        {
            let to = boundary - usize::from(from < boundary);
            if to == from {
                return;
            }
            let channel = self.slots.remove(from);
            self.slots.insert(to, channel);
            self.channels.sort_by_key(|d| {
                self.slots
                    .iter()
                    .position(|s| *s == Some(d.id))
                    .unwrap_or(usize::MAX)
            });
        }
    }
    pub fn remove_channel(&mut self, id: u64) -> Option<(Definition, usize)> {
        let index = self.channels.iter().position(|d| d.id == id)?;
        let slot = self
            .slots
            .iter()
            .position(|s| *s == Some(id))
            .unwrap_or(index);
        if slot < self.slots.len() {
            self.slots.remove(slot);
        }
        Some((self.channels.remove(index), slot))
    }

    pub fn allocate(&mut self) -> Option<u64> {
        if self.channels.len() >= MAX_SIGNALS {
            return None;
        }
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1)?;
        Some(id)
    }
    pub fn add(&mut self, method: Method) -> Option<u64> {
        let id = self.allocate()?;
        self.channels.push(Definition::recipe(id, method));
        Some(id)
    }
    pub fn duplicate(&mut self, source: u64) -> Option<u64> {
        let mut d = self.channels.iter().find(|d| d.id == source)?.clone();
        d.color = Some(d.color_rgb());
        d.id = self.allocate()?;
        d.name = format!("{} copy", d.name.chars().take(70).collect::<String>());
        let id = d.id;
        self.channels.push(d);
        Some(id)
    }
    #[cfg(test)]
    pub(crate) fn test_sources() -> Self {
        let mut bank = Self::default();
        for method in Method::ALL
            .into_iter()
            .filter(|m| !matches!(m, Method::Lfo | Method::Midi))
        {
            bank.add(method).unwrap();
        }
        bank
    }
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        let mut slots = std::collections::HashSet::new();
        self.slots.len() <= MAX_SIGNALS + 2
            && self
                .slots
                .iter()
                .flatten()
                .all(|id| slots.insert(*id) && self.channels.iter().any(|d| d.id == *id))
            && self.channels.len() <= MAX_SIGNALS
            && self.next_id > 0
            && self.channels.iter().all(|d| {
                d.id > 0
                    && d.id < self.next_id
                    && ids.insert(d.id)
                    && crate::workspace::valid_name(&d.name)
                    && (0.0..=2.0).contains(&d.influence)
                    && d.response.signal.is_none()
                    && d.lfo.valid()
                    && d.midi.valid()
                    && ((d.input == Input::Midi) == (d.method == Method::Midi))
                    && d.normalization.valid()
                    && d.shaping.valid()
                    && d.smoothing_ms.is_finite()
                    && (0.0..=5000.0).contains(&d.smoothing_ms)
                    && ((d.input == Input::Lfo) == (d.method == Method::Lfo))
                    && {
                        let r = &d.response;
                        (20.0..=19980.0).contains(&r.low)
                            && (r.low * 1.001..=20000.0).contains(&r.high)
                            && (-90.0..=-12.0).contains(&r.floor_db)
                            && (-24.0..=24.0).contains(&r.gain_db)
                            && (0.25..=4.0).contains(&r.curve)
                            && (0.01..=0.99).contains(&r.threshold)
                            && (20.0..=2000.0).contains(&r.hold_ms)
                            && (1.0..=2000.0).contains(&r.envelope.attack)
                            && (1.0..=3000.0).contains(&r.envelope.decay)
                            && (0.0..=1.0).contains(&r.envelope.sustain)
                            && (1.0..=5000.0).contains(&r.envelope.release)
                    }
            })
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Event {
    pub id: u64,
    pub time: f64,
    pub strength: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Status {
    Live,
    Stale,
    Muted,
    #[default]
    Waiting,
    Planned,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Live => "Live",
            Self::Stale => "Disconnected · fading",
            Self::Muted => "Muted",
            Self::Waiting => "Waiting for audio",
            Self::Planned => "Not connected · planned",
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Sample {
    /// Input activity in dB-normalized display space, before artistic shaping.
    pub level: f32,
    pub input: f32,
    /// Detector/envelope output before calibration and the influence fader.
    pub raw: f32,
    pub value: f32,
    pub availability: f32,
    pub status: Status,
    pub event: Event,
}
impl Sample {
    pub fn mapped(&self, r: &Route) -> f32 {
        let v = if r.invert { -self.value } else { self.value };
        // Range floor must fade with the provider, too; missing data never leaves an offset.
        (r.output_min * self.availability + v * (r.output_max - r.output_min)) * r.amount
    }
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub time: f64,
    pub values: BTreeMap<u64, Sample>,
}
/// A time-based one-pole smoother shared by every provider. Its value is
/// transient; changing the amount does not reset it or restart the detector.
#[derive(Default)]
struct OutputSmoother {
    value: Option<f32>,
}
impl OutputSmoother {
    fn process(&mut self, target: f32, milliseconds: f32, dt: f32) -> f32 {
        let value = match self.value {
            Some(previous) if milliseconds > 0.0 => {
                let alpha = -(-dt.max(0.0) * 1000.0 / milliseconds).exp_m1();
                previous + (target - previous) * alpha
            }
            _ => target,
        };
        self.value = Some(value);
        value
    }
}
#[derive(Default)]
struct ChannelState {
    signature: Option<(Input, Method, Source, Response, u32, u32)>,
    voice: Voice,
    sample: Sample,
    prior_spectrum: Option<[f32; 96]>,
    spectral: f32,
    lfo_phase: f64,
    output_smoother: OutputSmoother,
    midi_binding: Option<crate::midi::Binding>,
}
#[derive(Default)]
pub struct Processor {
    states: BTreeMap<u64, ChannelState>,
    epoch: Option<(u64, u32, usize)>,
    sequence: Option<u64>,
    last_frame: f64,
    last_step: f64,
    next_event: u64,
}
impl Processor {
    #[cfg(test)]
    pub fn step(
        &mut self,
        bank: &Bank,
        frame: &AnalysisFrame,
        time: f64,
        connected: bool,
    ) -> Snapshot {
        self.step_with_midi(
            bank,
            frame,
            time,
            connected,
            &crate::midi::Snapshot::default(),
        )
    }
    pub fn step_with_midi(
        &mut self,
        bank: &Bank,
        frame: &AnalysisFrame,
        time: f64,
        connected: bool,
        midi: &crate::midi::Snapshot,
    ) -> Snapshot {
        let dt = (time - self.last_step).clamp(0.0, 0.1) as f32;
        self.last_step = time;
        let epoch = (frame.capture_epoch, frame.sample_rate, frame.fft_size);
        if self.epoch != Some(epoch) {
            self.epoch = Some(epoch);
            self.sequence = None;
            // Keep output for the disconnect fade, discard only detector history.
            for s in self.states.values_mut() {
                s.voice = Voice::default();
                s.prior_spectrum = None;
            }
        }
        let fresh = frame.sequence > 0 && self.sequence != Some(frame.sequence);
        let frame_dt = (time - self.last_frame).max(1.0 / 240.0) as f32;
        if fresh {
            self.last_frame = time;
            self.sequence = Some(frame.sequence);
        }
        let live = connected && self.sequence.is_some() && time - self.last_frame <= STALE_SECONDS;
        let fallback;
        let f = if let Some(f) = &frame.features {
            f.as_ref()
        } else {
            fallback = AudioFeatures::new(&frame.bins, frame.mix_rms, frame.rms);
            &fallback
        };
        self.states
            .retain(|id, _| bank.channels.iter().any(|d| d.id == *id));
        let any_solo = bank.channels.iter().any(|d| d.solo);
        for d in &bank.channels {
            let s = self.states.entry(d.id).or_default();
            let sig = (
                d.input,
                d.method,
                d.response.source,
                d.response.response,
                d.response.low.to_bits(),
                d.response.high.to_bits(),
            );
            if s.signature != Some(sig)
                || (d.input == Input::Midi && s.midi_binding.as_ref() != Some(&d.midi))
            {
                *s = ChannelState {
                    signature: Some(sig),
                    midi_binding: Some(d.midi.clone()),
                    ..Default::default()
                };
            }
            let status = if !d.enabled || (any_solo && !d.solo) {
                Status::Muted
            } else if d.input == Input::Lfo {
                Status::Live
            } else if d.input == Input::Midi {
                if midi.available(&d.midi) {
                    Status::Live
                } else {
                    Status::Stale
                }
            } else if d.input != Input::System {
                Status::Planned
            } else if live {
                Status::Live
            } else if self.sequence.is_some() {
                Status::Stale
            } else {
                Status::Waiting
            };
            s.sample.status = status;
            if status != Status::Live {
                let factor = (1.0 - dt / FADE_SECONDS).max(0.0);
                s.sample.value *= factor;
                s.sample.availability *= factor;
                s.sample.input = 0.0;
                s.sample.raw = 0.0;
                s.sample.level = 0.0;
                if s.sample.availability < 0.001 {
                    s.sample.value = 0.0;
                    s.sample.availability = 0.0;
                }
                s.voice = Voice::default();
                s.prior_spectrum = None;
                // Disconnect/mute uses the existing short fade, independent of
                // smoothing. Reconnection resumes on the very next live sample.
                s.output_smoother = OutputSmoother::default();
                continue;
            }
            if d.input == Input::Midi {
                s.sample.raw = midi.value(&d.midi);
                s.sample.input = s.sample.raw.abs();
                s.sample.level = s.sample.input;
                s.sample.value = s.output_smoother.process(
                    d.shaping.map(
                        d.normalization.map(s.sample.raw),
                        d.kind() == OutputKind::Bipolar,
                    ) * d.influence,
                    d.smoothing_ms,
                    dt,
                );
                s.sample.availability = 1.0;
                continue;
            }
            if d.input == Input::Lfo {
                s.lfo_phase = (s.lfo_phase + f64::from(dt * d.lfo.rate)).rem_euclid(1.0);
                let wave = d.lfo.waveform.value(s.lfo_phase + f64::from(d.lfo.phase));
                let value = if d.lfo.bipolar {
                    wave
                } else {
                    (wave + 1.0) * 0.5
                };
                s.sample.raw = value * d.lfo.depth;
                s.sample.value = s.output_smoother.process(
                    d.shaping.map(
                        d.normalization.map(s.sample.raw),
                        d.kind() == OutputKind::Bipolar,
                    ) * d.influence,
                    d.smoothing_ms,
                    dt,
                );
                s.sample.input = s.sample.raw.abs();
                s.sample.level = s.sample.input;
                s.sample.availability = 1.0;
                continue;
            }
            if fresh && d.method == Method::SpectralChange {
                s.spectral = s.prior_spectrum.map_or(0.0, |old| {
                    f.spectrum
                        .iter()
                        .zip(old)
                        .map(|(a, b)| (a - b).max(0.0))
                        .sum::<f32>()
                        / 96.0
                        / frame_dt
                });
                s.prior_spectrum = Some(f.spectrum);
            }
            let amplitude = match d.method {
                Method::Lfo | Method::Midi => 0.0,
                Method::Intensity => f.rms,
                Method::Band | Method::BassHits => f.band(d.response.low, d.response.high),
                Method::Balance => f.rms * f.balance.abs(),
                Method::SpectralChange => s.spectral,
            };
            let measured = if matches!(d.method, Method::Balance | Method::SpectralChange) {
                f.rms
            } else {
                amplitude
            };
            s.sample.level = if measured > 1e-9 {
                ((20.0 * measured.log10() + d.response.gain_db + 72.0) / 66.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let sign = if d.method == Method::Balance {
                f.balance.signum()
            } else {
                1.0
            };
            let mut response = d.response.clone();
            response.amount = 1.0;
            response.output_min = 0.0;
            response.output_max = 1.0;
            s.voice.advance(amplitude, sign, &response, d.adaptive, dt);
            if s.voice.fired && fresh {
                self.next_event = self.next_event.wrapping_add(1).max(1);
                s.sample.event = Event {
                    id: self.next_event,
                    time,
                    strength: s.voice.input * d.influence,
                };
            }
            s.sample.input = s.voice.input;
            s.sample.raw = s.voice.output;
            s.sample.value = s.output_smoother.process(
                d.shaping.map(
                    d.normalization.map(s.sample.raw),
                    d.kind() == OutputKind::Bipolar,
                ) * d.influence,
                d.smoothing_ms,
                dt,
            );
            s.sample.availability = 1.0;
        }
        Snapshot {
            time,
            values: self
                .states
                .iter()
                .map(|(id, s)| (*id, s.sample.clone()))
                .collect(),
        }
    }
}
/// Independent 120 Hz worker: continues while tabs, panes or the OS window are hidden.
pub struct Runtime {
    audio: Arc<ArcSwap<ArcSwap<AnalysisFrame>>>,
    pub definitions: Arc<ArcSwap<Bank>>,
    pub snapshot: Arc<ArcSwap<Snapshot>>,
    pub connected: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Runtime {
    pub fn start(
        audio: Arc<ArcSwap<AnalysisFrame>>,
        midi: Arc<ArcSwap<crate::midi::Snapshot>>,
    ) -> Self {
        let audio = Arc::new(ArcSwap::from(audio));
        let source = audio.clone();
        let definitions = Arc::new(ArcSwap::from_pointee(Bank::default()));
        let snapshot = Arc::new(ArcSwap::from_pointee(Snapshot::default()));
        let connected = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let (defs, out, live, quit) = (
            definitions.clone(),
            snapshot.clone(),
            connected.clone(),
            stop.clone(),
        );
        let worker = std::thread::spawn(move || {
            let start = Instant::now();
            let mut processor = Processor::default();
            let mut bound = source.load_full();
            let interval = Duration::from_secs_f64(1.0 / 120.0);
            let mut deadline = Instant::now();
            while !quit.load(Ordering::Relaxed) {
                let next = source.load_full();
                if !Arc::ptr_eq(&bound, &next) {
                    // CoreAudio publishes a new analysis runtime for each capture session.
                    // Epoch and sequence numbers can repeat across different runtimes.
                    processor.epoch = None;
                    processor.sequence = None;
                    bound = next;
                }
                let time = start.elapsed().as_secs_f64();
                out.store(Arc::new(processor.step_with_midi(
                    &defs.load(),
                    &bound.load(),
                    time,
                    live.load(Ordering::Relaxed),
                    &midi.load(),
                )));
                // Processing time is part of the 120 Hz budget, not added to it.
                // Skip missed deadlines instead of issuing bursts after a stall.
                deadline += interval;
                let now = Instant::now();
                if deadline > now {
                    std::thread::sleep(deadline - now);
                } else {
                    deadline = now;
                }
            }
        });
        Self {
            audio,
            definitions,
            snapshot,
            connected,
            stop,
            worker: Some(worker),
        }
    }
    pub fn bind_audio(&self, audio: Arc<ArcSwap<AnalysisFrame>>) {
        if !Arc::ptr_eq(&self.audio.load_full(), &audio) {
            self.audio.store(audio);
        }
    }
    pub fn publish(&self, bank: &Bank) {
        if **self.definitions.load() != *bank {
            self.definitions.store(Arc::new(bank.clone()));
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::FrequencyBin,
        modulation::{Modulator, Target},
    };
    fn tone(sequence: u64, hz: f32, amplitude: f32) -> AnalysisFrame {
        AnalysisFrame {
            sequence,
            bins: vec![FrequencyBin {
                frequency_hz: hz,
                magnitude: amplitude,
                dbfs: 20.0 * amplitude.max(1e-9).log10(),
            }]
            .into(),
            mix_rms: amplitude,
            rms: [amplitude; 2],
            ..Default::default()
        }
    }
    fn bank(method: Method) -> Bank {
        Bank {
            slots: Vec::new(),
            channels: vec![Definition::recipe(1, method)],
            next_id: 2,
        }
    }
    #[test]
    fn final_smoothing_is_time_based_and_bypass_preserves_exact_targets() {
        let mut off = OutputSmoother::default();
        for target in [0.5, 1.0, -0.8, 0.0, 2.0] {
            assert_eq!(off.process(target, 0.0, 1.0 / 120.0), target);
        }
        let settle = |steps: &[f32]| {
            let mut filter = OutputSmoother::default();
            filter.process(0.0, 300.0, 0.0);
            for &dt in steps {
                filter.process(1.0, 300.0, dt);
            }
            filter.value.unwrap()
        };
        let want = 1.0 - (-1.0_f32).exp();
        for steps in [
            vec![1.0 / 60.0; 18],
            vec![1.0 / 120.0; 36],
            vec![0.017, 0.043, 0.08, 0.07, 0.09],
        ] {
            assert!((settle(&steps) - want).abs() < 1e-6);
        }
        let mut filter = OutputSmoother::default();
        filter.process(0.0, 200.0, 0.0);
        for n in 0..240 {
            let value = filter.process(if n % 2 == 0 { 1.0 } else { -1.0 }, 200.0, 1.0 / 120.0);
            assert!(value.abs() < 0.05); // preserves signed output while suppressing jitter
        }
    }
    #[test]
    fn smoothing_follows_normalization_and_fader_without_changing_detector_or_events() {
        for method in Method::ALL {
            let mut smoothed = bank(method);
            smoothed.channels[0].normalization = Normalization {
                enabled: true,
                low: 0.2,
                high: 0.7,
            };
            smoothed.channels[0].influence = 0.6;
            smoothed.channels[0].smoothing_ms = 200.0;
            let mut immediate = smoothed.clone();
            immediate.channels[0].smoothing_ms = 0.0;
            let (mut a, mut b) = (Processor::default(), Processor::default());
            let mut previous = None;
            for n in 1..=240 {
                let f = tone(n, 80.0, if n % 60 < 30 { 0.2 } else { 0.0001 });
                let time = n as f64 / 120.0;
                let sa = a.step(&smoothed, &f, time, true);
                let sb = b.step(&immediate, &f, time, true);
                let (sa, sb) = (&sa.values[&1], &sb.values[&1]);
                assert_eq!(sa.input, sb.input);
                assert_eq!(sa.raw, sb.raw);
                assert_eq!(sa.event.id, sb.event.id);
                assert_eq!(sa.event.strength, sb.event.strength);
                let want = previous.map_or(sb.value, |old: f32| {
                    old + (sb.value - old) * -(-1.0_f32 / 120.0 * 1000.0 / 200.0).exp_m1()
                });
                assert!((sa.value - want).abs() < 1e-6);
                assert!((0.0..=0.6).contains(&sa.value));
                previous = Some(sa.value);
            }
        }
    }
    #[test]
    fn heavy_smoothing_does_not_delay_disconnect_fade_or_live_reconnection() {
        let mut bank = bank(Method::Lfo);
        bank.channels[0].smoothing_ms = 5000.0;
        let mut processor = Processor::default();
        let f = AnalysisFrame::default();
        processor.step(&bank, &f, 0.0, false);
        let active = processor.step(&bank, &f, 0.1, false).values[&1].value;
        bank.channels[0].enabled = false;
        let faded = processor.step(&bank, &f, 0.2, false);
        assert!((faded.values[&1].value - active * (1.0 - 0.1 / FADE_SECONDS)).abs() < 1e-6);
        bank.channels[0].enabled = true;
        let resumed = processor.step(&bank, &f, 0.21, false);
        let s = &resumed.values[&1];
        assert_eq!(s.status, Status::Live);
        assert_eq!(s.value, s.raw * bank.channels[0].influence);
        assert_eq!(s.availability, 1.0);
    }
    #[test]
    fn smoothing_is_saved_per_channel_and_old_projects_default_off() {
        let mut bank = bank(Method::Intensity);
        bank.channels[0].smoothing_ms = 250.0;
        let restored: Bank = serde_json::from_value(serde_json::to_value(&bank).unwrap()).unwrap();
        assert_eq!(bank, restored);
        assert!(bank.valid());
        let id = bank.duplicate(1).unwrap();
        assert_eq!(
            bank.channels
                .iter()
                .find(|d| d.id == id)
                .unwrap()
                .smoothing_ms,
            250.0
        );
        let mut old = serde_json::to_value(&bank).unwrap();
        for d in old["channels"].as_array_mut().unwrap() {
            d.as_object_mut().unwrap().remove("smoothing_ms");
        }
        let restored: Bank = serde_json::from_value(old).unwrap();
        assert!(restored.channels.iter().all(|d| d.smoothing_ms == 0.0));
        bank.channels[0].smoothing_ms = f32::NAN;
        assert!(!bank.valid());
        bank.channels[0].sanitize();
        assert_eq!(bank.channels[0].smoothing_ms, 0.0);
        bank.channels[0].smoothing_ms = 6000.0;
        assert!(!bank.valid());
        bank.channels[0].sanitize();
        assert!(bank.valid());
    }
    #[test]
    fn calibrated_narrow_range_covers_full_macro_travel_and_is_saved() {
        let calibration = Normalization {
            enabled: true,
            low: 0.50,
            high: 0.55,
        };
        for (raw, want) in [
            (0.4, 0.0),
            (0.50, 0.0),
            (0.525, 0.5),
            (0.55, 1.0),
            (0.8, 1.0),
        ] {
            assert!((calibration.map(raw) - want).abs() < 1e-5);
        }
        let mut bank = bank(Method::Band);
        bank.channels[0].normalization = calibration;
        assert!(bank.valid());
        let restored: Bank = serde_json::from_value(serde_json::to_value(&bank).unwrap()).unwrap();
        assert_eq!(bank, restored);
        let mut old = serde_json::to_value(&bank).unwrap();
        old["channels"][0]
            .as_object_mut()
            .unwrap()
            .remove("normalization");
        let restored: Bank = serde_json::from_value(old).unwrap();
        assert!(!restored.channels[0].normalization.enabled);
        bank.channels[0].normalization.high = 0.5;
        assert!(!bank.valid());
        bank.channels[0].normalization.high = f32::NAN;
        bank.channels[0].sanitize();
        assert!(bank.valid());
        for low in [0.4, 0.9999] {
            let mut range = Normalization {
                enabled: true,
                low,
                high: low,
            };
            range.sanitize();
            assert!(range.valid());
            assert_eq!(range.map(range.low), 0.0);
            assert_eq!(range.map(range.high), 1.0);
        }
    }
    #[test]
    fn processor_calibrates_after_envelope_before_influence_and_disconnect_still_fades() {
        let mut bank = bank(Method::Intensity);
        bank.channels[0].normalization = Normalization {
            enabled: true,
            low: 0.0,
            high: 0.3,
        };
        bank.channels[0].influence = 0.75;
        let mut plain = bank.clone();
        plain.channels[0].normalization.enabled = false;
        let (mut processor, mut reference) = (Processor::default(), Processor::default());
        for n in 1..=240 {
            let f = tone(n, 80.0, 0.1);
            let a = processor.step(&bank, &f, n as f64 / 120.0, true);
            let b = reference.step(&plain, &f, n as f64 / 120.0, true);
            let (a, b) = (&a.values[&1], &b.values[&1]);
            assert_eq!(a.raw, b.raw);
            assert_eq!(a.input, b.input);
            assert!((a.value - bank.channels[0].normalization.map(a.raw) * 0.75).abs() < 1e-6);
        }
        let f = tone(240, 80.0, 0.1);
        let start = processor.step(&bank, &f, 2.001, false).values[&1].value;
        let end = processor.step(&bank, &f, 2.1, false).values[&1].value;
        assert!(end < start && end >= 0.0);
        let resumed = processor.step(&bank, &tone(241, 80.0, 0.1), 2.11, true);
        assert_eq!(resumed.values[&1].status, Status::Live);
        assert_eq!(resumed.values[&1].availability, 1.0);
    }
    #[test]
    fn lfo_calibration_handles_signed_sources_and_preserves_output_fader() {
        let mut bank = bank(Method::Lfo);
        bank.channels[0].normalization = Normalization {
            enabled: true,
            low: -1.0,
            high: 1.0,
        };
        bank.channels[0].influence = 0.5;
        assert_eq!(bank.channels[0].kind(), OutputKind::Unipolar);
        let mut processor = Processor::default();
        for n in 1..=480 {
            let out = processor.step(&bank, &AnalysisFrame::default(), n as f64 / 120.0, false);
            let s = &out.values[&1];
            assert!((s.value - (s.raw + 1.0) * 0.25).abs() < 1e-5);
            assert!((0.0..=0.5).contains(&s.value));
        }
    }
    #[test]
    fn shared_followers_are_independent_of_consumers_and_mapping_depth() {
        let bank = bank(Method::Band);
        let mut p = Processor::default();
        let mut f = tone(1, 80.0, 0.5);
        for i in 1..=120 {
            f.sequence = i;
            p.step(&bank, &f, i as f64 / 120.0, true);
        }
        let snapshot = p.step(&bank, &f, 1.01, true);
        assert!(snapshot.values[&1].value > 0.9);
        f.signals = Some(Arc::new(snapshot));
        let mut s = ModulationSettings {
            routes: vec![Route {
                signal: Some(1),
                target: Target::Expansion,
                ..Default::default()
            }],
            ..Default::default()
        };
        let (mut a, mut b) = (Modulator::default(), Modulator::default());
        a.advance(&f, true, &s, 0.01, 1);
        s.routes[0].amount = 0.3;
        b.advance(&f, true, &s, 0.01, 1);
        assert!((b.value(Target::Expansion) - a.value(Target::Expansion) * 0.3).abs() < 1e-5);
        assert_eq!(
            f.signals.as_ref().unwrap().values[&1].value,
            a.value(Target::Expansion)
        );
    }
    #[test]
    fn band_rejects_treble_and_balance_keeps_polarity() {
        let b = bank(Method::Band);
        let mut p = Processor::default();
        let mut s = Snapshot::default();
        for i in 1..120 {
            s = p.step(&b, &tone(i, 5000.0, 0.5), i as f64 / 120.0, true);
        }
        assert_eq!(s.values[&1].value, 0.0);
        let b = bank(Method::Balance);
        let mut p = Processor::default();
        for i in 1..120 {
            let mut f = tone(i, 80.0, 0.5);
            f.rms = [0.5, 0.0];
            s = p.step(&b, &f, i as f64 / 120.0, true);
        }
        assert!(s.values[&1].value < -0.9);
    }
    #[test]
    fn stale_fades_including_output_floor_and_recovers_on_next_frame() {
        let b = bank(Method::Intensity);
        let mut p = Processor::default();
        let f = tone(1, 80.0, 0.5);
        for i in 1..60 {
            p.step(&b, &tone(i, 80.0, 0.5), i as f64 / 120.0, true);
        }
        let r = Route {
            output_min: 0.5,
            ..Default::default()
        };
        let mut last = 2.0;
        for i in 60..300 {
            let s = p.step(&b, &f, i as f64 / 120.0, false);
            let v = s.values[&1].mapped(&r);
            assert!(v <= last);
            last = v;
        }
        assert_eq!(last, 0.0);
        let s = p.step(&b, &tone(400, 80.0, 0.5), 2.51, true);
        assert_eq!(s.values[&1].status, Status::Live);
        assert!(s.values[&1].value > 0.0);
    }
    #[test]
    fn silence_is_live_but_an_unchanged_sequence_expires() {
        let b = bank(Method::Intensity);
        let mut p = Processor::default();
        let f = tone(1, 80.0, 0.0);
        assert_eq!(p.step(&b, &f, 0.01, true).values[&1].status, Status::Live);
        assert_eq!(p.step(&b, &f, 0.6, true).values[&1].status, Status::Stale);
        assert_eq!(
            p.step(&b, &tone(2, 80.0, 0.0), 0.61, true).values[&1].status,
            Status::Live
        );
    }
    #[test]
    fn hit_is_dispatched_once_and_not_replayed_when_resuming() {
        let b = bank(Method::BassHits);
        let mut p = Processor::default();
        let mut f = tone(1, 80.0, 0.0);
        f.signals = Some(Arc::new(p.step(&b, &f, 0.01, true)));
        let s = ModulationSettings {
            routes: vec![Route {
                signal: Some(1),
                target: Target::Impact,
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut m = Modulator::default();
        m.advance(&f, true, &s, 0.01, 1);
        f = tone(2, 80.0, 0.5);
        f.signals = Some(Arc::new(p.step(&b, &f, 0.02, true)));
        assert!(f.signals.as_ref().unwrap().values[&1].event.id > 0);
        m.advance(&f, true, &s, 0.01, 1);
        assert!(m.event_strength > 0.0);
        m.advance(&f, true, &s, 0.01, 1);
        assert_eq!(m.event_strength, 0.0);
        m.consume_current_events(&f, &s);
        m.advance(&f, true, &s, 0.01, 1);
        assert_eq!(m.event_strength, 0.0);
        let event = f.signals.as_ref().unwrap().values[&1].event.id;
        for i in 3..100 {
            let s = p.step(&b, &tone(i, 80.0, 0.5), i as f64 / 120.0, true);
            assert_eq!(s.values[&1].event.id, event);
        }
    }
    #[test]
    fn manual_creation_and_duplication_keep_ids_and_independent_settings() {
        let mut b = Bank::default();
        assert!(b.channels.is_empty());
        let id = b.add(Method::BassHits).unwrap();
        let copy = b.duplicate(id).unwrap();
        b.channels
            .iter_mut()
            .find(|d| d.id == copy)
            .unwrap()
            .response
            .low = 50.0;
        assert_eq!(
            b.channels.iter().find(|d| d.id == id).unwrap().response.low,
            20.0
        );
        assert_ne!(id, copy);
        assert!(b.valid());
    }
    #[test]
    fn spectral_change_reacts_to_rises_and_settles_on_a_steady_spectrum() {
        let b = bank(Method::SpectralChange);
        let mut p = Processor::default();
        p.step(&b, &tone(1, 80.0, 0.0), 0.01, true);
        let rise = p.step(&b, &tone(2, 80.0, 0.5), 0.02, true);
        assert!(rise.values[&1].input > 0.0);
        let stable = p.step(&b, &tone(3, 80.0, 0.5), 0.03, true);
        assert_eq!(stable.values[&1].input, 0.0);
    }
    #[test]
    fn definition_edit_resets_only_that_detector_and_missing_ids_output_zero() {
        let mut b = Bank::test_sources();
        let mut p = Processor::default();
        for i in 1..120 {
            p.step(&b, &tone(i, 80.0, 0.5), i as f64 / 120.0, true);
        }
        b.channels[1].response.low = 5000.0;
        b.channels[1].response.high = 10000.0;
        let snapshot = p.step(&b, &tone(120, 80.0, 0.5), 1.0, true);
        assert!(snapshot.values[&1].value > 0.9);
        assert_eq!(snapshot.values[&2].value, 0.0);
        let mut m = Modulator::default();
        let mut f = tone(121, 80.0, 0.5);
        f.signals = Some(Arc::new(snapshot));
        let s = ModulationSettings {
            routes: vec![Route {
                signal: Some(999),
                output_min: 0.5,
                target: Target::Expansion,
                ..Default::default()
            }],
            ..Default::default()
        };
        m.advance(&f, true, &s, 0.01, 1);
        assert_eq!(m.value(Target::Expansion), 0.0);
    }
    #[test]
    #[ignore = "manual shared signal processing timing"]
    fn signal_processing_timing() {
        for count in [48, 512] {
            let mut b = Bank::default();
            for _ in 5..count {
                b.add(Method::Band);
            }
            let mut f = tone(1, 80.0, 0.1);
            f.bins = (0..=4096)
                .map(|i| FrequencyBin {
                    frequency_hz: i as f32 * 48000.0 / 8192.0,
                    magnitude: 0.002,
                    dbfs: -54.0,
                })
                .collect();
            f.features = Some(Arc::new(AudioFeatures::new(&f.bins, f.mix_rms, f.rms)));
            let mut p = Processor::default();
            let start = Instant::now();
            for i in 1..=10000 {
                f.sequence = i;
                std::hint::black_box(p.step(&b, &f, i as f64 / 120.0, true));
            }
            eprintln!(
                "{count} shared channels, cached 8192 FFT: {:.3} ms/step",
                start.elapsed().as_secs_f64() * 1000.0 / 10000.0
            );
        }
    }
}

#[cfg(all(test, feature = "development"))]
mod worker_tests {
    use super::*;
    #[test]
    fn generated_audio_drives_worker_without_any_ui_or_visualizer() {
        let engine = crate::audio::AudioEngine::development_mode(true);
        let runtime = Runtime::start(
            engine.analysis().snapshot.clone(),
            Arc::new(ArcSwap::from_pointee(crate::midi::Snapshot::default())),
        );
        runtime.publish(&Bank::test_sources());
        let deadline = Instant::now() + Duration::from_secs(3);
        while runtime
            .snapshot
            .load()
            .values
            .get(&1)
            .is_none_or(|s| s.value <= 0.05)
        {
            assert!(
                Instant::now() < deadline,
                "shared worker did not receive audio"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let wait_for = |predicate: &dyn Fn(&Snapshot) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !predicate(&runtime.snapshot.load()) {
                assert!(Instant::now() < deadline, "signal worker did not advance");
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        let before = runtime.snapshot.load().time;
        wait_for(&|snapshot| snapshot.time > before);
        runtime.connected.store(false, Ordering::Relaxed);
        wait_for(&|snapshot| snapshot.values[&1].status != Status::Live);
        runtime.connected.store(true, Ordering::Relaxed);
        wait_for(&|snapshot| snapshot.values[&1].status == Status::Live);
    }
}

#[cfg(test)]
mod session_rebinding_tests {
    use super::*;
    fn wait_for(runtime: &Runtime, predicate: impl Fn(&Snapshot) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !predicate(&runtime.snapshot.load()) {
            assert!(
                Instant::now() < deadline,
                "signal snapshot did not follow the current capture session"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn frame(amplitude: f32) -> AnalysisFrame {
        AnalysisFrame {
            sequence: 1,
            mix_rms: amplitude,
            rms: [amplitude; 2],
            ..Default::default()
        }
    }
    #[test]
    fn follows_new_capture_runtime_even_when_epoch_and_sequence_repeat() {
        let startup = Arc::new(ArcSwap::from_pointee(AnalysisFrame::default()));
        let runtime = Runtime::start(
            startup,
            Arc::new(ArcSwap::from_pointee(crate::midi::Snapshot::default())),
        );
        runtime.publish(&Bank::test_sources());
        let first = Arc::new(ArcSwap::from_pointee(frame(0.5)));
        runtime.bind_audio(first.clone());
        wait_for(&runtime, |s| {
            s.values
                .get(&1)
                .is_some_and(|v| v.status == Status::Live && v.value > 0.5 && v.level > 0.5)
        });
        // CoreAudio recreates AnalysisRuntime on reconnect; metadata starts from the same IDs.
        let second = Arc::new(ArcSwap::from_pointee(frame(0.0)));
        runtime.bind_audio(second.clone());
        wait_for(&runtime, |s| {
            s.values
                .get(&1)
                .is_some_and(|v| v.status == Status::Live && v.level == 0.0 && v.value == 0.0)
        });
        first.store(Arc::new(AnalysisFrame {
            sequence: 2,
            ..frame(1.0)
        }));
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(runtime.snapshot.load().values[&1].level, 0.0);
        second.store(Arc::new(AnalysisFrame {
            sequence: 2,
            ..frame(0.4)
        }));
        wait_for(&runtime, |s| {
            s.values[&1].value > 0.1 && s.values[&1].level > 0.5
        });
    }
    #[test]
    fn centered_stereo_has_input_activity_with_a_zero_balance_output() {
        let mut processor = Processor::default();
        let bank = Bank {
            slots: Vec::new(),
            channels: vec![Definition::recipe(1, Method::Balance)],
            next_id: 2,
        };
        let snapshot = processor.step(&bank, &frame(0.5), 0.01, true);
        assert!(snapshot.values[&1].level > 0.5);
        assert_eq!(snapshot.values[&1].value, 0.0);
    }
}

#[cfg(test)]
mod lfo_tests {
    use super::*;
    #[test]
    fn lfo_runs_without_audio_and_rate_edits_and_capture_changes_preserve_phase() {
        let mut bank = Bank::default();
        let id = bank.add(Method::Lfo).unwrap();
        let d = bank.channels.iter_mut().find(|d| d.id == id).unwrap();
        d.lfo.rate = 1.0;
        let mut processor = Processor::default();
        let mut frame = AnalysisFrame::default();
        let mut sample = Sample::default();
        for i in 0..=5 {
            sample = processor.step(&bank, &frame, i as f64 * 0.05, false).values[&id].clone();
        }
        assert_eq!(sample.status, Status::Live);
        assert_eq!(sample.availability, 1.0);
        assert!((sample.value - 1.0).abs() < 0.001);
        bank.channels
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap()
            .lfo
            .rate = 2.0;
        frame.capture_epoch = 99;
        let same = processor.step(&bank, &frame, 0.25, false).values[&id].value;
        assert!(
            (same - sample.value).abs() < 0.001,
            "editing rate or rebinding audio jumped phase"
        );
        let next = processor.step(&bank, &frame, 0.30, false).values[&id].value;
        assert!((next - 0.809017).abs() < 0.001);
    }
    #[test]
    fn solo_is_shared_by_all_mappings_allows_multiple_channels_and_preserves_mute() {
        let mut bank = Bank::default();
        let first = bank.add(Method::Lfo).unwrap();
        let second = bank.add(Method::Lfo).unwrap();
        let third = bank.add(Method::Lfo).unwrap();
        for d in &mut bank.channels {
            d.lfo.bipolar = false;
        }
        let frame = AnalysisFrame::default();
        let mut processor = Processor::default();
        processor.step(&bank, &frame, 0.0, false);
        bank.channels[0].solo = true;
        for i in 1..=200 {
            processor.step(&bank, &frame, i as f64 * 0.01, false);
        }
        let solo = processor.step(&bank, &frame, 2.01, false);
        assert_eq!(solo.values[&first].status, Status::Live);
        for id in [second, third] {
            let sample = &solo.values[&id];
            assert_eq!(sample.status, Status::Muted);
            assert_eq!(sample.value, 0.0);
            assert_eq!(sample.availability, 0.0);
            assert_eq!(
                sample.mapped(&Route {
                    output_min: 0.4,
                    ..Default::default()
                }),
                0.0
            );
        }
        bank.channels[1].solo = true;
        let pair = processor.step(&bank, &frame, 2.02, false);
        assert_eq!(pair.values[&first].status, Status::Live);
        assert_eq!(pair.values[&second].status, Status::Live);
        assert_eq!(pair.values[&third].status, Status::Muted);
        bank.channels[0].enabled = false;
        let muted = processor.step(&bank, &frame, 2.03, false);
        assert_eq!(muted.values[&first].status, Status::Muted);
        for d in &mut bank.channels {
            d.solo = false;
        }
        let restored = processor.step(&bank, &frame, 2.04, false);
        assert_eq!(restored.values[&first].status, Status::Muted);
        assert_eq!(restored.values[&second].status, Status::Live);
        assert_eq!(restored.values[&third].status, Status::Live);
        bank.channels[1].solo = true;
        let loaded: Bank = serde_json::from_slice(&serde_json::to_vec(&bank).unwrap()).unwrap();
        assert_eq!(loaded, bank);
        let mut legacy = serde_json::to_value(Definition::recipe(1, Method::Band)).unwrap();
        legacy.as_object_mut().unwrap().remove("solo");
        assert!(!serde_json::from_value::<Definition>(legacy).unwrap().solo);
    }

    #[test]
    fn lfo_waveforms_have_declared_polarity_and_mute_fades_all_influence() {
        for waveform in Waveform::ALL {
            let mut bank = Bank::default();
            let id = bank.add(Method::Lfo).unwrap();
            let d = bank.channels.iter_mut().find(|d| d.id == id).unwrap();
            d.lfo.waveform = waveform;
            d.lfo.bipolar = false;
            d.lfo.depth = 0.7;
            let mut processor = Processor::default();
            let frame = AnalysisFrame::default();
            for i in 0..=100 {
                let sample =
                    processor.step(&bank, &frame, i as f64 * 0.01, false).values[&id].clone();
                assert!((0.0..=0.7).contains(&sample.value));
                assert_eq!(
                    bank.channels.iter().find(|d| d.id == id).unwrap().kind(),
                    OutputKind::Unipolar
                );
            }
            bank.channels
                .iter_mut()
                .find(|d| d.id == id)
                .unwrap()
                .enabled = false;
            for i in 101..=300 {
                processor.step(&bank, &frame, i as f64 * 0.01, false);
            }
            let muted = processor.step(&bank, &frame, 3.01, false).values[&id].clone();
            assert_eq!(muted.value, 0.0);
            assert_eq!(muted.availability, 0.0);
            bank.channels
                .iter_mut()
                .find(|d| d.id == id)
                .unwrap()
                .enabled = true;
            assert_eq!(
                processor.step(&bank, &frame, 3.02, false).values[&id].status,
                Status::Live
            );
        }
    }
    #[test]
    fn old_signal_files_default_lfo_and_invalid_lfos_are_rejected() {
        let mut json = serde_json::to_value(Definition::recipe(1, Method::Band)).unwrap();
        json.as_object_mut().unwrap().remove("lfo");
        let old: Definition = serde_json::from_value(json).unwrap();
        assert_eq!(old.lfo, Lfo::default());
        let mut converted = Definition::recipe(2, Method::BassHits);
        converted.input = Input::Lfo;
        converted.method = Method::Lfo;
        converted.lfo.bipolar = false;
        assert_eq!(converted.kind(), OutputKind::Unipolar);
        let mut bank = Bank::default();
        let id = bank.add(Method::Lfo).unwrap();
        assert!(bank.valid());
        bank.channels
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap()
            .lfo
            .rate = f32::NAN;
        assert!(!bank.valid());
        bank.channels
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap()
            .sanitize();
        assert!(bank.valid());
    }
}
