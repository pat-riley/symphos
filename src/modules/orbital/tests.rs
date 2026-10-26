use super::*;
use crate::modules::{ModuleKind, ModulePane, ModuleSettings};
#[test]
fn every_exposed_parameter_roundtrips_and_is_macro_addressable() {
    let mut settings = ModuleSettings::Orbital(Box::default());
    let specs = OrbitalSettings::specs();
    assert!(specs.len() >= 25);
    for spec in specs {
        assert!(spec.valid(), "{}", spec.field);
        let changed = spec.min + (spec.max - spec.min) * 0.37;
        crate::macros::set_many(&mut settings, &[(spec.field.clone(), changed)]);
        let value = serde_json::to_value(&settings).unwrap();
        assert!(
            (value["Orbital"][&spec.field].as_f64().unwrap() - changed).abs() < 0.001,
            "{}",
            spec.field
        );
    }
    let mut pane = ModulePane::new(ModuleKind::Orbital);
    assert!(pane.paste_settings(&settings));
    let state = pane.snapshot();
    let json = serde_json::to_vec(&state).unwrap();
    let decoded = serde_json::from_slice(&json).unwrap();
    let mut restored = ModulePane::new(ModuleKind::Spectrum);
    restored.restore(&decoded);
    assert_eq!(restored.snapshot(), state);
    assert!(!String::from_utf8(json).unwrap().contains("GPU particles"));
}
#[test]
fn hostile_settings_are_bounded_before_shader_upload() {
    let mut settings = OrbitalSettings {
        speed: f32::NAN,
        boundary: -10.0,
        quality: u32::MAX,
        attractors: 0,
        ..Default::default()
    };
    settings.sanitize();
    assert_eq!(settings.speed, 1.8);
    assert_eq!(settings.boundary, 2.0);
    assert_eq!(settings.quality, 3);
    assert_eq!(settings.attractors, 1);
}
#[test]
fn thirteen_component_projects_upgrade_without_changing_existing_settings() {
    let original = ModulePane::new(ModuleKind::Resonance).snapshot();
    let mut value = serde_json::to_value(&original).unwrap();
    value["settings"].as_array_mut().unwrap().retain(|s| {
        s.get("Orbital").is_none()
            && s.get("Milkdrop").is_none()
            && s.get("Cymatics").is_none()
            && s.get("ParticleWaves").is_none()
            && s.get("SpectralBloom").is_none()
            && s.get("Filament").is_none()
    });
    value["sections"].as_array_mut().unwrap().truncate(13);
    value["macros"]["banks"]
        .as_array_mut()
        .unwrap()
        .truncate(13);
    let upgraded: crate::modules::PaneState = serde_json::from_value(value).unwrap();
    assert_eq!(upgraded, original);
}

#[test]
fn old_orbital_projects_gain_appearance_defaults_and_custom_colors_persist() {
    let mut value = serde_json::to_value(OrbitalSettings::default()).unwrap();
    for key in [
        "particle_shape",
        "color_palette",
        "color_mode",
        "color_start",
        "color_end",
        "size_variation",
        "opacity",
        "saturation",
        "color_spread",
        "orb_brightness",
    ] {
        value.as_object_mut().unwrap().remove(key);
    }
    let old: OrbitalSettings = serde_json::from_value(value).unwrap();
    assert_eq!(old, OrbitalSettings::default());
    let custom = OrbitalSettings {
        particle_shape: 4,
        color_palette: 7,
        color_mode: 3,
        color_start: [17, 42, 231],
        color_end: [224, 15, 96],
        ..old
    };
    let restored: OrbitalSettings =
        serde_json::from_slice(&serde_json::to_vec(&custom).unwrap()).unwrap();
    assert_eq!(custom, restored);
    let mut invalid = OrbitalSettings {
        particle_shape: u32::MAX,
        color_palette: u32::MAX,
        color_mode: u32::MAX,
        opacity: -1.0,
        aspect: f32::NAN,
        ..custom
    };
    invalid.sanitize();
    assert_eq!(
        (
            invalid.particle_shape,
            invalid.color_palette,
            invalid.color_mode
        ),
        (6, 7, 4)
    );
    assert_eq!(invalid.opacity, 0.0);
    assert_eq!(invalid.aspect, 1.0);
}

