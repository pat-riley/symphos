//! Native frequency selection and envelope editing for visualizer mappings.
use super::*;
use crate::help::HoverHelp;
use crate::parameter::Parameter;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

impl Modulator {
    pub fn editor(
        &mut self,
        ui: &mut egui::Ui,
        frame: &AnalysisFrame,
        settings: &mut ModulationSettings,
        kind: Visualizer,
        layers: usize,
    ) {
        ui.add(Parameter::new(&mut settings.ambient, 0.0..=1.0, 0.25).text("Ambient motion"));
        ui.add(Parameter::new(&mut settings.amount, 0.0..=4.0, 1.0).text("Modulation amount"));
        if ui
            .data(|d| {
                d.get_temp::<crate::signals::ui::Shared>(egui::Id::new(
                    crate::signals::ui::CONTEXT_KEY,
                ))
            })
            .is_none()
            || settings
                .routes
                .iter()
                .any(|r| r.signal.is_none() && r.target.layered())
        {
            ui.checkbox(&mut settings.adaptive, "Adaptive sensitivity")
                .help_text("Track each band's recent peak. The noise floor still rejects silence.");
        }
        ui.add_space(8.0);
        if !settings.routes.iter().any(|r| Some(r.id) == self.selected) {
            self.selected = settings.routes.first().map(|r| r.id);
        }
        let mut remove = None;
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 22.0;
            ui.spacing_mut().item_spacing.y = 2.0;
            for r in &mut settings.routes {
                ui.push_id(r.id, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut r.enabled, "");
                        let label = if r.target.layered() && r.layer == 0 {
                            format!("{} · auto", r.target.label())
                        } else {
                            r.target.label().into()
                        };
                        if ui
                            .add(
                                egui::Button::selectable(self.selected == Some(r.id), label)
                                    .truncate()
                                    .min_size(Vec2::new(
                                        (ui.available_width() - 44.0).max(60.0),
                                        22.0,
                                    )),
                            )
                            .clicked()
                        {
                            self.selected = Some(r.id);
                        }
                        if ui.small_button("×").help_text("Remove mapping").clicked() {
                            remove = Some(r.id);
                        }
                    });
                });
            }
        });
        if let Some(id) = remove {
            settings.routes.retain(|r| r.id != id);
        }
        ui.add_enabled_ui(settings.routes.len() < MAX_ROUTES, |ui| {
            ui.menu_button("+ Add mapping", |ui| {
                for &target in kind.targets() {
                    if ui.button(target.label()).clicked() {
                        if let Some(route) = settings.routes.iter().find(|r| r.target == target) {
                            self.selected = Some(route.id);
                            ui.close();
                            continue;
                        }
                        let id = (1..)
                            .find(|id| settings.routes.iter().all(|r| r.id != *id))
                            .unwrap();
                        settings.routes.push(Route {
                            id,
                            target,
                            ..Route::default()
                        });
                        self.selected = Some(id);
                        ui.close();
                    }
                }
            });
        });
        let Some(r) = settings
            .routes
            .iter_mut()
            .find(|r| Some(r.id) == self.selected)
        else {
            return;
        };
        ui.separator();
        ui.strong(r.target.label());
        if crate::signals::ui::mapping_editor(ui, r, kind) {
            settings.sanitize(kind);
            return;
        }
        combo(
            ui,
            "source",
            "Source",
            &mut r.source,
            &[Source::Band, Source::FullMix, Source::Stereo],
            Source::label,
        );
        if r.target.layered() {
            crate::dropdown::ComboBox::from_id_salt("mapping-layer")
                .selected_text(if r.layer == 0 {
                    if r.source == Source::Band {
                        "All layers · auto bands".into()
                    } else {
                        "All layers".into()
                    }
                } else {
                    format!("Layer {} · bottom = 1", r.layer)
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut r.layer,
                        0,
                        if r.source == Source::Band {
                            "All layers · auto bands"
                        } else {
                            "All layers"
                        },
                    );
                    for i in 1..=layers {
                        ui.selectable_value(&mut r.layer, i, format!("Layer {i}"));
                    }
                });
        }
        if r.source == Source::Band {
            let fallback;
            let features = if let Some(f) = &frame.features {
                f.as_ref()
            } else {
                fallback = AudioFeatures::new(&frame.bins, frame.mix_rms, frame.rms);
                &fallback
            };
            spectrum_editor(
                ui,
                features,
                r,
                if r.target.layered() && r.layer == 0 {
                    layers
                } else {
                    1
                },
            );
        } else {
            ui.small(if r.source == Source::Stereo {
                "Signed left/right balance, gated by mix level."
            } else {
                "Full-mix RMS level, independent of frequency selection."
            });
        }
        combo(
            ui,
            "response",
            "Response",
            &mut r.response,
            &[Response::Follow, Response::Gate, Response::Transient],
            Response::label,
        );
        let meter = self
            .states
            .iter()
            .find(|s| s.id == r.id)
            .map(|s| {
                s.voices.iter().fold((0.0_f32, 0.0_f32), |(a, b), v| {
                    (a.max(v.input), b.max(v.value))
                })
            })
            .unwrap_or_default();
        ui.horizontal(|ui| {
            let width = (ui.available_width() - ui.spacing().item_spacing.x) * 0.5;
            ui.add(
                egui::ProgressBar::new(meter.0)
                    .desired_width(width)
                    .desired_height(18.0)
                    .text("Input"),
            );
            ui.add(
                egui::ProgressBar::new(meter.1)
                    .desired_width(width)
                    .desired_height(18.0)
                    .text("Envelope"),
            );
        });
        envelope_editor(
            ui,
            &mut r.envelope,
            r.response,
            &mut self.envelope_span,
            meter.1,
        );
        if r.response != Response::Follow {
            ui.add(Parameter::new(&mut r.threshold, 0.01..=0.99, 0.25).text("Trigger threshold"));
            if r.response == Response::Transient {
                ui.add(
                    Parameter::new(&mut r.hold_ms, 20.0..=2000.0, 120.0)
                        .logarithmic(true)
                        .text("Hold ms"),
                );
            }
        }
        ui.add(Parameter::new(&mut r.amount, -3.0..=3.0, 1.0).text("Amount"));
        ui.collapsing("Sensitivity & output range", |ui| {
            ui.add(Parameter::new(&mut r.floor_db,-90.0..=-12.0,-66.0).text("Floor dB"));
            ui.add(Parameter::new(&mut r.gain_db,-24.0..=24.0,0.0).text("Gain dB"));
            ui.add(Parameter::new(&mut r.curve,0.25..=4.0,2.0).text("Curve"));
            ui.add(Parameter::new(&mut r.output_min,0.0..=r.output_max,0.0).text("Output min"));
            ui.add(Parameter::new(&mut r.output_max,r.output_min..=1.0,1.0).text("Output max"));
            ui.small("Amounts add to the base parameter. Negative amounts reduce it. A nonzero output minimum also applies in silence.");
        });
        settings.sanitize(kind);
    }
}
pub(crate) fn combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut T,
    options: &[T],
    name: impl Fn(T) -> &'static str,
) {
    crate::modules::property_row(ui, label, |ui| {
        crate::dropdown::ComboBox::from_id_salt(id)
            .selected_text(name(*value))
            .width(ui.available_width())
            .truncate()
            .show_ui(ui, |ui| {
                for &option in options {
                    ui.selectable_value(value, option, name(option));
                }
            })
            .response
            .help_text(format!(
                "{label}\n{}",
                crate::help::control_description(label)
            ));
    });
}
pub(crate) fn spectrum_editor(
    ui: &mut egui::Ui,
    f: &AudioFeatures,
    r: &mut Route,
    layers: usize,
) -> Rect {
    ui.add_space(6.0);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 100.0), Sense::hover());
    response.help_text("Frequency selection\nDrag the left and right handles to isolate the frequency band used for detection. The horizontal scale is logarithmic: bass is left, treble is right. A band can emphasize a kick or melody, but cannot separate instruments that overlap in frequency.");
    let plot = rect.shrink2(Vec2::new(6.0, 16.0));
    let x = |hz: f32| plot.left() + (hz / 20.0).log10() / 3.0 * plot.width();
    let hz = |x: f32| 20.0 * 1000.0_f32.powf(((x - plot.left()) / plot.width()).clamp(0.0, 1.0));
    let accent = ui.visuals().selection.bg_fill;
    let p = ui.painter();
    p.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    for (i, v) in f.spectrum.iter().enumerate() {
        let x = plot.left() + (i as f32 + 0.5) / 96.0 * plot.width();
        p.line_segment(
            [
                Pos2::new(x, plot.bottom()),
                Pos2::new(x, plot.bottom() - v * plot.height()),
            ],
            Stroke::new(
                (plot.width() / 96.0 - 1.0).max(1.0),
                Color32::from_gray(100),
            ),
        );
    }
    let selection = Rect::from_min_max(
        Pos2::new(x(r.low), plot.top()),
        Pos2::new(x(r.high), plot.bottom()),
    );
    p.rect_filled(selection, 0.0, accent.gamma_multiply(0.25));
    for i in 1..layers {
        let (edge, _) = band_range(r.low, r.high, i, layers);
        p.line_segment(
            [
                Pos2::new(x(edge), plot.top()),
                Pos2::new(x(edge), plot.bottom()),
            ],
            Stroke::new(1.0, accent.gamma_multiply(0.6)),
        );
    }
    for (label, hz) in [
        ("20", 20.0),
        ("200", 200.0),
        ("2k", 2000.0),
        ("20k Hz", 20000.0),
    ] {
        p.text(
            Pos2::new(x(hz), rect.bottom() - 2.0),
            if hz == 20000.0 {
                egui::Align2::RIGHT_BOTTOM
            } else {
                egui::Align2::LEFT_BOTTOM
            },
            label,
            egui::FontId::monospace(9.0),
            ui.visuals().weak_text_color(),
        );
    }
    for (index, value) in [r.low, r.high].into_iter().enumerate() {
        let handle = Rect::from_center_size(
            Pos2::new(x(value), plot.center().y),
            Vec2::new(12.0, plot.height() + 14.0),
        );
        let response = ui
            .interact(
                handle,
                ui.id().with(("band-handle", r.id, index)),
                Sense::drag(),
            )
            .on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal)
            .help_text("Frequency boundary\nDrag horizontally to set the lower or upper frequency limit. Use the Hz fields below for exact values.");
        if response.dragged()
            && let Some(pos) = response.interact_pointer_pos()
        {
            if index == 0 {
                r.low = hz(pos.x).clamp(20.0, r.high / 1.001);
            } else {
                r.high = hz(pos.x).clamp(r.low * 1.001, 20000.0);
            }
        }
        ui.painter().line_segment(
            [
                Pos2::new(x(value), plot.top() - 3.0),
                Pos2::new(x(value), plot.bottom() + 3.0),
            ],
            Stroke::new(2.0, ui.visuals().text_color()),
        );
    }
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut r.low)
                .range(20.0..=r.high / 1.001)
                .speed(1.0)
                .suffix(" Hz"),
        );
        ui.label("—");
        ui.add(
            egui::DragValue::new(&mut r.high)
                .range(r.low * 1.001..=20000.0)
                .speed(10.0)
                .suffix(" Hz"),
        );
    });
    if layers > 1 {
        ui.small(format!(
            "{layers} logarithmic bands · bottom: bass / top: treble"
        ));
    }
    rect
}

