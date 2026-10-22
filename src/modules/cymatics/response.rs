use super::*;
use symphos_visuals::cymatics::{GRID, Plate, frequency_hz, sample};
/// Dominant, separated local spectral peaks. This is a reduced-band audio
/// approximation, not an exact reconstruction of phase from FFT magnitudes.
pub(super) fn peaks(frame: &AnalysisFrame, s: &CymaticsSettings, live: bool) -> Vec<(f32, f32)> {
    if !live {
        return Vec::new();
    }
    let floor = 10.0_f32.powf(s.gate_db / 20.0);
    let mut peaks: Vec<_> = frame
        .bins
        .iter()
        .enumerate()
        .filter(|&(i, b)| {
            b.frequency_hz >= s.low_hz
                && b.frequency_hz <= s.high_hz
                && b.magnitude.is_finite()
                && b.magnitude > floor
                && (i == 0 || b.magnitude >= frame.bins[i - 1].magnitude)
                && (i + 1 == frame.bins.len() || b.magnitude >= frame.bins[i + 1].magnitude)
        })
        .map(|(i, b)| {
            if i > 0 && i + 1 < frame.bins.len() {
                let left = frame.bins[i - 1].magnitude.max(1e-12).ln();
                let center = b.magnitude.max(1e-12).ln();
                let right = frame.bins[i + 1].magnitude.max(1e-12).ln();
                let curvature = left - 2.0 * center + right;
                if curvature.is_finite() && curvature < -1e-6 {
                    let offset = (0.5 * (left - right) / curvature).clamp(-0.5, 0.5);
                    let bin_width =
                        (frame.bins[i + 1].frequency_hz - frame.bins[i - 1].frequency_hz) * 0.5;
                    return (
                        (b.frequency_hz + offset * bin_width).clamp(s.low_hz, s.high_hz),
                        (center - 0.25 * (left - right) * offset).exp(),
                    );
                }
            }
            (b.frequency_hz, b.magnitude)
        })
        .collect();
    peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
    let spacing = frame.sample_rate as f32 / frame.fft_size.max(1) as f32 * 1.5;
    let mut selected: Vec<(f32, f32)> = Vec::new();
    for peak in peaks {
        if selected.iter().all(|p| (p.0 - peak.0).abs() > spacing) {
            selected.push(peak);
            if selected.len() == 6 {
                break;
            }
        }
    }
    selected
}
pub(super) fn frequencies(plate: &Plate, s: &CymaticsSettings) -> Vec<f32> {
    let (young, density) = match s.material {
        1 => (200e9, 7850.0),
        2 => (110e9, 8500.0),
        _ => (69e9, 2700.0),
    };
    plate
        .modes
        .iter()
        .map(|m| {
            frequency_hz(
                m.frequency,
                s.width_cm * 0.01,
                s.thickness_mm * 0.001,
                young,
                density,
                s.poisson,
            )
        })
        .collect()
}
pub(super) fn field(
    plate: &Plate,
    s: &CymaticsSettings,
    peaks: &[(f32, f32)],
    time: f32,
) -> Vec<[f32; 4]> {
    let mut out = vec![[0.0; 4]; GRID * GRID];
    if peaks.is_empty() {
        return out;
    }
    let frequencies = frequencies(plate, s);
    let mut excite = egui::vec2(s.excite_x, s.excite_y);
    if plate.spec.circle && excite.length() > 0.95 {
        excite = excite.normalized() * 0.95;
    }
    if s.mode == 3 {
        // Artistic radial interference, with bending-wave dispersion k²=omega/sqrt(D/mass).
        let (frequency, amplitude) = peaks[0];
        let k =
            (frequency / frequencies[0]).sqrt() * plate.modes[0].frequency.sqrt() * s.wave_scale;
        for ((x, y), v) in symphos_visuals::cymatics::grid().zip(&mut out) {
            let mut real = 0.0;
            let mut imag = 0.0;
            for i in 0..s.emitters {
                let angle = i as f32 / s.emitters as f32 * std::f32::consts::TAU;
                let dx = x as f32 - angle.cos() * s.emitter_spacing;
                let dy = (y as f32 - angle.sin() * s.emitter_spacing) * plate.spec.aspect as f32;
                let r = dx.hypot(dy);
                let phase = k * r + i as f32 * s.emitter_phase * std::f32::consts::TAU;
                let a = amplitude * s.gain / (s.emitters as f32 * (0.2 + r).sqrt());
                real += a * phase.cos();
                imag += a * phase.sin();
            }
            v[0] = 0.5 * (real * real + imag * imag);
            v[1] = real * (time * s.display_rate * std::f32::consts::TAU).cos()
                + imag * (time * s.display_rate * std::f32::consts::TAU).sin();
        }
    } else {
        for &(frequency, amplitude) in peaks {
            let coefficients: Vec<_> = plate
                .modes
                .iter()
                .zip(&frequencies)
                .map(|(mode, &natural)| {
                    let r = frequency / natural;
                    let re = 1.0 - r * r;
                    let im = 2.0 * s.damping * r;
                    let denom = (re * re + im * im).max(1e-8);
                    let coupling = sample(&mode.shape, excite.x, excite.y);
                    let a = amplitude * s.gain * coupling * (frequencies[0] / natural).powi(2)
                        / mode.mass;
                    (a * re / denom, -a * im / denom)
                })
                .collect();
            let phase =
                time * s.display_rate * std::f32::consts::TAU * (frequency / frequencies[0]).sqrt();
            for (i, v) in out.iter_mut().enumerate() {
                let mut re = 0.0;
                let mut im = 0.0;
                for (mode, (a, b)) in plate.modes.iter().zip(&coefficients) {
                    re += mode.shape[i] * a;
                    im += mode.shape[i] * b;
                }
                // Different drive frequencies are incoherent; modes at the same
                // drive frequency interfere coherently (including degenerate pairs).
                v[0] += 0.5 * (re * re + im * im);
                v[1] += re * phase.cos() - im * phase.sin();
            }
        }
    }
    if s.auto_sensitivity {
        normalize_visual_response(&mut out, plate, peaks, s.gain);
    }
    for v in &mut out {
        v[0] = v[0].min(1e4);
        v[1] = v[1].tanh();
    }
    out
}
// A display gain applied uniformly to the entire plate, before envelope
// following. This preserves zeros, modal interference and spatial energy ratios.
// Its target follows actual accepted audio amplitude, never the render clock;
// silence is not normalized into movement. Physical amplitudes remain available
// by disabling automatic sensitivity.
fn normalize_visual_response(
    field: &mut [[f32; 4]],
    plate: &Plate,
    peaks: &[(f32, f32)],
    gain: f32,
) {
    let (energy, count) = symphos_visuals::cymatics::grid()
        .zip(field.iter())
        .filter(|((x, y), _)| !plate.spec.circle || x * x + y * y <= 1.0)
        .fold((0.0_f64, 0usize), |(sum, count), (_, v)| {
            (sum + f64::from(v[0]), count + 1)
        });
    let rms = (energy / count.max(1) as f64).sqrt() as f32;
    if rms < 1e-9 {
        return;
    }
    let input = peaks.iter().map(|&(_, a)| a * a).sum::<f32>().sqrt();
    let target = (input * gain).min(2.0);
    let scale = (target / rms).min(1e6);
    for v in field {
        v[0] *= scale * scale;
        v[1] *= scale;
    }
}
pub(super) fn smooth_field(
    current: &mut [[f32; 4]],
    target: &[[f32; 4]],
    s: &CymaticsSettings,
    dt: f32,
) {
    for (v, t) in current.iter_mut().zip(target) {
        let tau = if t[0] > v[0] {
            s.attack_ms
        } else {
            s.release_ms
        } * 0.001;
        let alpha = 1.0 - (-dt / tau).exp();
        v[0] += (t[0] - v[0]) * alpha;
        v[1] += (t[1] - v[1]) * alpha;
    }
    // Gradient of bounded vibration energy avoids excessive grain forces at
    // sharp resonances while preserving all zeros and relative nodal positions.
    for y in 0..GRID {
        for x in 0..GRID {
            let sample = |x: usize, y: usize| {
                let e = current[y * GRID + x][0];
                e / (1.0 + e)
            };
            let dx = (sample((x + 1).min(GRID - 1), y) - sample(x.saturating_sub(1), y)) * 16.0;
            let dy = (sample(x, (y + 1).min(GRID - 1)) - sample(x, y.saturating_sub(1))) * 16.0;
            current[y * GRID + x][2] = dx;
            current[y * GRID + x][3] = dy;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spectral_peak_interpolation_recovers_between_bin_frequency() {
        let frame = AnalysisFrame {
            fft_size: 1200,
            bins: [100.0_f32, 140.0, 180.0]
                .into_iter()
                .map(|f| crate::analysis::FrequencyBin {
                    frequency_hz: f,
                    magnitude: 0.1 * (-((f - 150.0) / 40.0).powi(2)).exp(),
                    dbfs: -20.0,
                })
                .collect(),
            ..Default::default()
        };
        let p = peaks(&frame, &CymaticsSettings::default(), true);
        assert_eq!(p.len(), 1);
        assert!((p[0].0 - 150.0).abs() < 0.001);
        assert!((p[0].1 - 0.1).abs() < 0.001);
    }
    #[test]
    fn resonant_drive_exceeds_off_resonance_and_node_excitation_rejects() {
        let p = Plate::solve(Default::default());
        let mut s = CymaticsSettings {
            gain: 1.0,
            auto_sensitivity: false,
            excite_x: 0.0,
            excite_y: 0.0,
            ..Default::default()
        };
        let f = frequencies(&p, &s)[0];
        let energy = |s: &CymaticsSettings, f: f32| {
            field(&p, s, &[(f, 0.1)], 0.0)
                .iter()
                .map(|v| v[0])
                .sum::<f32>()
        };
        assert!(energy(&s, f) > energy(&s, f * 1.4) * 10.0);
        s.excite_x = -1.0;
        assert!(energy(&s, f) < 1e-6);
    }
    #[test]
    fn silence_gate_and_envelope_are_frame_rate_independent() {
        let s = CymaticsSettings::default();
        let p = Plate::solve(Default::default());
        let frame = AnalysisFrame {
            bins: vec![crate::analysis::FrequencyBin {
                frequency_hz: 400.0,
                magnitude: 0.0000001,
                dbfs: -140.0,
            }],
            ..Default::default()
        };
        assert!(peaks(&frame, &s, true).is_empty());
        assert!(field(&p, &s, &[], 1.0).iter().all(|v| *v == [0.0; 4]));
        let target = vec![[1.0, 0.5, 0.0, 0.0]; GRID * GRID];
        let mut a = vec![[0.0; 4]; GRID * GRID];
        let mut b = a.clone();
        for _ in 0..30 {
            smooth_field(&mut a, &target, &s, 1.0 / 30.0);
        }
        for _ in 0..120 {
            smooth_field(&mut b, &target, &s, 1.0 / 120.0);
        }
        assert!((a[0][0] - b[0][0]).abs() < 1e-5);
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    #[test]
    #[ignore = "manual CPU field timing"]
    fn six_peak_modal_field_timing() {
        let plate = Plate::solve(Default::default());
        let s = CymaticsSettings::default();
        let peaks: Vec<_> = frequencies(&plate, &s)
            .into_iter()
            .step_by(4)
            .map(|f| (f, 0.02))
            .collect();
        let start = std::time::Instant::now();
        for i in 0..240 {
            std::hint::black_box(field(&plate, &s, &peaks, i as f32 / 60.0));
        }
        eprintln!(
            "24 modes, six spectral peaks, 65×65 field: {:.2} ms/frame",
            start.elapsed().as_secs_f64() * 1000.0 / 240.0
        );
    }
}

#[cfg(test)]
mod audio_pipeline_tests {
    use super::*;
    #[test]
    fn quiet_midrange_audio_creates_visible_force_and_silence_releases_it() {
        let plate = Plate::solve(Default::default());
        let settings = CymaticsSettings::default();
        for hz in [125.0, 440.0, 1000.0, 1500.0] {
            let frame = crate::analysis::test_audio_frame(&[(hz, 0.01)]);
            let accepted = peaks(&frame, &settings, true);
            assert!(!accepted.is_empty(), "FFT accepts {hz} Hz");
            let target = field(&plate, &settings, &accepted, 0.0);
            let mut current = vec![[0.0; 4]; GRID * GRID];
            for _ in 0..120 {
                smooth_field(&mut current, &target, &settings, 1.0 / 60.0);
            }
            let force = (current
                .iter()
                .map(|v| v[2] * v[2] + v[3] * v[3])
                .sum::<f32>()
                / (GRID * GRID) as f32)
                .sqrt();
            assert!(
                force > 0.05,
                "{hz} Hz produces visible grain force, got {force}"
            );
            let before = current.iter().map(|v| v[0]).sum::<f32>();
            let silent = crate::analysis::test_audio_frame(&[]);
            let target = field(&plate, &settings, &peaks(&silent, &settings, true), 2.0);
            assert!(target.iter().all(|v| *v == [0.0; 4]));
            for _ in 0..240 {
                smooth_field(&mut current, &target, &settings, 1.0 / 60.0);
            }
            assert!(current.iter().map(|v| v[0]).sum::<f32>() < before * 0.001);
        }
    }
    #[test]
    fn automatic_sensitivity_preserves_nodes_and_input_level_differences() {
        let plate = Plate::solve(Default::default());
        let s = CymaticsSettings::default();
        let loud = field(&plate, &s, &[(440.0, 0.02)], 0.0);
        let quiet = field(&plate, &s, &[(440.0, 0.01)], 0.0);
        for (a, b) in loud.iter().zip(&quiet) {
            assert!((a[0] - 4.0 * b[0]).abs() < 1e-5);
        }
        assert!(
            loud.iter().take(GRID).all(|v| v[0] < 1e-10),
            "supported edge remains a node"
        );
        let physical = CymaticsSettings {
            auto_sensitivity: false,
            ..s
        };
        let raw = field(&plate, &physical, &[(440.0, 0.01)], 0.0);
        let ratio = quiet[GRID * GRID / 2][0] / raw[GRID * GRID / 2][0];
        for (a, b) in quiet.iter().zip(&raw) {
            assert!((a[0] - b[0] * ratio).abs() < 1e-5);
        }
    }
}
