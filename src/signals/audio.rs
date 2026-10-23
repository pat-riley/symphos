//! Typed waveform/spectrum inputs, processed once per capture on the Signals worker.
use super::*;
use crate::analysis::{AnalysisSettings, Analyzer, StereoSample};
use crate::capture_history::CaptureHistory;
use crate::help::HoverHelp;
use std::sync::Mutex;

pub const MAX_INPUTS: usize = super::MAX_SIGNALS;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Connection {
    #[default]
    Main,
    Input(u64),
    Off,
}
const DEFAULT_SOURCE_KEY: &str = "default-audio-source-label";

pub fn default_source_label(ctx: &egui::Context) -> String {
    ctx.data(|data| data.get_temp::<String>(egui::Id::new(DEFAULT_SOURCE_KEY)))
        .unwrap_or_else(|| "System audio".into())
}

pub fn set_default_source_label(ctx: &egui::Context, label: String) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(DEFAULT_SOURCE_KEY), label));
}

pub fn connection_picker(ui: &mut egui::Ui, value: &mut Connection, buses: &[InputBus]) {
    let main = default_source_label(ui.ctx());
    let label = match value {
        Connection::Main => main.as_str(),
        Connection::Off => "Unassigned",
        Connection::Input(id) => buses
            .iter()
            .find(|b| b.id == *id)
            .map_or("Unavailable", |b| b.name.as_str()),
    };
    crate::dropdown::ComboBox::from_id_salt("audio-input-connection").width(ui.available_width().min(240.0)).selected_text(label).show_ui(ui, |ui| {
        ui.weak("Always available");
        ui.selectable_value(value, Connection::Main, &main);
        ui.separator();
        if !buses.is_empty() {
            ui.weak("Scene signals");
            for bus in buses { ui.selectable_value(value, Connection::Input(bus.id), &bus.name); }
            ui.separator();
        }
        ui.selectable_value(value, Connection::Off, "Unassigned");
    }).response.help_text("Choose the waveform, spectrum and measurements feeding this consumer. Unassigned disables built-in audio response; separate modulation mappings continue.");
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channels {
    #[default]
    Stereo,
    Left,
    Right,
    Mid,
    Side,
}
impl Channels {
    pub const ALL: [Self; 5] = [Self::Stereo, Self::Left, Self::Right, Self::Mid, Self::Side];
    pub fn label(self) -> &'static str {
        match self {
            Self::Stereo => "Stereo",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Mid => "Mid",
            Self::Side => "Side",
        }
    }
}
impl Connection {
    pub fn valid(self) -> bool {
        !matches!(self, Self::Input(0))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputBus {
    pub color: Option<[u8; 3]>,
    pub solo: bool,
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub channels: Channels,
    pub gain_db: f32,
    pub high_pass: bool,
    pub low_hz: f32,
    pub low_pass: bool,
    pub high_hz: f32,
    pub gate: bool,
    pub gate_db: f32,
}
impl Default for InputBus {
    fn default() -> Self {
        Self {
            color: None,
            solo: false,
            id: 0,
            name: "Audio input".into(),
            enabled: true,
            channels: Channels::Stereo,
            gain_db: 0.0,
            high_pass: false,
            low_hz: 80.0,
            low_pass: false,
            high_hz: 4000.0,
            gate: false,
            gate_db: -60.0,
        }
    }
}
impl InputBus {
    pub fn color_rgb(&self) -> [u8; 3] {
        self.color
            .unwrap_or(super::CHANNEL_COLORS[(self.id.saturating_sub(1) % 8) as usize])
    }
    fn same_history(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && self.channels == other.channels
            && self.high_pass == other.high_pass
            && self.low_hz == other.low_hz
            && self.low_pass == other.low_pass
            && self.high_hz == other.high_hz
            && self.gate == other.gate
            && self.gate_db == other.gate_db
    }
    pub fn valid(&self) -> bool {
        crate::workspace::valid_name(&self.name)
            && (-48.0..=36.0).contains(&self.gain_db)
            && (20.0..=20000.0).contains(&self.low_hz)
            && (20.0..=20000.0).contains(&self.high_hz)
            && (!self.high_pass || !self.low_pass || self.low_hz < self.high_hz)
            && (-100.0..=0.0).contains(&self.gate_db)
    }
}

pub struct Feed {
    pub frame: Arc<AnalysisFrame>,
    pub history: Arc<Mutex<CaptureHistory>>,
    pub revision: u64,
}
impl std::fmt::Debug for Feed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Feed")
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}
struct State {
    definition: InputBus,
    settings: Arc<AnalysisSettings>,
    epoch: u64,
    sequence: u64,
    source_bins: Arc<[crate::analysis::FrequencyBin]>,
    analyzer: Option<Analyzer>,
    feed: Arc<Feed>,
}
#[derive(Default)]
pub struct Inputs {
    states: BTreeMap<u64, State>,
    revision: u64,
}
impl Inputs {
    pub fn clear(&mut self) {
        self.states.clear();
    }
    pub fn process(
        &mut self,
        buses: &[InputBus],
        source: &AnalysisFrame,
    ) -> BTreeMap<u64, Arc<Feed>> {
        self.states
            .retain(|id, _| buses.iter().any(|b| b.id == *id && b.valid()));
        let now = Instant::now();
        let solo = buses.iter().any(|b| b.solo);
        for bus in buses.iter().take(MAX_INPUTS).filter(|b| b.valid()) {
            let mut effective = bus.clone();
            effective.enabled &= !solo || bus.solo;
            let bus = &effective;
            let replace = self.states.get(&bus.id).is_none_or(|s| {
                !s.definition.same_history(bus)
                    || s.epoch != source.capture_epoch
                    || s.feed.frame.sample_rate != source.sample_rate
                    || *s.settings != *source.analysis_settings
                    || s.feed.frame.fft_size != source.fft_size
            });
            if replace {
                self.revision = self.revision.wrapping_add(1);
                let mut history = CaptureHistory::default();
                history.reset(source.capture_epoch);
                self.states.insert(
                    bus.id,
                    State {
                        definition: bus.clone(),
                        settings: source.analysis_settings.clone(),
                        epoch: source.capture_epoch,
                        sequence: u64::MAX,
                        source_bins: source.bins.clone(),
                        analyzer: None,
                        feed: Arc::new(Feed {
                            frame: Arc::new(silence(source)),
                            history: Arc::new(Mutex::new(history)),
                            revision: self.revision,
                        }),
                    },
                );
            }
            let state = self.states.get_mut(&bus.id).unwrap();
            // Gain is a live level edit, not a new source. Recompute the current
            // frame without replacing the archive or its consumer revision.
            // Upstream gains can also change an insert's input at the same sequence.
            if state.sequence == source.sequence
                && state.definition.gain_db == bus.gain_db
                && Arc::ptr_eq(&state.source_bins, &source.bins)
            {
                continue;
            }
            state.sequence = source.sequence;
            state.definition.gain_db = bus.gain_db;
            state.source_bins = source.bins.clone();
            let frame = if !bus.enabled || source.sequence == 0 {
                silence(source)
            } else if bus.channels == Channels::Stereo
                && bus.gain_db == 0.0
                && !bus.high_pass
                && !bus.low_pass
                && !bus.gate
            {
                source.clone()
            } else {
                // Replaying the bounded PCM window supplies filter pre-roll before
                // the latest FFT window. This never runs in capture or rendering.
                let mut samples: Vec<StereoSample> = source
                    .waveform_left
                    .iter()
                    .zip(source.waveform_right.iter())
                    .map(|(&l, &r)| match bus.channels {
                        Channels::Stereo => [l, r],
                        Channels::Left => [l, l],
                        Channels::Right => [r, r],
                        Channels::Mid => [(l + r) * 0.5; 2],
                        Channels::Side => [(l - r) * 0.5; 2],
                    })
                    .collect();
                samples.resize(samples.len().max(source.fft_size), [0.0; 2]);
                let gain = 10.0_f32.powf(bus.gain_db / 20.0);
                let hp = Biquad::new(bus.low_hz, source.sample_rate, true);
                let lp = Biquad::new(bus.high_hz, source.sample_rate, false);
                let mut state_hp = [[0.0; 2]; 2];
                let mut state_lp = [[0.0; 2]; 2];
                for sample in &mut samples {
                    for c in 0..2 {
                        let mut v = sample[c] * gain;
                        if bus.high_pass {
                            v = hp.process(v, &mut state_hp[c]);
                        }
                        if bus.low_pass {
                            v = lp.process(v, &mut state_lp[c]);
                        }
                        sample[c] = v;
                    }
                }
                if bus.gate {
                    let rms = (samples
                        .iter()
                        .rev()
                        .take(source.fft_size)
                        .map(|s| (f64::from(s[0]).powi(2) + f64::from(s[1]).powi(2)) * 0.5)
                        .sum::<f64>()
                        / source.fft_size as f64)
                        .sqrt();
                    if rms < 10.0_f64.powf(f64::from(bus.gate_db) / 20.0) {
                        samples.fill([0.0; 2]);
                    }
                }
                let mut frame = state
                    .analyzer
                    .get_or_insert_with(|| {
                        Analyzer::new(source.fft_size, source.analysis_settings.window)
                    })
                    .analyze(
                        &samples,
                        0,
                        source.sample_rate,
                        source.channels,
                        &source.analysis_settings,
                        source.sequence,
                        source.dropped_samples,
                    );
                frame.capture_epoch = source.capture_epoch;
                frame
            };
            let history = state.feed.history.clone();
            history
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(&frame, now);
            state.feed = Arc::new(Feed {
                frame: Arc::new(frame),
                history,
                revision: state.feed.revision,
            });
        }
        self.states
            .iter()
            .map(|(id, state)| (*id, state.feed.clone()))
            .collect()
    }
}

pub fn silence(source: &AnalysisFrame) -> AnalysisFrame {
    AnalysisFrame {
        analysis_settings: source.analysis_settings.clone(),
        capture_epoch: source.capture_epoch,
        sequence: source.sequence,
        sample_rate: source.sample_rate,
        channels: source.channels,
        fft_size: source.fft_size,
        waveform_left: vec![0.0; source.waveform_left.len()].into(),
        waveform_right: vec![0.0; source.waveform_right.len()].into(),
        bins: source
            .bins
            .iter()
            .map(|b| crate::analysis::FrequencyBin {
                frequency_hz: b.frequency_hz,
                magnitude: 0.0,
                dbfs: -140.0,
            })
            .collect(),
        ..Default::default()
    }
}

struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
}

impl Biquad {
    fn new(hz: f32, rate: u32, high: bool) -> Self {
        let w = std::f32::consts::TAU * hz.min(rate as f32 * 0.45) / rate.max(1) as f32;
        let c = w.cos();
        let alpha = w.sin() / 2.0_f32.sqrt();
        let a0 = 1.0 + alpha;
        let b = if high {
            [(1.0 + c) * 0.5, -(1.0 + c), (1.0 + c) * 0.5]
        } else {
            [(1.0 - c) * 0.5, 1.0 - c, (1.0 - c) * 0.5]
        };
        Self {
            b: b.map(|v| v / a0),
            a: [-2.0 * c / a0, (1.0 - alpha) / a0],
        }
    }
    fn process(&self, x: f32, s: &mut [f32; 2]) -> f32 {
        let y = self.b[0] * x + s[0];
        s[0] = self.b[1] * x - self.a[0] * y + s[1];
        s[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

#[cfg(test)]
mod tests;
