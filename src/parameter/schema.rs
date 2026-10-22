//! Stable parameter addresses and numeric behavior shared by inspectors, macros and MIDI.
use crate::modules::ModuleKind;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterContext {
    pub pane: String,
    pub kind: ModuleKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterSpec {
    pub field: String,
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub safe_min: f64,
    pub safe_max: f64,
    pub default: f64,
    pub integer: bool,
    pub logarithmic: bool,
    pub scale: f64,
}
impl ParameterSpec {
    pub fn valid(&self) -> bool {
        !self.field.is_empty()
            && self.field.len() <= 120
            && !self.label.is_empty()
            && self.label.len() <= 120
            && [
                self.min,
                self.max,
                self.safe_min,
                self.safe_max,
                self.default,
                self.scale,
            ]
            .iter()
            .all(|v| v.is_finite())
            && self.min < self.max
            && self.safe_min <= self.min
            && self.safe_max >= self.max
            && self.safe_min >= -384000.0
            && self.safe_max <= 384000.0
            && self.scale > 0.0
            && (!self.logarithmic || self.min > 0.0)
    }
    pub fn normalized(&self, value: f64) -> f32 {
        if self.logarithmic {
            ((value.max(self.min) / self.min).ln() / (self.max / self.min).ln()).clamp(0.0, 1.0)
                as f32
        } else {
            ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
        }
    }
    pub fn value(&self, normalized: f32) -> f64 {
        let n = f64::from(normalized);
        let value = if self.logarithmic {
            self.min * (self.max / self.min).powf(n)
        } else {
            self.min + n * (self.max - self.min)
        };
        if self.integer { value.round() } else { value }
    }
}
