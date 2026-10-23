use super::*;

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
