//! Upgrade saved detector channels in place: stable channel IDs keep every mapping intact.
use super::*;
use inserts::{Chain, Detection, Detector, Effect};
impl Bank {
    pub fn add_source(&mut self, input: Input) -> Option<u64> {
        let method = match input {
            Input::System => Method::Intensity,
            Input::Lfo => Method::Lfo,
            Input::Midi => Method::Midi,
            _ => return None,
        };
        let id = self.add(method)?;
        let d = self.channels.last_mut().unwrap();
        d.name = match input {
            Input::System => "System audio",
            Input::Lfo => "LFO",
            _ => "MIDI",
        }
        .into();
        d.inserts = Some(Chain::default());
        self.sync_audio_channels();
        self.sync_slots();
        Some(id)
    }
    /// Idempotent. Runtime state is never serialized or migrated.
    pub fn migrate_inserts(&mut self) {
        // Old audio-only branches become complete channels without adding a mixer slot.
        for bus in &mut self.audio_inputs {
            if self.channels.iter().any(|d| d.id == bus.id) {
                continue;
            }
            let mut d = Definition::recipe(bus.id, Method::Intensity);
            d.name = bus.name.clone();
            d.color = bus.color;
            d.enabled = bus.enabled;
            d.solo = bus.solo;
            let mut chain = Chain::default();
            if bus.channels != audio::Channels::Stereo {
                chain.add(
                    Input::System,
                    chain.stages.len(),
                    Effect::Channels {
                        channels: bus.channels,
                    },
                );
            }
            if bus.gain_db != 0.0 {
                chain.add(
                    Input::System,
                    chain.stages.len(),
                    Effect::AudioGain { db: bus.gain_db },
                );
            }
            if bus.high_pass {
                chain.add(
                    Input::System,
                    chain.stages.len(),
                    Effect::HighPass { hz: bus.low_hz },
                );
            }
            if bus.low_pass {
                chain.add(
                    Input::System,
                    chain.stages.len(),
                    Effect::LowPass { hz: bus.high_hz },
                );
            }
            if bus.gate {
                chain.add(
                    Input::System,
                    chain.stages.len(),
                    Effect::AudioGate { db: bus.gate_db },
                );
            }
            d.inserts = Some(chain);
            self.channels.push(d);
            *bus = audio::InputBus {
                id: bus.id,
                name: bus.name.clone(),
                color: bus.color,
                solo: bus.solo,
                enabled: bus.enabled,
                ..Default::default()
            };
        }
        for d in &mut self.channels {
            if d.inserts.is_some() {
                continue;
            }
            let mut chain = Chain::default();
            if d.input == Input::System {
                let detector = match d.method {
                    Method::Intensity => Detector::Level,
                    Method::Band => Detector::Band,
                    Method::BassHits => Detector::Hits,
                    Method::SpectralChange => Detector::Change,
                    Method::Balance => Detector::Balance,
                    _ => Detector::Level,
                };
                let mut response = d.response.clone();
                // These route fields were ignored by the legacy signal engine.
                response.amount = 1.0;
                response.output_min = 0.0;
                response.output_max = 1.0;
                response.signal = None;
                response.invert = false;
                chain.add(
                    d.input,
                    0,
                    Effect::Detect(Detection {
                        detector,
                        response,
                        adaptive: d.adaptive,
                    }),
                );
            }
            if d.normalization.enabled {
                chain.add(
                    d.input,
                    chain.stages.len(),
                    Effect::Normalize {
                        low: d.normalization.low,
                        high: d.normalization.high,
                        automatic: false,
                    },
                );
            }
            if d.shaping.enabled {
                chain.add(
                    d.input,
                    chain.stages.len(),
                    Effect::Shape(d.shaping.clone()),
                );
            }
            if d.smoothing_ms > 0.0 {
                chain.add(
                    d.input,
                    chain.stages.len(),
                    Effect::Smooth { ms: d.smoothing_ms },
                );
            }
            d.normalization = Default::default();
            d.shaping = Default::default();
            d.smoothing_ms = 0.0;
            d.inserts = Some(chain);
        }
        self.sync_audio_channels();
        self.sync_slots();
    }
    pub fn sync_audio_channels(&mut self) {
        for d in self
            .channels
            .iter()
            .filter(|d| d.input == Input::System && d.inserts.is_some())
        {
            if let Some(bus) = self.audio_inputs.iter_mut().find(|b| b.id == d.id) {
                bus.name = d.name.clone();
                bus.color = d.color;
                bus.enabled = d.enabled;
                bus.solo = d.solo;
            } else {
                self.audio_inputs.push(audio::InputBus {
                    id: d.id,
                    name: d.name.clone(),
                    color: d.color,
                    enabled: d.enabled,
                    solo: d.solo,
                    ..Default::default()
                });
            }
        }
    }
}
