//! Stateless output transfer, shared by audio, LFO and MIDI signals. Keeping this
//! between calibration and the final smoother gives predictable common behavior.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shaping {
    pub enabled: bool,
    pub exponent: f32,
    pub dead_zone: f32,
    pub steps: u32,
    pub invert: bool,
}
impl Default for Shaping {
    fn default() -> Self {
        Self {
            enabled: false,
            exponent: 1.0,
            dead_zone: 0.0,
            steps: 0,
            invert: false,
        }
    }
}
impl Shaping {
    pub fn valid(&self) -> bool {
        self.exponent.is_finite()
            && (0.1..=8.0).contains(&self.exponent)
            && self.dead_zone.is_finite()
            && (0.0..=0.95).contains(&self.dead_zone)
            && self.steps <= 128
    }
    pub fn sanitize(&mut self) {
        if !self.exponent.is_finite() {
            self.exponent = 1.0;
        }
        if !self.dead_zone.is_finite() {
            self.dead_zone = 0.0;
        }
        self.exponent = self.exponent.clamp(0.1, 8.0);
        self.dead_zone = self.dead_zone.clamp(0.0, 0.95);
        self.steps = self.steps.min(128);
    }
    pub fn map(&self, input: f32, bipolar: bool) -> f32 {
        if !self.enabled {
            return input;
        }
        let mut value = ((input.abs() - self.dead_zone) / (1.0 - self.dead_zone))
            .clamp(0.0, 1.0)
            .powf(self.exponent);
        if self.steps >= 2 {
            let intervals = (self.steps - 1) as f32;
            value = (value * intervals).round() / intervals;
        }
        if bipolar {
            value *= input.signum();
            if self.invert { -value } else { value }
        } else if self.invert {
            1.0 - value
        } else {
            value
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bypass_preserves_range_and_signed_curve_keeps_center() {
        let mut s = Shaping::default();
        assert_eq!(s.map(1.8, false), 1.8);
        s.enabled = true;
        s.exponent = 2.0;
        assert_eq!(s.map(-0.5, true), -0.25);
        assert_eq!(s.map(0.0, true), 0.0);
        s.invert = true;
        assert_eq!(s.map(-0.5, true), 0.25);
        assert_eq!(s.map(0.5, false), 0.75);
    }
    #[test]
    fn dead_zone_and_steps_keep_endpoints() {
        let s = Shaping {
            enabled: true,
            dead_zone: 0.2,
            steps: 3,
            ..Default::default()
        };
        assert_eq!(s.map(0.15, false), 0.0);
        assert_eq!(s.map(0.6, false), 0.5);
        assert_eq!(s.map(1.0, false), 1.0);
        assert_eq!(s.map(-1.0, true), -1.0);
    }
}