pub(crate) fn envelope_editor(
    ui: &mut egui::Ui,
    e: &mut Envelope,
    mode: Response,
    drag_span: &mut Option<f32>,
    level: f32,
) -> Rect {
    ui.add_space(6.0);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 108.0), Sense::hover());
    response.help_text("Response envelope\nAttack sets rise time; decay falls toward sustain; sustain holds a level; release returns to zero. Drag A, D and R horizontally and S vertically. Follow mode uses attack and release to follow incoming level instead of triggering a full ADSR shape.");
    let plot = rect.shrink2(Vec2::new(12.0, 20.0));
    let follow = mode == Response::Follow;
    let decay = if follow { 0.0 } else { e.decay };
    let sustain = if follow { 1.0 } else { e.sustain };
    let span = drag_span.unwrap_or((e.attack + decay + 300.0 + e.release) * 1.15);
    let point = |ms: f32, y: f32| {
        Pos2::new(
            plot.left() + ms / span * plot.width(),
            plot.bottom() - y * plot.height(),
        )
    };
    let points = [
        point(0.0, 0.0),
        point(e.attack, 1.0),
        point(e.attack + decay, sustain),
        point(e.attack + decay + 300.0, sustain),
        point(e.attack + decay + 300.0 + e.release, 0.0),
    ];
    let p = ui.painter();
    p.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    for i in 0..=4 {
        let y = plot.bottom() - i as f32 / 4.0 * plot.height();
        p.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        );
    }
    p.add(egui::Shape::line(
        points.to_vec(),
        Stroke::new(2.0, ui.visuals().text_color()),
    ));
    p.line_segment(
        [point(0.0, level), point(span, level)],
        Stroke::new(1.0, ui.visuals().selection.bg_fill.gamma_multiply(0.5)),
    );
    let mut dragging = false;
    for i in 1..=4 {
        if follow && (i == 2 || i == 3) {
            continue;
        }
        let response = ui
            .interact(
                Rect::from_center_size(points[i], Vec2::splat(18.0)),
                ui.id().with(("adsr", i)),
                Sense::drag(),
            )
            .on_hover_and_drag_cursor(if i == 3 {
                egui::CursorIcon::ResizeVertical
            } else {
                egui::CursorIcon::ResizeHorizontal
            });
        if response.dragged() {
            dragging = true;
            *drag_span = Some(span);
            let dx = response.drag_delta().x / plot.width() * span;
            match i {
                1 => e.attack = (e.attack + dx).clamp(1.0, 2000.0),
                2 => e.decay = (e.decay + dx).clamp(1.0, 3000.0),
                3 => {
                    e.sustain =
                        (e.sustain - response.drag_delta().y / plot.height()).clamp(0.0, 1.0)
                }
                _ => e.release = (e.release + dx).clamp(1.0, 5000.0),
            }
        }
        ui.painter()
            .circle_filled(points[i], 4.0, ui.visuals().text_color());
        ui.painter().text(
            points[i] + Vec2::new(0.0, if i == 1 { -7.0 } else { 7.0 }),
            if i == 1 {
                egui::Align2::CENTER_BOTTOM
            } else {
                egui::Align2::CENTER_TOP
            },
            ["", "A", "D", "S", "R"][i],
            egui::FontId::monospace(10.0),
            ui.visuals().text_color(),
        );
    }
    if !dragging {
        *drag_span = None;
    }
    ui.horizontal_wrapped(|ui| {
        for (label, value, max) in [
            ("A", &mut e.attack, 2000.0),
            ("D", &mut e.decay, 3000.0),
            ("R", &mut e.release, 5000.0),
        ] {
            if follow && label == "D" {
                continue;
            }
            ui.scope(|ui| {
                ui.add(
                    egui::DragValue::new(value)
                        .range(1.0..=max)
                        .speed(2.0)
                        .prefix(format!("{label} "))
                        .suffix("ms"),
                );
            });
        }
    });
    if !follow {
        ui.scope(|ui| {
            ui.add(Parameter::new(&mut e.sustain, 0.0..=1.0, 0.65).text("Sustain"));
        });
    }
    ui.small(if follow {
        "Attack and release follow the incoming level."
    } else {
        "Drag A / D / R horizontally; S vertically."
    });
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    #[test]
    fn dragging_band_edges_edits_frequency_range_without_crossing() {
        let ctx = egui::Context::default();
        let features = AudioFeatures::new(&[], 0.0, [0.0; 2]);
        let mut route = Route::default();
        let render = |r: &mut Route, events| {
            let mut rect = Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = spectrum_editor(ui, &features, r, 1);
                },
            );
            output.textures_delta.clear();
            rect
        };
        let rect = render(&mut route, vec![]);
        let plot = rect.shrink2(Vec2::new(6.0, 16.0));
        let a = Pos2::new(plot.left(), plot.center().y);
        let b = a + Vec2::new(25.0, 0.0);
        render(
            &mut route,
            vec![egui::Event::PointerMoved(a), pointer(a, true)],
        );
        render(&mut route, vec![egui::Event::PointerMoved(b)]);
        render(&mut route, vec![pointer(b, false)]);
        assert!(route.low > 20.0 && route.low < 250.0);
        assert_eq!(route.high, 250.0);
        let a = Pos2::new(
            plot.left() + (route.low / 20.0).log10() / 3.0 * plot.width(),
            plot.center().y,
        );
        let b = Pos2::new(plot.right(), plot.center().y);
        render(
            &mut route,
            vec![egui::Event::PointerMoved(a), pointer(a, true)],
        );
        render(&mut route, vec![egui::Event::PointerMoved(b)]);
        render(&mut route, vec![pointer(b, false)]);
        assert!(route.low < route.high);
    }
    #[test]
    fn dragging_adsr_handles_edits_attack_and_sustain() {
        let ctx = egui::Context::default();
        let mut env = Envelope::default();
        let mut span = None;
        let render = |e: &mut Envelope, span: &mut Option<f32>, events| {
            let mut rect = Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = envelope_editor(ui, e, Response::Gate, span, 0.0);
                },
            );
            output.textures_delta.clear();
            rect
        };
        let rect = render(&mut env, &mut span, vec![]);
        let plot = rect.shrink2(Vec2::new(12.0, 20.0));
        let duration = (env.attack + env.decay + 300.0 + env.release) * 1.15;
        let a = Pos2::new(
            plot.left() + env.attack / duration * plot.width(),
            plot.top(),
        );
        let b = a + Vec2::new(25.0, 0.0);
        render(
            &mut env,
            &mut span,
            vec![egui::Event::PointerMoved(a), pointer(a, true)],
        );
        render(&mut env, &mut span, vec![egui::Event::PointerMoved(b)]);
        render(&mut env, &mut span, vec![pointer(b, false)]);
        assert!(env.attack > 100.0);
        assert_eq!(env.decay, 180.0);
        assert_eq!(env.sustain, 0.65);
        let duration = (env.attack + env.decay + 300.0 + env.release) * 1.15;
        let a = Pos2::new(
            plot.left() + (env.attack + env.decay + 300.0) / duration * plot.width(),
            plot.bottom() - env.sustain * plot.height(),
        );
        let b = a + Vec2::new(0.0, 15.0);
        render(
            &mut env,
            &mut span,
            vec![egui::Event::PointerMoved(a), pointer(a, true)],
        );
        render(&mut env, &mut span, vec![egui::Event::PointerMoved(b)]);
        render(&mut env, &mut span, vec![pointer(b, false)]);
        assert!(env.sustain < 0.5);
        assert_eq!(env.release, 600.0);
    }
}
