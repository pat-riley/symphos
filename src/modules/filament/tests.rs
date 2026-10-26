use super::*;

#[test]
fn forms_are_distinct_finite_and_audio_deforms_the_resting_sculpture() {
    let mut m = Filament::default();
    let mut forms = Vec::new();
    for form in Form::ALL {
        m.settings.form = form;
        let quiet = m.point(0, 0.32, 1.2);
        assert!(quiet.iter().all(|v| v.is_finite()));
        m.bands.fill(0.8);
        m.energy = 0.8;
        let loud = m.point(0, 0.32, 1.2);
        assert_ne!(quiet, loud);
        m.settings.audio_bend = 0.0;
        m.settings.audio_expand = 0.0;
        let held = m.point(0, 0.32, 1.2);
        m.bands.fill(0.0);
        m.energy = 0.0;
        assert_eq!(held, m.point(0, 0.32, 1.2));
        forms.push(quiet);
        m.settings.audio_bend = 0.32;
        m.settings.audio_expand = 0.18;
    }
    assert!(forms.windows(2).all(|v| v[0] != v[1]));
}

#[test]
fn mesh_budget_gaps_palette_alpha_and_corrupt_settings_are_bounded() {
    let mut m = Filament::default();
    m.settings.shells = usize::MAX;
    m.settings.strands = usize::MAX;
    m.settings.detail = usize::MAX;
    m.settings.depth = f32::NAN;
    m.settings.zoom = f32::INFINITY;
    m.settings.sanitize();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(600.0));
    let theme = AppTheme::default();
    m.settings.breaks = 0.0;
    let full = m.build_mesh(rect, &theme);
    assert!(full.is_valid());
    assert!(full.vertices.len() <= 24_000 * 12);
    assert!(full.vertices.iter().all(|v| v.pos.is_finite()));
    m.settings.breaks = 0.7;
    let broken = m.build_mesh(rect, &theme);
    assert!(broken.vertices.len() < full.vertices.len() / 2);
    m.settings.opacity = 0.0;
    assert!(
        m.build_mesh(rect, &theme)
            .vertices
            .iter()
            .all(|v| v.color.a() == 0)
    );
}

#[test]
fn settings_round_trip_and_old_panes_gain_the_new_component_without_losing_data() {
    use crate::modules::{ModuleKind, ModulePane};
    let pane = ModulePane::new(ModuleKind::Filament);
    let mut saved = serde_json::to_value(pane.snapshot()).unwrap();
    let state: crate::modules::PaneState = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(state, pane.snapshot());
    saved["kind"] = serde_json::json!("Rings");
    saved["settings"]
        .as_array_mut()
        .unwrap()
        .retain(|s| s.get("Filament").is_none());
    saved["sections"].as_array_mut().unwrap().truncate(18);
    saved["macros"]["banks"]
        .as_array_mut()
        .unwrap()
        .truncate(18);
    let legacy: crate::modules::PaneState = serde_json::from_value(saved).unwrap();
    assert_eq!(legacy.settings.len(), ModuleKind::ALL.len());
    assert!(
        legacy
            .settings
            .iter()
            .any(|s| s.kind() == ModuleKind::Filament)
    );
    assert_eq!(legacy.kind, ModuleKind::Rings);
}

#[test]
fn frequency_response_is_localized_and_disconnect_or_source_changes_clear_energy() {
    use crate::analysis::FrequencyBin;
    use std::time::Duration;
    let tone = |hz: f32, sequence| {
        let rate = 48_000;
        let size = 8192;
        let index = (hz * size as f32 / rate as f32).round() as usize;
        AnalysisFrame {
            sample_rate: rate,
            fft_size: size,
            sequence,
            bins: (0..=size / 2)
                .map(|i| FrequencyBin {
                    frequency_hz: i as f32 * rate as f32 / size as f32,
                    magnitude: if i == index { 0.25 } else { 0.0 },
                    dbfs: if i == index { -12.0412 } else { -120.0 },
                })
                .collect::<Vec<_>>()
                .into(),
            ..Default::default()
        }
    };
    let start = Instant::now();
    let mut peaks = Vec::new();
    for hz in [90.0, 6000.0] {
        let mut m = Filament::default();
        m.settings.frequency.smoothing = 0.0;
        let frame = tone(hz, 1);
        m.update(&frame, start, true);
        m.update(&frame, start + Duration::from_millis(100), true);
        assert!(m.energy > 0.0);
        peaks.push(
            m.bands
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0,
        );
        let frozen = (m.bands, m.phase, m.flow_time);
        // A frozen pane supplies the same virtual time and analysis frame.
        m.update(&frame, start + Duration::from_millis(100), true);
        assert_eq!(frozen, (m.bands, m.phase, m.flow_time));
        let energy = m.energy;
        for tick in 2..20 {
            m.update(&frame, start + Duration::from_millis(tick * 100), false);
        }
        assert!(m.energy < energy * 0.01);
        let next = AnalysisFrame {
            capture_epoch: 2,
            ..Default::default()
        };
        m.update(&next, start + Duration::from_secs(2), true);
        assert_eq!(m.energy, 0.0);
    }
    assert!(
        peaks[1] > peaks[0] + 30,
        "bass and treble must affect different contours"
    );
}