#[test]
fn legacy_orbs_keep_their_look_and_independent_swatches_survive_count_changes() {
    let mut legacy = serde_json::to_value(OrbitalSettings::default()).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .retain(|key, _| !key.starts_with("orb_") || key == "orb_brightness");
    let mut restored: OrbitalSettings = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored, OrbitalSettings::default());
    restored.orb_style = 3;
    restored.orb_color_mode = 3;
    restored.orb_colors[4] = [27, 153, 249];
    restored.attractors = 1;
    let mut restored: OrbitalSettings =
        serde_json::from_slice(&serde_json::to_vec(&restored).unwrap()).unwrap();
    restored.attractors = 5;
    assert_eq!(restored.orb_colors[4], [27, 153, 249]);
    assert_eq!(restored.orb_style, 3);
    assert_eq!(restored.orb_color_mode, 3);
    restored.orb_style = u32::MAX;
    restored.orb_color_mode = u32::MAX;
    restored.orb_palette = u32::MAX;
    restored.orb_scale = f32::INFINITY;
    restored.orb_roughness = 0.0;
    restored.sanitize();
    assert_eq!(
        (
            restored.orb_style,
            restored.orb_color_mode,
            restored.orb_palette
        ),
        (8, 3, 7)
    );
    assert_eq!(restored.orb_scale, 1.0);
    assert_eq!(restored.orb_roughness, 0.05);
}

#[test]
fn camera_guides_use_the_renderer_projection_and_preserve_saved_controls() {
    let rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(800.0, 400.0));
    let mut s = OrbitalSettings {
        yaw: 0.0,
        elevation: 0.0,
        roll: 0.0,
        ..Default::default()
    };
    assert_eq!(camera::project(&s, rect, [0.0; 3]).unwrap(), rect.center());
    let up = camera::project(&s, rect, [0.0, 1.0, 0.0]).unwrap();
    assert!(
        up.y < rect.center().y,
        "world +Y is up in the GPU and guide projection"
    );
    s.projection = 1;
    let a = camera::project(&s, rect, [1.0, 0.0, -2.0]).unwrap();
    let b = camera::project(&s, rect, [1.0, 0.0, 2.0]).unwrap();
    assert!(
        (a.x - b.x).abs() < 0.001,
        "orthographic positions ignore depth"
    );
    s.projection = 0;
    assert!(
        camera::project(&s, rect, [1.0, 0.0, -2.0]).unwrap().x
            > camera::project(&s, rect, [1.0, 0.0, 2.0]).unwrap().x
    );
    assert!(camera::project(&s, rect, [0.0, 0.0, -10.0]).is_none());
    s.show_paths = true;
    s.path_length = 3.0;
    s.particle_shape = 6;
    s.grid_plane = 3;
    s.rear_awareness = 0.1;
    s.plane_confinement = 2.0;
    let restored: OrbitalSettings =
        serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
    assert_eq!(restored, s);
    let text = help::parameter(
        "cohesion",
        "Cohesion",
        "Gather the flock.",
        "Flow & Motion",
        [0.0, 4.0, 0.8],
    );
    assert!(text.contains("Separation") && text.contains("macro") && text.contains("Default: 0.8"));
}

#[test]
fn surface_fields_are_opt_in_for_old_projects_and_save_with_the_scene() {
    let mut old = serde_json::to_value(OrbitalSettings::default()).unwrap();
    old.as_object_mut().unwrap().retain(|key, _| {
        !key.starts_with("field_") && key != "orb_field_enabled" && key != "orb_solid"
    });
    let mut settings: OrbitalSettings = serde_json::from_value(old).unwrap();
    assert!(!settings.orb_field_enabled);
    assert!(settings.orb_solid);
    settings.orb_field_enabled = true;
    settings.orb_solid = false;
    settings.field_clearance = 0.42;
    settings.field_flow = -3.0;
    let restored: OrbitalSettings =
        serde_json::from_value(serde_json::to_value(&settings).unwrap()).unwrap();
    assert_eq!(restored, settings);
}

#[test]
fn optical_looks_preserve_navigation_and_new_settings_roundtrip() {
    let mut legacy = serde_json::to_value(OrbitalSettings::default()).unwrap();
    legacy.as_object_mut().unwrap().retain(|key, _| {
        !key.starts_with("orb_core_")
            && !key.starts_with("orb_lens")
            && !key.starts_with("orb_dispersion")
            && !key.starts_with("orb_corona")
            && !key.starts_with("orb_ray")
            && !key.starts_with("orb_burst")
    });
    let old: OrbitalSettings = serde_json::from_value(legacy).unwrap();
    assert_eq!(old, OrbitalSettings::default());
    for solar in [false, true] {
        let mut s = OrbitalSettings {
            quality: 3,
            yaw: 0.7,
            zoom: 1.8,
            show_paths: true,
            ..old.clone()
        };
        s.apply_look(solar);
        s.sanitize();
        assert_eq!(
            (s.quality, s.yaw, s.zoom, s.show_paths),
            (3, 0.7, 1.8, true)
        );
        assert!(s.orb_field_enabled && s.orb_core_darkness > 0.0 && s.orb_corona > 0.0);
        assert_eq!(s.orb_style, if solar { 8 } else { 7 });
        let restored: OrbitalSettings =
            serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
        assert_eq!(restored, s);
    }
}

