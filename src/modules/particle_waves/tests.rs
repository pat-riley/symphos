use super::*;
use crate::{
    analysis::FrequencyBin,
    modules::{ModuleKind, ModulePane, ModuleSettings, PaneState},
};
fn tone(sequence: u64, hz: f32) -> AnalysisFrame {
    let mut frame = AnalysisFrame {
        sequence,
        fft_size: 4096,
        ..Default::default()
    };
    let peak = (hz * 4096.0 / 48000.0).round() as usize;
    frame.bins = (0..2049)
        .map(|i| {
            let magnitude: f32 = if i == peak { 0.25 } else { 0.0 };
            FrequencyBin {
                frequency_hz: i as f32 * 48000.0 / 4096.0,
                magnitude,
                dbfs: 20.0 * magnitude.max(1e-7).log10(),
            }
        })
        .collect();
    frame.dominant_frequency_hz = hz;
    frame
}
#[test]
fn measured_frequency_controls_the_ribbon_peak_and_silence_clears_it() {
    let start = Instant::now();
    let mut wave = ParticleWaves::default();
    wave.settings.frequency.smoothing = 0.0;
    for i in 0..60 {
        wave.advance(
            &tone(i + 1, 1000.0),
            start + Duration::from_secs_f32(i as f32 / 60.0),
            true,
        );
    }
    let peak = wave.field[..BANDS]
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    let expected = wave
        .settings
        .frequency
        .fraction_for_frequency(1000.0, 48000)
        * BANDS as f32;
    assert!(
        (peak as f32 - expected).abs() < 2.0,
        "peak={peak}, expected={expected}"
    );
    assert!(wave.field[peak] > 0.8);
    assert!(wave.envelope > 0.8);
    let silent = AnalysisFrame::default();
    for i in 0..300 {
        wave.advance(
            &silent,
            start + Duration::from_secs_f32(1.0 + i as f32 / 60.0),
            false,
        );
    }
    assert!(wave.field.iter().all(|v| *v == 0.0));
    assert!(wave.envelope < 1e-5);
}
#[test]
fn elapsed_time_drives_motion_and_pause_does_not_advance() {
    let start = Instant::now();
    let mut results = Vec::new();
    for fps in [30, 60, 120] {
        let mut wave = ParticleWaves::default();
        wave.settings.frequency.smoothing = 0.0;
        for i in 0..=fps {
            wave.advance(
                &tone(i + 1, 220.0),
                start + Duration::from_secs_f64(i as f64 / fps as f64),
                true,
            );
        }
        let before = (wave.time, wave.envelope, wave.field.clone());
        for _ in 0..10 {
            assert_eq!(
                wave.advance(&tone(fps + 1, 220.0), start + Duration::from_secs(1), true),
                0.0
            );
        }
        assert_eq!(before, (wave.time, wave.envelope, wave.field.clone()));
        results.push((wave.time, wave.envelope));
    }
    for pair in results.windows(2) {
        assert!((pair[0].0 - pair[1].0).abs() < 0.00001);
        assert!((pair[0].1 - pair[1].1).abs() < 0.0001);
    }
}
#[test]
fn pane_resets_clear_both_particle_profiles_without_changing_settings() {
    let mut pane = ModulePane::new(ModuleKind::SpectralBloom);
    let saved = pane.copy_settings();
    for capture_only in [true, false] {
        for wave in [&mut pane.particle_waves, &mut pane.spectral_bloom] {
            wave.advance(&tone(1, 220.0), Instant::now(), true);
            assert!(wave.last.is_some());
        }
        if capture_only {
            pane.reset_capture();
        } else {
            pane.reset();
        }
        for wave in [&pane.particle_waves, &pane.spectral_bloom] {
            assert!(wave.last.is_none());
            assert_eq!(wave.time, 0.0);
            assert_eq!(wave.envelope, 0.0);
            assert!(wave.field.iter().all(|v| *v == 0.0));
        }
        assert_eq!(pane.copy_settings(), saved);
    }
}
#[test]
fn per_layer_parameters_round_trip_and_are_addressable_without_affecting_neighbors() {
    let mut pane = ModulePane::new(ModuleKind::ParticleWaves);
    let mut settings = pane.copy_settings();
    assert!(crate::macros::set(&mut settings, "layers.3.size", 2.5));
    assert!(crate::macros::set(&mut settings, "layers.3.rotation", 67.0));
    assert!(crate::macros::set(&mut settings, "frequency.min_hz", 90.0));
    assert_eq!(crate::macros::get(&settings, "layers.3.size"), Some(2.5));
    assert_eq!(crate::macros::get(&settings, "layers.2.size"), Some(1.0));
    for path in [
        "layers.8.size",
        "layers.-1.size",
        "layers.bad.size",
        "layers.0.enabled",
    ] {
        assert!(!crate::macros::set(&mut settings, path, 1.0));
    }
    pane.paste_settings(&settings);
    let saved = pane.snapshot();
    let json = serde_json::to_vec(&saved).unwrap();
    let restored: PaneState = serde_json::from_slice(&json).unwrap();
    assert_eq!(restored, saved);
    let mut new = ModulePane::new(ModuleKind::Spectrum);
    new.restore(&restored);
    assert_eq!(new.snapshot(), saved);
}
#[test]
fn sixteen_component_projects_upgrade_without_changing_existing_settings() {
    let original = ModulePane::new(ModuleKind::Waterfall).snapshot();
    let mut json = serde_json::to_value(&original).unwrap();
    json["sections"].as_array_mut().unwrap().truncate(16);
    json["macros"]["banks"].as_array_mut().unwrap().truncate(16);
    json["settings"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v.get("ParticleWaves").is_none() && v.get("SpectralBloom").is_none());
    let restored: PaneState = serde_json::from_value(json).unwrap();
    assert_eq!(original, restored);
}
#[test]
fn invalid_values_are_bounded_before_rendering_and_auto_rotation_is_not_an_edit() {
    let mut wave = ParticleWaves::default();
    let mut s = Settings {
        layer_count: 999,
        height: f32::NAN,
        focus_distance: f32::INFINITY,
        auto_orbit: true,
        ..Default::default()
    };
    s.layers[0].size = -12.0;
    s.frequency.floor = f32::NAN;
    s.frequency.min_hz = 20000.0;
    s.frequency.max_hz = 2.0;
    wave.apply_settings(&s);
    assert_eq!(wave.settings.layer_count, 8);
    assert!(wave.settings.height.is_finite());
    assert_eq!(wave.settings.layers[0].size, 0.1);
    assert!(wave.settings.frequency.max_hz > wave.settings.frequency.min_hz);
    let snapshot = ModuleSettings::ParticleWaves(Box::new(wave.settings_snapshot()));
    assert!(matches!(snapshot, ModuleSettings::ParticleWaves(ref settings) if settings.auto_orbit));
    wave.clear();
    assert_eq!(wave.settings.layer_count, 8);
    assert!(wave.field.iter().all(|v| *v == 0.0));
}

