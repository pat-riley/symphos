use super::*;
use std::time::Duration;

#[test]
fn petal_shells_close_and_geometry_controls_change_the_surface() {
    let mut r = Resonance::default();
    r.settings.flow = 0.0;
    for layer in 0..r.settings.layers {
        for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let a = r.point(layer, 0.0, v);
            let b = r.point(layer, 1.0, v);
            assert!(a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5));
        }
    }
    let baseline = r.point(8, 0.13, 0.6);
    for field in [
        "petals",
        "spacing",
        "twist",
        "opening",
        "flattening",
        "layer_depth",
    ] {
        let mut value = serde_json::to_value(&r.settings).unwrap();
        value[field] = match field {
            "petals" => 5.into(),
            "twist" => 30.into(),
            _ => 0.8.into(),
        };
        let mut changed = Resonance::default();
        changed.apply_settings(&serde_json::from_value(value).unwrap());
        assert_ne!(baseline, changed.point(8, 0.13, 0.6), "{field}");
    }
}

#[test]
fn audio_expands_shells_and_disconnect_fades_response() {
    let mut r = Resonance::default();
    r.settings.flow = 0.0;
    r.settings.spin = 0.0;
    r.settings.ripple_speed = 0.0;
    let frame = AnalysisFrame {
        mix_rms: 0.5,
        ..Default::default()
    };
    let now = Instant::now();
    let before = r.point(8, 0.0, 0.5);
    r.update(&frame, now, true);
    assert!(r.energy > 0.0);
    assert!(r.point(8, 0.0, 0.5)[0].abs() > before[0].abs());
    let held = (r.energy, r.spin_phase, r.flow_phase, r.ripple_phase);
    r.update(&frame, now, true);
    assert_eq!(held, (r.energy, r.spin_phase, r.flow_phase, r.ripple_phase));
    for i in 1..=100 {
        r.update(&frame, now + Duration::from_millis(i * 20), false);
    }
    assert!(r.energy < 0.0001);
}

#[test]
fn all_styles_have_finite_bounded_meshes_at_high_detail() {
    let mut r = Resonance::default();
    r.settings.layers = 16;
    r.settings.detail = 48;
    r.settings.opening = 0.6;
    for style in [
        Style::Membranes,
        Style::Wireframe,
        Style::Contours,
        Style::Points,
    ] {
        r.settings.style = style;
        let mesh = r.build_mesh(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0)),
            &AppTheme::default(),
        );
        assert!(!mesh.vertices.is_empty());
        assert!(mesh.vertices.len() < 400_000);
        assert!(mesh.is_valid());
        assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
    }
}

#[test]
fn settings_roundtrip_sanitizes_and_excludes_live_state() {
    let mut r = Resonance::default();
    r.settings.petals = 5;
    r.settings.layers = 12;
    r.update(
        &AnalysisFrame {
            mix_rms: 0.6,
            ..Default::default()
        },
        Instant::now(),
        true,
    );
    let encoded = serde_json::to_string(&r.settings_snapshot()).unwrap();
    assert!(!encoded.contains("flow_phase"));
    let mut restored = Resonance::default();
    restored.apply_settings(&serde_json::from_str(&encoded).unwrap());
    assert_eq!(restored.settings, r.settings);
    assert_eq!(restored.energy, 0.0);
    let mut invalid = restored.settings_snapshot();
    invalid.layers = 0;
    invalid.detail = usize::MAX;
    invalid.radius = f32::NAN;
    restored.apply_settings(&invalid);
    assert_eq!(restored.settings.layers, 1);
    assert_eq!(restored.settings.detail, 48);
    assert!(restored.settings.radius.is_finite());
}

#[test]
#[ignore = "manual visual review: exports actual renderer meshes to /tmp"]
fn export_resonance_mesh() {
    let mut r = Resonance {
        flow_phase: 0.04,
        ..Default::default()
    };
    for (name, style) in [
        ("membranes", Style::Membranes),
        ("wireframe", Style::Wireframe),
    ] {
        r.settings.style = style;
        let mesh = r.build_mesh(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0)),
            &AppTheme::default(),
        );
        std::fs::write(format!("/tmp/symphos-resonance-{name}.json"), serde_json::to_vec(&serde_json::json!({
            "vertices":mesh.vertices.iter().map(|v|(v.pos.x,v.pos.y,v.color.to_srgba_unmultiplied())).collect::<Vec<_>>(),
            "indices":mesh.indices
        })).unwrap()).unwrap();
    }
}