#[test]
fn transient_bursts_trigger_once_per_attack_and_decay_on_disconnect() {
    let mut detector = BurstEnvelope::default();
    let mut frame = AnalysisFrame {
        mix_rms: 0.01,
        sequence: 1,
        ..Default::default()
    };
    assert_eq!(
        detector.update(&frame, true, 1.0 / 60.0, 350.0, 1.5),
        0.0,
        "capture primes quietly"
    );
    frame.sequence += 1;
    frame.mix_rms = 0.2;
    let attack = detector.update(&frame, true, 1.0 / 60.0, 350.0, 1.5);
    assert!(attack > 0.8);
    for _ in 0..120 {
        detector.update(&frame, true, 1.0 / 60.0, 350.0, 1.5);
    }
    assert!(
        detector.value < 0.01,
        "one stale analysis frame must not retrigger"
    );
    for _ in 0..120 {
        frame.sequence += 1;
        detector.update(&frame, true, 1.0 / 60.0, 350.0, 1.5);
    }
    assert!(detector.value < 0.01, "steady sustained sound settles");
    frame.sequence += 1;
    frame.mix_rms = 0.0;
    for _ in 0..60 {
        frame.sequence += 1;
        detector.update(&frame, true, 1.0 / 60.0, 350.0, 1.5);
    }
    frame.sequence += 1;
    frame.mix_rms = 0.3;
    assert!(detector.update(&frame, true, 1.0 / 60.0, 350.0, 1.5) > 0.8);
    for _ in 0..120 {
        detector.update(&frame, false, 1.0 / 60.0, 350.0, 1.5);
    }
    assert!(detector.value < 0.01);
    frame.capture_epoch += 1;
    assert!(
        detector.update(&frame, true, 1.0 / 60.0, 350.0, 1.5) < 0.01,
        "reconnection primes without flashing"
    );
}

#[test]
fn consolidated_inspector_migrates_legacy_tabs_and_preserves_saved_values() {
    use crate::modules::SettingsSection;
    let mut pane = ModulePane::new(ModuleKind::Orbital);
    let settings = ModuleSettings::Orbital(Box::new(OrbitalSettings {
        orb_lens: 0.72,
        field_flow: -2.0,
        orb_audio_size: 0.8,
        ..Default::default()
    }));
    pane.paste_settings(&settings);
    let lens = OrbitalSettings::specs()
        .into_iter()
        .find(|spec| spec.field == "orb_lens")
        .unwrap();
    pane.macros.assign(ModuleKind::Orbital, 0, lens);
    let slot = &mut pane.macros.banks[ModuleKind::Orbital as usize][0];
    slot.signal = Some(42);
    slot.depth = -0.63;
    slot.invert = true;
    slot.color = Some([17, 112, 233]);
    assert_eq!(
        pane.sections(),
        &[
            SettingsSection::Particles,
            SettingsSection::Orbs,
            SettingsSection::Response,
            SettingsSection::Camera,
            SettingsSection::Rendering
        ]
    );
    let expected = [0, 1, 0, 2, 0, 3, 1, 0, 1, 1, 1];
    for (old_index, new_index) in expected.into_iter().enumerate() {
        let mut old = serde_json::to_value(pane.snapshot()).unwrap();
        old.as_object_mut()
            .unwrap()
            .remove("orbital_inspector_version");
        old["sections"][ModuleKind::Orbital as usize] = serde_json::json!(old_index);
        let state = serde_json::from_value(old).unwrap();
        let mut restored = ModulePane::new(ModuleKind::Spectrum);
        restored.restore(&state);
        assert_eq!(restored.active_section(), restored.sections()[new_index]);
        assert_eq!(restored.copy_settings(), settings);
        assert_eq!(restored.snapshot().macros, state.macros);
    }
    for &section in pane.sections() {
        pane.select_section(section);
        let state = serde_json::from_value(serde_json::to_value(pane.snapshot()).unwrap()).unwrap();
        let mut restored = ModulePane::new(ModuleKind::Spectrum);
        restored.restore(&state);
        assert_eq!(
            restored.active_section(),
            section,
            "new category indices must not be migrated again"
        );
    }
}