#[test]
fn canvas_drag_rotates_freely_and_takes_over_from_auto_rotation() {
    let ctx = egui::Context::default();
    let mut wave = ParticleWaves::default();
    wave.settings.auto_orbit = true;
    let initial = (
        wave.settings.yaw,
        wave.settings.elevation,
        wave.settings.roll,
    );
    let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let now = Instant::now();
    let start = egui::pos2(400.0, 300.0);
    let events = [
        vec![],
        vec![egui::Event::PointerMoved(start)],
        vec![egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
        vec![egui::Event::PointerMoved(egui::pos2(450.0, 340.0))],
        vec![egui::Event::PointerMoved(egui::pos2(500.0, 380.0))],
    ];
    for events in events {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                events,
                ..Default::default()
            },
            |ui| {
                wave.draw(
                    ui,
                    rect.shrink(10.0),
                    &AnalysisFrame::default(),
                    &AppTheme::default(),
                    now,
                    false,
                );
            },
        );
        output.textures_delta.clear();
    }
    assert!(!wave.settings.auto_orbit);
    assert_ne!(initial.0, wave.settings.yaw);
    assert_ne!(initial.1, wave.settings.elevation);
    assert_ne!(initial.2, wave.settings.roll);
}

#[test]
fn bloom_has_independent_defaults_and_mapped_feedback_survives_roundtrip() {
    let mut pane = ModulePane::new(ModuleKind::SpectralBloom);
    let original = pane.snapshot();
    let mut settings = pane.copy_settings();
    assert!(crate::macros::set(&mut settings, "bloom.persistence", 1.25));
    assert!(crate::macros::set(&mut settings, "bloom.curl", 1.7));
    pane.paste_settings(&settings);
    let encoded = serde_json::to_vec(&pane.snapshot()).unwrap();
    let restored: PaneState = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(pane.snapshot(), restored);
    assert_ne!(original, restored);
    let mut wave = ParticleWaves::spectral_bloom();
    wave.settings.height = 0.0;
    wave.reset_settings();
    assert_eq!(wave.settings, Settings::spectral_bloom());
    assert_ne!(wave.settings, Settings::default());
}

#[test]
fn seventeen_component_projects_gain_bloom_without_changing_saved_settings() {
    let original = ModulePane::new(ModuleKind::ParticleWaves).snapshot();
    let mut json = serde_json::to_value(&original).unwrap();
    json["sections"].as_array_mut().unwrap().truncate(17);
    json["macros"]["banks"].as_array_mut().unwrap().truncate(17);
    json["settings"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v.get("SpectralBloom").is_none());
    let restored: PaneState = serde_json::from_value(json).unwrap();
    assert_eq!(original, restored);
}
