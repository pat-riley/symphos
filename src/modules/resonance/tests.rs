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
