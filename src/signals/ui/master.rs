//! The permanent default capture is a view of Main, not a removable scene bus.
use super::*;

pub(super) fn layout(content: Rect) -> (Rect, Rect) {
    let split = (content.right() - 156.0).max(content.left());
    (
        Rect::from_min_max(content.min, egui::pos2(split, content.bottom())),
        Rect::from_min_max(egui::pos2(split, content.top()), content.max),
    )
}

fn levels(ui: &mut egui::Ui, frame: &AnalysisFrame, color: egui::Color32) {
    for (index, name) in ["L", "R"].into_iter().enumerate() {
        ui.horizontal(|ui| {
            ui.add_sized(
                [10.0, 18.0],
                egui::Label::new(egui::RichText::new(name).size(10.0)),
            );
            let width = (ui.available_width() - 43.0).max(10.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 5.0), egui::Sense::hover());
            let db = |level: f32| {
                if level.is_finite() {
                    (20.0 * level.max(1e-5).log10()).clamp(-90.0, 0.0)
                } else {
                    -90.0
                }
            };
            let rms = db(frame.rms[index]);
            ui.painter()
                .rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
            let mut fill = rect;
            fill.max.x = rect.left() + rect.width() * (rms + 90.0) / 90.0;
            ui.painter().rect_filled(fill, 2.0, color);
            let peak_x = rect.left() + rect.width() * (db(frame.peak[index]) + 90.0) / 90.0;
            ui.painter()
                .vline(peak_x, rect.y_range(), egui::Stroke::new(1.0, color));
            ui.label(
                egui::RichText::new(format!("{rms:.1}"))
                    .monospace()
                    .size(9.0),
            );
        });
    }
}

pub(super) fn strip(
    ui: &mut egui::Ui,
    rect: Rect,
    frame: &AnalysisFrame,
    selected: bool,
    color: egui::Color32,
) -> (bool, bool) {
    let mut strip = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("master-audio-strip")
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    strip.set_clip_rect(rect.intersect(ui.clip_rect()));
    let ui = &mut strip;
    let text = ui.visuals().text_color();
    ui.painter()
        .rect_filled(rect, 0.0, workspace_ui::strip_background(ui, selected));
    ui.painter().vline(
        rect.left(),
        rect.y_range(),
        egui::Stroke::new(1.0, text.gamma_multiply(0.18)),
    );
    let response = ui.interact(rect, ui.id().with("select-master"), egui::Sense::click())
        .help_text("Default audio input · always available. Change the capture source in Audio settings. Add a scene audio channel for inserts and modulation.");
    let header = Rect::from_min_size(rect.min, egui::vec2(rect.width(), 28.0));
    workspace_ui::strip_title(
        ui,
        header.shrink2(egui::vec2(8.0, 0.0)),
        audio::default_source_label(ui.ctx()),
        text,
    );
    ui.painter().hline(
        rect.x_range(),
        header.bottom(),
        egui::Stroke::new(1.0, text.gamma_multiply(0.12)),
    );
    let inner = rect.shrink2(egui::vec2(8.0, 0.0));
    let mut body = ui.new_child(
        egui::UiBuilder::new()
            .layout(egui::Layout::top_down(egui::Align::Min))
            .max_rect(Rect::from_min_max(
                inner.min + egui::vec2(0.0, 38.0),
                inner.max - egui::vec2(0.0, 8.0),
            )),
    );
    let settings = egui::ScrollArea::vertical()
        .id_salt("master-audio-body")
        .auto_shrink([false, false])
        .show(&mut body, |body| {
            body.label(egui::RichText::new("Always available").weak().size(10.0));
            body.label(
                egui::RichText::new(format!(
                    "Stereo · {:.0} kHz",
                    frame.sample_rate as f32 / 1000.0
                ))
                .weak()
                .size(10.0),
            );
            body.add_space(8.0);
            let plot_height = (body.available_height() - 100.0).max(64.0);
            let (plot, _) = body.allocate_exact_size(
                egui::vec2(body.available_width(), plot_height),
                egui::Sense::hover(),
            );
            outputs::audio(body, plot, Some(frame), color, true);
            body.add_space(8.0);
            levels(body, frame, color);
            body.add_space(6.0);
            body.add_sized(
                [body.available_width(), 22.0],
                egui::Button::new(egui::RichText::new("Audio settings…").size(11.0)),
            )
            .clicked()
        })
        .inner;
    (response.clicked() || settings, settings)
}