#[test]
#[ignore = "manual CPU mesh timing; excludes GPU drawing and presentation"]
fn benchmark_resonance_mesh() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0));
    let theme = AppTheme::default();
    for (layers, detail) in [(9, 28), (16, 48)] {
        for style in [
            Style::Membranes,
            Style::Wireframe,
            Style::Contours,
            Style::Points,
        ] {
            let mut r = Resonance::default();
            r.settings.layers = layers;
            r.settings.detail = detail;
            r.settings.style = style;
            r.flow_phase = 0.04;
            let mesh = r.build_mesh(rect, &theme);
            for _ in 0..5 {
                std::hint::black_box(r.build_mesh(rect, &theme));
            }
            let started = Instant::now();
            for _ in 0..100 {
                std::hint::black_box(r.build_mesh(rect, &theme));
            }
            println!(
                "Resonance {style:?}, {layers} layers, detail {detail}: {:.3} ms, {} vertices, {} triangles",
                started.elapsed().as_secs_f64() * 10.0,
                mesh.vertices.len(),
                mesh.indices.len() / 3
            );
        }
    }
}

#[test]
fn optimized_mesh_preserves_reference_geometry_and_colors() {
    let mut r = Resonance::default();
    r.settings.layers = 5;
    r.settings.detail = 20;
    r.settings.opening = 0.7;
    r.settings.audio_expand = 0.7;
    r.settings.audio_ripple = 0.6;
    r.energy = 0.4;
    r.bands = std::array::from_fn(|i| i as f32 / BANDS as f32);
    r.flow_phase = 0.13;
    r.spin_phase = 0.7;
    r.ripple_phase = 0.3;
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
    // Sorting triangles with almost equal depths can change tie order after
    // factoring arithmetic. Compare their geometry as an unordered multiset.
    fn signature(mesh: &egui::Mesh) -> Vec<Vec<(i32, i32, [u8; 4])>> {
        let mut triangles: Vec<_> = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|tri| {
                let mut vertices: Vec<_> = tri
                    .iter()
                    .map(|i| {
                        let v = mesh.vertices[*i as usize];
                        (
                            (v.pos.x * 8.0).round() as i32,
                            (v.pos.y * 8.0).round() as i32,
                            v.color.to_array(),
                        )
                    })
                    .collect();
                vertices.sort();
                vertices
            })
            .collect();
        triangles.sort();
        triangles
    }
    for layout in [
        SpectrumLayout::Layers,
        SpectrumLayout::PoleToPole,
        SpectrumLayout::AroundPetals,
    ] {
        r.settings.spectrum_layout = layout;
        for style in [
            Style::Membranes,
            Style::Wireframe,
            Style::Contours,
            Style::Points,
        ] {
            r.settings.style = style;
            let expected = r.reference_mesh(rect, &AppTheme::default());
            let actual = r.build_mesh(rect, &AppTheme::default());
            assert_eq!(actual.indices.len(), expected.indices.len());
            // Numerical changes stay subpixel; comparing quantized geometry catches
            // topology loss while allowing insignificant sin_cos rounding.
            let a = signature(&actual);
            let b = signature(&expected);
            let matching = a.iter().zip(&b).filter(|(a, b)| a == b).count();
            assert!(
                matching as f32 / a.len() as f32 > 0.98,
                "{style:?}: {matching}/{}",
                a.len()
            );
        }
    }
}

