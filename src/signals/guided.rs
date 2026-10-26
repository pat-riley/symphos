//! Guided editing is a view of existing chains, never a second processing model.
use super::{
    Definition, Input,
    inserts::{Detection, Detector, Effect},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Audio,
    Loudness,
    Bass,
    Band,
    Hits,
    Change,
    Balance,
    Beat,
}
impl Kind {
    pub const ALL: [Self; 8] = [
        Self::Loudness,
        Self::Bass,
        Self::Band,
        Self::Hits,
        Self::Change,
        Self::Balance,
        Self::Beat,
        Self::Audio,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Audio => "Audio only",
            Self::Loudness => "Loudness",
            Self::Bass => "Bass Energy",
            Self::Band => "Band Energy",
            Self::Hits => "Bass Hits",
            Self::Change => "Spectral Change",
            Self::Balance => "Stereo Balance",
            Self::Beat => "Beat",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Audio => {
                "Waveform and spectrum for visualizers. Choose a detection type to also drive a macro or visual parameter."
            }
            Self::Loudness => {
                "Follows the overall strength of the music. Useful for brightness, size and gentle movement. This is an RMS-based visual control, not a LUFS measurement."
            }
            Self::Bass => {
                "Follows sustained low-frequency energy. A held bass note stays active instead of producing repeated hits."
            }
            Self::Band => {
                "Follows energy within a frequency range you choose. Useful for giving different parts of the music their own movement."
            }
            Self::Hits => {
                "Produces pulses when bass energy rises suddenly. Useful for kicks and accents; it does not isolate a drum from a mix."
            }
            Self::Change => {
                "Follows new spectral activity. Attacks and changing textures respond strongly; steady sounds settle down."
            }
            Self::Balance => {
                "Follows the stereo balance: left is negative, center is zero, and right is positive."
            }
            Self::Beat => {
                "Estimates rhythmic hits or beat phase. Use manual tempo when the music has no clear pulse; confidence shows how stable the estimate is."
            }
        }
    }
    pub fn effect(self) -> Option<Effect> {
        let detector = match self {
            Self::Audio => return None,
            Self::Beat => {
                return Some(Effect::Beat {
                    sensitivity: 1.5,
                    manual: false,
                    bpm: 120.0,
                    output: super::inserts::BeatOutput::Pulse,
                });
            }
            Self::Loudness => Detector::Level,
            Self::Bass => Detector::Bass,
            Self::Band => Detector::Band,
            Self::Hits => Detector::Hits,
            Self::Change => Detector::Change,
            Self::Balance => Detector::Balance,
        };
        Some(Effect::Detect(Detection::new(detector)))
    }
    pub fn from_effect(effect: &Effect) -> Option<Self> {
        match effect {
            Effect::Detect(d) => Some(match d.detector {
                Detector::Level => Self::Loudness,
                Detector::Bass => Self::Bass,
                Detector::Band => Self::Band,
                Detector::Hits => Self::Hits,
                Detector::Change => Self::Change,
                Detector::Balance => Self::Balance,
                Detector::Beat => Self::Beat,
            }),
            Effect::Beat { .. } => Some(Self::Beat),
            _ => None,
        }
    }
}
impl Definition {
    pub fn detection_index(&self) -> Option<usize> {
        self.inserts
            .as_ref()?
            .stages
            .iter()
            .position(|s| Kind::from_effect(&s.effect).is_some())
    }
    pub fn signal_kind(&self) -> Kind {
        self.detection_index()
            .and_then(|i| Kind::from_effect(&self.inserts.as_ref()?.stages[i].effect))
            .unwrap_or(Kind::Audio)
    }
    /// Change only the detector; retain IDs, audio preprocessing and downstream controls.
    /// Audio-only conversion is deliberately handled by Advanced's explicit removal action.
    pub fn set_signal_kind(&mut self, kind: Kind) -> bool {
        if self.input != Input::System {
            return false;
        }
        let previous = self.signal_kind();
        if previous == kind {
            return true;
        }
        let index = self.detection_index();
        let Some(effect) = kind.effect() else {
            return false;
        };
        let Some(chain) = &mut self.inserts else {
            return false;
        };
        if let Some(index) = index {
            chain.stages[index].effect = effect;
        } else if chain.add(self.input, chain.stages.len(), effect).is_none() {
            return false;
        }
        if self.name == previous.label() || self.name == "System audio" {
            self.name = kind.label().into();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals::{Bank, inserts::Chain};
    #[test]
    fn guided_types_use_valid_existing_processors_and_preserve_serialization() {
        for kind in Kind::ALL {
            let mut bank = Bank::default();
            let id = bank.add_source(Input::System).unwrap();
            let d = bank.channels.iter_mut().find(|d| d.id == id).unwrap();
            assert!(d.set_signal_kind(kind));
            assert_eq!(d.signal_kind(), kind);
            assert_eq!(d.has_control_output(), kind != Kind::Audio);
            bank.sync_audio_channels();
            assert!(bank.valid());
            let bytes = serde_json::to_vec(&bank).unwrap();
            let restored: Bank = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(restored, bank);
        }
    }
    #[test]
    fn changing_type_keeps_custom_chain_identity_name_routing_and_controls() {
        let mut d = Definition::recipe(42, super::super::Method::Intensity);
        d.name = "My kick".into();
        d.inserts = Some(Chain::default());
        let chain = d.inserts.as_mut().unwrap();
        chain.add(Input::System, 0, Effect::HighPass { hz: 50.0 });
        assert!(d.set_signal_kind(Kind::Bass));
        d.inserts
            .as_mut()
            .unwrap()
            .add(Input::System, 2, Effect::Invert);
        let before = d.clone();
        assert!(d.set_signal_kind(Kind::Hits));
        assert_eq!(d.id, before.id);
        assert_eq!(d.name, before.name);
        assert_eq!(d.audio_input, before.audio_input);
        let chain = d.inserts.as_ref().unwrap();
        assert_eq!(chain.stages[0], before.inserts.as_ref().unwrap().stages[0]);
        assert_eq!(
            chain.stages[1].id,
            before.inserts.as_ref().unwrap().stages[1].id
        );
        assert_eq!(chain.stages[2], before.inserts.as_ref().unwrap().stages[2]);
        assert!(chain.valid(Input::System));
        let before = d.clone();
        assert!(!d.set_signal_kind(Kind::Audio));
        assert_eq!(d, before, "never discard dependent processors implicitly");
    }
    #[test]
    fn selecting_current_type_and_inspecting_custom_chains_are_lossless() {
        let mut bank = Bank::default();
        let id = bank.add_source(Input::System).unwrap();
        let d = bank.channels.iter_mut().find(|d| d.id == id).unwrap();
        d.set_signal_kind(Kind::Loudness);
        if let Effect::Detect(config) = &mut d.inserts.as_mut().unwrap().stages[0].effect {
            config.response.envelope.attack = 333.0;
            config.adaptive = false;
        }
        let before = d.clone();
        assert!(d.set_signal_kind(Kind::Loudness));
        assert_eq!(d, &before);
        assert_eq!(d.signal_kind(), Kind::Loudness);
        assert_eq!(d, &before);
    }
}