pub(super) fn inspector(
    ui: &mut egui::Ui,
    rect: Rect,
    frame: &AnalysisFrame,
    theme: &AppTheme,
) -> (bool, bool) {
    let mut editor = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("master-audio-inspector")
            .max_rect(rect),
    );
    editor.set_clip_rect(rect.intersect(ui.clip_rect()));
    workspace_ui::inspector_style(&mut editor, theme);
    let close = workspace_ui::sidebar_header(
        &mut editor,
        Icon::AudioMappings,
        &audio::default_source_label(ui.ctx()),
        "Default audio input",
        &format!(
            "Always available · {:.0} kHz",
            frame.sample_rate as f32 / 1000.0
        ),
        "Close audio inspector",
    )
    .clicked();
    let mut settings = false;
    workspace_ui::inspector_scroll(&mut editor, "master-audio-properties", |ui| {
        audio_channel::inspector_group(ui, "Audio source", |ui| {
            ui.label("Uses the default input selected in Settings.");
            settings = ui.button("Audio settings…").clicked();
            ui.add_space(6.0);
            ui.weak("Create an Audio channel in Signals to add inserts or shape a separate modulation output.");
        });
        audio_channel::inspector_group(ui, "Live spectrum", |ui| {
            let color = theme.accent;
            let (plot, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 120.0),
                egui::Sense::hover(),
            );
            audio_channel::spectrum(ui, plot, Some(frame), color);
            ui.add_space(8.0);
            levels(ui, frame, color);
        });
    });
    (close, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(
        ctx: &egui::Context,
        panel: &mut Panel,
        width: f32,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 400.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                // Large parent control sizes must not leak into the strip footer.
                ui.spacing_mut().interact_size.y = 48.0;
                ui.spacing_mut().item_spacing.y = 16.0;
                panel.show(
                    ui,
                    Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 400.0)),
                    &AnalysisFrame::default(),
                    false,
                    &AppTheme::default(),
                );
            },
        );
        out.textures_delta.clear();
        out
    }
    fn text_rect(out: &egui::FullOutput, label: &str) -> Rect {
        out.shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(Rect::from_min_size(t.pos, t.galley.size()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label}"))
    }
    fn click(ctx: &egui::Context, panel: &mut Panel, width: f32, pos: egui::Pos2) {
        render(ctx, panel, width, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            render(
                ctx,
                panel,
                width,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    }
    #[test]
    fn master_is_visible_when_empty_and_pinned_when_tracks_scroll() {
        for width in [400.0, 1200.0] {
            let ctx = egui::Context::default();
            audio::set_default_source_label(&ctx, "Studio capture".into());
            let mut panel = Panel::default();
            let out = render(&ctx, &mut panel, width, vec![]);
            let title = text_rect(&out, "Studio capture");
            assert!(title.left() > width - 156.0 && title.right() <= width);
            assert!(text_rect(&out, "Always available").left() > width - 156.0);
            click(&ctx, &mut panel, width, title.center());
            assert_eq!(panel.selected, Some(MASTER_ID));
            assert!(panel.inspect_requested);
            assert!(panel.bank.channels.is_empty() && panel.bank.slots.is_empty());
            let out = render(&ctx, &mut panel, width, vec![]);
            let settings = text_rect(&out, "Audio settings…");
            assert!(
                settings.bottom() < 392.0,
                "settings must fit inside the strip"
            );
            click(&ctx, &mut panel, width, settings.center());
            assert!(panel.master_settings_requested);
            for _ in 0..MAX_SIGNALS {
                panel.bank.add_source(Input::Lfo).unwrap();
            }
            let out = render(&ctx, &mut panel, width, vec![]);
            let before = text_rect(&out, "Studio capture");
            let track_before = text_rect(&out, "1 LFO");
            render(
                &ctx,
                &mut panel,
                width,
                vec![
                    egui::Event::PointerMoved(egui::pos2(80.0, 180.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(-1200.0, 0.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            let out = render(&ctx, &mut panel, width, vec![]);
            assert_eq!(before, text_rect(&out, "Studio capture"));
            assert!(
                text_rect(&out, "1 LFO").left() < track_before.left(),
                "tracks must actually scroll"
            );
            assert_eq!(panel.bank.channels.len(), MAX_SIGNALS);
            assert!(!panel.bank.slots.contains(&Some(MASTER_ID)));
            assert!(panel.bank.valid());
            // The strip and the blank board use the same panel fill.
            assert!(
                out.shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                if r.rect.contains(before.center()) && r.fill == AppTheme::default().navigation))
            );
        }
    }
}