#[test]
fn source_switch_clears_the_previous_signal_without_resetting_the_form_motion() {
    use crate::modules::{ModuleKind, ModulePane};
    use crate::signals::audio::Connection;
    let mut pane = ModulePane::new(ModuleKind::Filament);
    pane.ingest_input(&[], &AnalysisFrame::default());
    pane.filament.bands.fill(0.6);
    pane.filament.energy = 0.6;
    pane.filament.phase = 1.2;
    pane.audio_input = Connection::Off;
    pane.ingest_input(&[], &AnalysisFrame::default());
    assert_eq!(pane.filament.energy, 0.0);
    assert!(pane.filament.bands.iter().all(|v| *v == 0.0));
    pane.filament
        .update(&AnalysisFrame::default(), Instant::now(), true);
    assert_eq!(pane.filament.phase, 1.2);
}

#[test]
fn fine_and_dense_options_preserve_settings_and_produce_finer_more_numerous_threads() {
    let mut m = Filament::default();
    m.settings.breaks = 0.0;
    m.settings.detail = 64;
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(600.0));
    let theme = AppTheme::default();
    let plain = m.build_mesh(rect, &theme);
    m.settings.fine_strands = true;
    let fine = m.build_mesh(rect, &theme);
    assert_eq!(plain.vertices.len(), fine.vertices.len());
    let edge_width = |mesh: &egui::Mesh| mesh.vertices[0].pos.distance(mesh.vertices[3].pos);
    assert!((edge_width(&fine) / edge_width(&plain) - 0.45).abs() < 0.001);
    m.settings.dense_weave = true;
    let dense = m.build_mesh(rect, &theme);
    assert_eq!(dense.vertices.len(), plain.vertices.len() * 2);
    assert_eq!(m.settings.strands, 44);
    assert_eq!(m.settings.line_width, 1.25);
    m.settings.shells = 5;
    m.settings.strands = 96;
    m.settings.detail = 256;
    for form in Form::ALL {
        m.settings.form = form;
        let mesh = m.build_mesh(rect, &theme);
        assert!(mesh.is_valid());
        assert!(mesh.vertices.len() <= 64_000 * 12);
        assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
    }
}

#[test]
fn spiral_motion_flows_inward_and_is_continuous_independent_of_camera_orbit() {
    use std::time::Duration;
    let mut m = Filament::default();
    m.settings.centripetal_preset();
    m.settings.drift = 0.0;
    let radius = |p: [f32; 3]| p[0].hypot(p[1]);
    let rim = m.point(0, 0.3, 0.0);
    let core = m.point(0, 0.3, TAU);
    assert!(radius(rim) > radius(core) * 10.0);
    assert!(core[2] < rim[2]);
    let start = Instant::now();
    m.update(&AnalysisFrame::default(), start, false);
    let before = m.point(0, 0.3, 1.2);
    m.update(
        &AnalysisFrame::default(),
        start + Duration::from_millis(100),
        false,
    );
    assert_ne!(before, m.point(0, 0.3, 1.2));
    assert!(!m.settings.auto_orbit);
    m.settings.circulation = 0.0;
    let stopped = m.point(0, 0.3, 1.2);
    m.update(
        &AnalysisFrame::default(),
        start + Duration::from_millis(200),
        false,
    );
    assert_eq!(stopped, m.point(0, 0.3, 1.2));
    for form in [Form::Vortex, Form::GravityWell, Form::Orbital] {
        m.settings.form = form;
        m.flow_time = 1.0 - 0.000001;
        let a = m.point(0, 0.3, 1.2);
        m.flow_time = 1.0 + 0.000001;
        let b = m.point(0, 0.3, 1.2);
        assert!(a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.0001));
    }
}

#[test]
fn centripetal_preset_and_new_options_roundtrip_and_older_sculptures_keep_their_appearance() {
    let mut s = Settings {
        palette: Palette::Ocean,
        yaw: 1.2,
        audio_bend: 0.8,
        ..Default::default()
    };
    s.centripetal_preset();
    assert!(s.fine_strands && s.dense_weave);
    assert_eq!(s.form, Form::GravityWell);
    assert_eq!(s.palette, Palette::Ocean);
    assert_eq!(s.yaw, 1.2);
    assert_eq!(s.audio_bend, 0.8);
    assert_eq!(
        s,
        serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap()
    );
    let mut old = serde_json::to_value(Settings::default()).unwrap();
    for key in [
        "fine_strands",
        "dense_weave",
        "circulation",
        "spiral_turns",
        "central_pull",
    ] {
        old.as_object_mut().unwrap().remove(key);
    }
    let loaded: Settings = serde_json::from_value(old).unwrap();
    assert_eq!(loaded, Settings::default());
}
