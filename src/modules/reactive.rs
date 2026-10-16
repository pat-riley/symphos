//! Shared energy mapping for artistic visualizers; independent of audio capture.
use crate::analysis::AnalysisFrame;

pub(super) fn energy(frame: &AnalysisFrame, sensitivity: f32, gate_db: f32) -> [f32; 4] {
    let mut powers = [0.0_f32; 3];
    for bin in &frame.bins {
        if !bin.magnitude.is_finite() || bin.frequency_hz < 20.0 || bin.frequency_hz > 20_000.0 {
            continue;
        }
        let band = if bin.frequency_hz < 250.0 {
            0
        } else if bin.frequency_hz < 4000.0 {
            1
        } else {
            2
        };
        powers[band] += bin.magnitude.powi(2);
    }
    let response = |amplitude: f32| {
        if !amplitude.is_finite() || amplitude <= 0.0 {
            return 0.0;
        }
        let db = 20.0 * amplitude.log10();
        if db <= gate_db {
            return 0.0;
        }
        // dB mapping retains response to quiet music; multiplying by distance
        // above the gate avoids a visible step when the gate opens.
        let level = ((db - gate_db) / (-6.0 - gate_db)).clamp(0.0, 1.0);
        (level * level * sensitivity).clamp(0.0, 1.0)
    };
    [
        response(powers[0].sqrt()),
        response(powers[1].sqrt()),
        response(powers[2].sqrt()),
        response(frame.mix_rms),
    ]
}
