use super::*;
use egui::Vec2;

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

#[test]
fn pane_canvases_omit_descriptions_and_gesture_instructions() {
    for kind in [
        ModuleKind::Waterfall,
        ModuleKind::Sphere,
        ModuleKind::Cylinder,
        ModuleKind::Rings,
        ModuleKind::Resonance,
        ModuleKind::Flux,
        ModuleKind::Strata,
    ] {
        let mut pane = ModulePane::new(kind);
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            pane.draw(
                ui,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0)),
                &AnalysisFrame::default(),
                &AppTheme::default(),
                false,
            );
        });
        out.textures_delta.clear();
        for shape in out.shapes {
            if let egui::Shape::Text(t) = shape.shape {
                let text = &t.galley.job.text;
                assert!(
                    ![
                        "Drag:",
                        "ORBITAL WAVEFORMS",
                        "ORBITAL OSCILLOSCOPE",
                        "LAYERED WAVEFRONTS",
                        "LONGITUDE =",
                        "AROUND =",
                        "POLE RIPPLES",
                        "AXIAL RIPPLES"
                    ]
                    .iter()
                    .any(|prefix| text.starts_with(prefix)),
                    "{kind:?}: {text}"
                );
            }
        }
    }
}

#[test]
fn detailed_grids_are_visual_controls_that_restore_from_new_and_legacy_presets() {
    for kind in [ModuleKind::Spectrum, ModuleKind::Oscilloscope] {
        let mut pane = ModulePane::new(kind);
        let original = pane.copy_settings();
        let mut encoded = serde_json::to_value(&original).unwrap();
        let body = encoded
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()
            .as_object_mut()
            .unwrap();
        body.get_mut("grid_detail").unwrap()["x_divisions"] = 16.into();
        body.get_mut("grid_detail").unwrap()["subdivisions"] = 5.into();
        let detailed: ModuleSettings = serde_json::from_value(encoded.clone()).unwrap();
        assert!(pane.paste_settings(&detailed));
        let saved = pane.snapshot();
        let mut restored = ModulePane::new(kind);
        restored.restore(&saved);
        assert_eq!(restored.copy_settings(), detailed);
        let ctx = egui::Context::default();
        ctx.memory_mut(|m| m.set_everything_is_visible(true));
        pane.select_inspector_section(SettingsSection::Appearance);
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(440.0, 12000.0))),
                ..Default::default()
            },
            |ui| pane.inspector_controls(ui, &AnalysisFrame::default()),
        );
        out.textures_delta.clear();
        let keys = ctx
            .data(|d| {
                d.get_temp::<std::collections::BTreeSet<String>>(egui::Id::new(
                    "test-rendered-parameter-keys",
                ))
            })
            .unwrap();
        for key in [
            "x_divisions",
            "y_divisions",
            "subdivisions",
            "opacity",
            "minor_opacity",
            "line_width",
        ] {
            assert!(
                keys.contains(&format!("grid_detail.{key}")),
                "{kind:?}: {key}"
            );
        }
        let body = encoded
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()
            .as_object_mut()
            .unwrap();
        body.remove("grid_detail");
        body.remove("status_labels");
        let legacy: ModuleSettings = serde_json::from_value(encoded).unwrap();
        assert_eq!(legacy, original);
    }
}

#[test]
fn every_axis_navigator_defaults_off_and_explicit_saved_choices_survive_restore() {
    let mut camera_count = 0;
    for kind in ModuleKind::ALL {
        let mut pane = ModulePane::new(kind);
        let mut encoded = serde_json::to_value(pane.copy_settings()).unwrap();
        let body = encoded
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        let Some(gizmo) = body.get_mut("show_gizmo") else {
            continue;
        };
        camera_count += 1;
        assert_eq!(
            *gizmo,
            serde_json::json!(false),
            "{kind:?} must start without an axis navigator"
        );
        *gizmo = true.into();
        let enabled: ModuleSettings = serde_json::from_value(encoded).unwrap();
        assert!(pane.paste_settings(&enabled));
        let mut restored = ModulePane::new(kind);
        restored.restore(&pane.snapshot());
        assert_eq!(
            restored.copy_settings(),
            enabled,
            "{kind:?} preserves a saved opt-in"
        );
        restored.reset_settings();
        let reset = serde_json::to_value(restored.copy_settings()).unwrap();
        let body = reset.as_object().unwrap().values().next().unwrap();
        assert_eq!(
            body["show_gizmo"], false,
            "{kind:?} reset also disables the navigator"
        );
    }
    assert_eq!(camera_count, 11);
}