fn tone_frame(hz: f32, sequence: u64) -> AnalysisFrame {
    let rate = 48_000;
    let size = 8192;
    let index = (hz * size as f32 / rate as f32).round() as usize;
    AnalysisFrame {
        sample_rate: rate,
        fft_size: size,
        sequence,
        mix_rms: 0.15,
        bins: (0..=size / 2)
            .map(|i| crate::analysis::FrequencyBin {
                frequency_hz: i as f32 * rate as f32 / size as f32,
                magnitude: if i == index { 0.25 } else { 0.0 },
                dbfs: if i == index { -12.0412 } else { -120.0 },
            })
            .collect(),
        ..Default::default()
    }
}
fn direct_flower() -> Resonance {
    let mut r = Resonance::default();
    r.settings.flow = 0.0;
    r.settings.spin = 0.0;
    r.settings.ripple_speed = 0.0;
    r.settings.ripple_amount = 0.0;
    r.settings.audio_expand = 0.0;
    r.settings.audio_ripple = 0.0;
    r.settings.audio_light = 0.0;
    r.settings.attack_accent = 0.0;
    r.settings.response_ms = 0.0;
    r.settings.frequency.smoothing = 0.0;
    r
}
#[test]
fn tones_in_different_frequency_ranges_move_different_layers() {
    let mut strongest = Vec::new();
    for hz in [375.0, 6000.0] {
        let mut r = direct_flower();
        let before: Vec<_> = (0..r.settings.layers)
            .map(|layer| r.point(layer, 0.0, 0.5))
            .collect();
        let frame = tone_frame(hz, 1);
        let unchanged = frame.bins.clone();
        r.update(&frame, Instant::now(), true);
        let levels: Vec<_> = (0..r.settings.layers)
            .map(|layer| r.spectrum_level(layer, 0.0, 0.5))
            .collect();
        let peak = levels
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        strongest.push(peak);
        assert!(levels[peak] > 0.5);
        assert!(r.point(peak, 0.0, 0.5)[0] > before[peak][0] * 1.15);
        for layer in 0..r.settings.layers {
            if levels[layer] == 0.0 {
                assert_eq!(r.point(layer, 0.0, 0.5), before[layer]);
            }
        }
        assert!(std::sync::Arc::ptr_eq(&frame.bins, &unchanged));
    }
    assert!(strongest[1] > strongest[0] + 2);
}
#[test]
fn new_response_keeps_the_silent_flower_and_all_mapping_seams_closed() {
    let mut r = Resonance::default();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0));
    let normal = r.build_mesh(rect, &AppTheme::default());
    r.settings.spectrum_depth = 0.0;
    r.settings.attack_accent = 0.0;
    r.settings.spectrum_light = 0.0;
    let legacy = r.build_mesh(rect, &AppTheme::default());
    assert_eq!(normal, legacy);
    for layout in [
        SpectrumLayout::Layers,
        SpectrumLayout::PoleToPole,
        SpectrumLayout::AroundPetals,
    ] {
        let mut r = direct_flower();
        r.settings.spectrum_layout = layout;
        r.update(&tone_frame(375.0, 1), Instant::now(), true);
        for layer in 0..r.settings.layers {
            for v in [0.0, 0.3, 0.6, 1.0] {
                let a = r.point(layer, 0.0, v);
                let b = r.point(layer, 1.0, v);
                assert!(a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5));
            }
        }
        assert!(
            r.build_mesh(rect, &AppTheme::default())
                .vertices
                .iter()
                .all(|v| v.pos.is_finite())
        );
    }
}
#[test]
fn attack_accents_are_time_based_hold_when_paused_and_release_on_sustained_audio() {
    let start = Instant::now();
    let frame = tone_frame(375.0, 2);
    let mut outputs = Vec::new();
    for fps in [30, 60, 120] {
        let mut r = Resonance::default();
        r.update(&AnalysisFrame::default(), start, true);
        for i in 1..=fps / 5 {
            r.update(
                &frame,
                start + Duration::from_secs_f64(i as f64 / fps as f64),
                true,
            );
        }
        outputs.push((r.energy, r.transient));
        assert!(r.transient > 0.1);
        let held = (r.energy, r.transient, r.bands, r.flow_phase);
        r.update(&frame, start + Duration::from_millis(200), true);
        assert_eq!(held, (r.energy, r.transient, r.bands, r.flow_phase));
        for i in 1..=2 * fps {
            r.update(
                &frame,
                start + Duration::from_millis(200) + Duration::from_secs_f64(i as f64 / fps as f64),
                true,
            );
        }
        assert!(r.transient < 0.001);
        for i in 1..=2 * fps {
            r.update(
                &frame,
                start
                    + Duration::from_millis(2200)
                    + Duration::from_secs_f64(i as f64 / fps as f64),
                false,
            );
        }
        assert!(r.bands.iter().all(|v| *v < 0.001));
        assert!(r.energy < 0.001);
        r.clear();
        assert_eq!(r.transient, 0.0);
    }
    for &(energy, transient) in &outputs[1..] {
        assert!((energy - outputs[0].0).abs() < 1e-5);
        assert!((transient - outputs[0].1).abs() < 1e-5);
    }
}
#[test]
fn legacy_resonance_presets_keep_their_geometry_and_gain_spectrum_controls() {
    let settings = ResonanceSettings {
        radius: 0.81,
        petals: 5,
        twist: 17.0,
        ..Default::default()
    };
    let mut value = serde_json::to_value(&settings).unwrap();
    for key in [
        "spectrum_layout",
        "spectrum_depth",
        "spectrum_visibility",
        "attack_accent",
        "spectrum_light",
    ] {
        value.as_object_mut().unwrap().remove(key);
    }
    let restored: ResonanceSettings = serde_json::from_value(value).unwrap();
    assert_eq!(restored, settings);
    assert_eq!(
        crate::modules::ModuleKind::Resonance.label(),
        "Lotus Flower"
    );
}

#[test]
fn attack_accent_survives_display_gain_changes_without_pulsing_on_noise() {
    let start = Instant::now();
    let mut pulses = Vec::new();
    for rms in [0.05, 0.5] {
        let mut r = Resonance::default();
        r.settings.frequency.gain = 36.0;
        r.update(&AnalysisFrame::default(), start, true);
        for i in 1..=12 {
            r.update(
                &AnalysisFrame {
                    mix_rms: rms,
                    ..Default::default()
                },
                start + Duration::from_secs_f64(i as f64 / 60.0),
                true,
            );
        }
        pulses.push(r.transient);
    }
    assert!((pulses[0] - pulses[1]).abs() < 1e-5);
    assert!(pulses[0] > 0.1);
    let mut quiet = Resonance::default();
    quiet.update(
        &AnalysisFrame {
            mix_rms: 0.0001,
            ..Default::default()
        },
        start,
        true,
    );
    assert_eq!(quiet.transient, 0.0);
}
