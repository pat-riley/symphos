use super::*;

#[test]
fn initial_palette_applies_to_shared_palettes_and_saved_overrides_survive_restore() {
    let palette = Palette::Custom(Palette::ClassicLed.gradient(&AppTheme::default()));
    let mut supported = 0;
    for kind in ModuleKind::ALL {
        let mut pane = ModulePane::new_with_palette(kind, palette);
        let settings = serde_json::to_value(pane.copy_settings()).unwrap();
        let active = settings.as_object().unwrap().values().next().unwrap();
        if let Some(value) = active.get("palette") {
            assert_eq!(*value, serde_json::to_value(palette).unwrap(), "{kind:?}");
            supported += 1;
            // An inspector override is stored with the pane, not linked to the
            // app preference that happened to be chosen when it was created.
            pane.set_initial_palette(Palette::Ocean);
            let saved = pane.snapshot();
            let mut restored = ModulePane::new_with_palette(kind, Palette::Ember);
            restored.restore(&saved);
            assert_eq!(restored.copy_settings(), pane.copy_settings());
        } else {
            assert_eq!(
                pane.copy_settings(),
                ModulePane::new(kind).copy_settings(),
                "{kind:?} keeps its authored colors"
            );
        }
    }
    assert_eq!(supported, 13);
}

#[test]
fn rings_visual_response_is_in_geometry_and_source_gain_stays_in_signals() {
    let collect = |input: bool| {
        let ctx = egui::Context::default();
        ctx.memory_mut(|m| m.set_everything_is_visible(true));
        let mut pane = ModulePane::new(ModuleKind::Rings);
        pane.select_inspector_section(SettingsSection::Geometry);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(440.0, 12000.0))),
                ..Default::default()
            },
            |ui| {
                if input {
                    pane.input_controls(ui, &AnalysisFrame::default());
                } else {
                    pane.inspector_controls(ui, &AnalysisFrame::default());
                }
            },
        );
        output.textures_delta.clear();
        ctx.data(|d| {
            d.get_temp::<std::collections::BTreeSet<String>>(egui::Id::new(
                "test-rendered-parameter-keys",
            ))
        })
        .unwrap()
    };
    let geometry = collect(false);
    let input = collect(true);
    for key in [
        "amplitude",
        "sideways",
        "waveform_visibility",
        "waveform_detail",
        "smoothing_ms",
        "line_smoothing",
        "pulse_amount",
        "pulse_release_ms",
    ] {
        assert!(geometry.contains(key), "{key} belongs in Geometry");
        assert!(
            !input.contains(key),
            "{key} is a drawing control, not a source processor"
        );
    }
    assert!(input.contains("gain"));
    assert!(!geometry.contains("gain"));
}

#[test]
fn consolidated_tabs_and_signals_keep_every_existing_numeric_parameter_reachable() {
    for kind in ModuleKind::ALL {
        let collect = |standard: bool| {
            let ctx = egui::Context::default();
            ctx.memory_mut(|m| m.set_everything_is_visible(true));
            let mut pane = ModulePane::new(kind);
            let sections = if standard {
                pane.inspector_sections()
            } else {
                pane.sections()
            }
            .to_vec();
            for section in sections {
                if standard {
                    pane.select_inspector_section(section);
                } else {
                    pane.select_section(section);
                }
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(440.0, 60000.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        if standard {
                            pane.inspector_controls(ui, &AnalysisFrame::default());
                        } else {
                            pane.controls(ui, &AnalysisFrame::default());
                        }
                    },
                );
                out.textures_delta.clear();
            }
            if standard {
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    pane.input_controls(ui, &AnalysisFrame::default())
                });
                out.textures_delta.clear();
            }
            ctx.data(|d| {
                d.get_temp::<std::collections::BTreeSet<String>>(egui::Id::new(
                    "test-rendered-parameter-keys",
                ))
            })
            .unwrap_or_default()
        };
        let old = collect(false);
        let new = collect(true);
        let missing = old.difference(&new).collect::<Vec<_>>();
        assert!(missing.is_empty(), "{kind:?} lost controls: {missing:?}");
    }
}

#[test]
fn camera_defaults_upgrade_old_presets_without_changing_existing_values() {
    for kind in ModuleKind::ALL {
        let pane = ModulePane::new(kind);
        let mut value = serde_json::to_value(pane.copy_settings()).unwrap();
        let object = value
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()
            .as_object_mut()
            .unwrap();
        object.remove("camera_motion");
        let original = object.clone();
        let restored: ModuleSettings = serde_json::from_value(value).unwrap();
        let encoded = serde_json::to_value(restored).unwrap();
        let encoded = encoded.as_object().unwrap().values().next().unwrap();
        for (key, value) in original {
            assert_eq!(encoded[&key], value, "{kind:?}: {key}");
        }
    }
}
