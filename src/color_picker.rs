//! Shared color popup with an asynchronous, explicitly invoked screen sampler.
use std::time::Duration;

use egui::{Color32, Context, Id, Popup, PopupCloseBehavior, Response, Ui};

use crate::help::HoverHelp;

mod editor;
mod formats;

const SAMPLING: &str = "screen-color-sampling";
const OPEN_PICKER: &str = "open-color-picker";

#[derive(Clone)]
struct OpenPicker {
    owner: Id,
    last_seen: u64,
}

pub fn is_open(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<OpenPicker>(Id::new(OPEN_PICKER)))
        .is_some_and(|open| ctx.cumulative_frame_nr().saturating_sub(open.last_seen) <= 1)
}

#[derive(Clone)]
struct Sampling {
    owner: Id,
    original: [u8; 3],
    request: u64,
    last_seen: u64,
    color: Option<[u8; 3]>,
}

/// Poll before drawing controls so an asynchronous color change gets its own
/// undo transaction and updates the active preset's edited state.
pub fn begin_frame(ctx: &Context) -> bool {
    if !is_open(ctx) {
        ctx.data_mut(|d| d.remove::<OpenPicker>(Id::new(OPEN_PICKER)));
    }
    let id = Id::new(SAMPLING);
    let Some(mut pending) = ctx.data(|d| d.get_temp::<Sampling>(id)) else {
        return false;
    };
    // A hidden/deleted control must not receive a delayed sample on reopening.
    if ctx.cumulative_frame_nr().saturating_sub(pending.last_seen) > 1 {
        ctx.data_mut(|d| d.remove::<Sampling>(id));
        return false;
    }
    if pending.color.is_none() {
        let mut color = [0; 3];
        match poll(pending.request, &mut color) {
            1 => pending.color = Some(color),
            0 => ctx.request_repaint_after(Duration::from_millis(30)),
            _ => {
                ctx.data_mut(|d| d.remove::<Sampling>(id));
                return false;
            }
        }
    }
    let ready = pending.color.is_some();
    ctx.data_mut(|d| d.insert_temp(id, pending));
    ready
}

pub fn color_edit_button_srgb(ui: &mut Ui, color: &mut [u8; 3]) -> Response {
    let mut response = ui.add(
        egui::Button::new("")
            .min_size(ui.spacing().interact_size)
            .fill(Color32::from_rgb(color[0], color[1], color[2])),
    );
    response.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::ColorButton));
    let state_id = Id::new(SAMPLING);
    if let Some(mut pending) = ui.ctx().data(|d| d.get_temp::<Sampling>(state_id))
        && pending.owner == response.id
    {
        pending.last_seen = ui.ctx().cumulative_frame_nr();
        if let Some(sampled) = pending.color {
            // Also reject stale results if the target changed in the meantime.
            if *color == pending.original && *color != sampled {
                *color = sampled;
                response.mark_changed();
                ui.ctx().request_repaint();
            }
            ui.ctx().data_mut(|d| d.remove::<Sampling>(state_id));
        } else {
            ui.ctx().data_mut(|d| d.insert_temp(state_id, pending));
        }
    }

    let popup_id = response.id.with("color-popup");
    let editor_id = response.id.with("color-editor");
    let open_id = Id::new(OPEN_PICKER);
    let mut open = ui
        .ctx()
        .data(|d| d.get_temp::<OpenPicker>(open_id))
        .is_some_and(|open| open.owner == response.id);
    if response.clicked() {
        open = !open;
    }
    // egui's ComboBox owns the single memory-backed popup. Keep the editor's
    // open state separate so choosing a representation cannot dismiss it.
    let nested_hovered = open
        && ui
            .ctx()
            .data(|d| d.get_temp::<editor::Editor>(editor_id))
            .is_some_and(|editor| editor.format_menu_hovered(ui.ctx()));
    let close_behavior = if nested_hovered {
        PopupCloseBehavior::IgnoreClicks
    } else {
        PopupCloseBehavior::CloseOnClickOutside
    };
    Popup::menu(&response)
        .id(popup_id)
        .open_bool(&mut open)
        .close_behavior(close_behavior)
        .show(|ui| {
            let mut editor = ui
                .ctx()
                .data(|d| d.get_temp::<editor::Editor>(editor_id))
                .unwrap_or_else(|| editor::Editor::new(*color));
            editor.sync(*color);
            let sample = editor.controls(ui);
            if *color != editor.rgb() {
                *color = editor.rgb();
                response.mark_changed();
            }
            ui.ctx().data_mut(|d| d.insert_temp(editor_id, editor));
            #[cfg(test)]
            ui.ctx()
                .data_mut(|d| d.insert_temp(response.id.with("test-eyedropper"), sample.rect));
            if sample
                .help_text(
                    "Sample a pixel anywhere on screen. Escape cancels without changing the color.",
                )
                .clicked()
                && let Some(request) = start()
            {
                let last_seen = ui.ctx().cumulative_frame_nr();
                ui.ctx().data_mut(|d| {
                    d.insert_temp(
                        state_id,
                        Sampling {
                            owner: response.id,
                            original: *color,
                            request,
                            last_seen,
                            color: None,
                        },
                    )
                });
                ui.ctx().request_repaint();
                ui.close();
            }
        });
    let last_seen = ui.ctx().cumulative_frame_nr();
    ui.ctx().data_mut(|d| {
        if open {
            d.insert_temp(
                open_id,
                OpenPicker {
                    owner: response.id,
                    last_seen,
                },
            );
        } else if d
            .get_temp::<OpenPicker>(open_id)
            .is_some_and(|open| open.owner == response.id)
        {
            d.remove::<OpenPicker>(open_id);
        }
    });
    response.help_text("Choose a color, or use the eyedropper to sample the screen.")
}

#[cfg(all(target_os = "macos", not(test)))]
unsafe extern "C" {
    fn symphos_color_sample_start() -> u64;
    fn symphos_color_sample_poll(request: u64, rgb: *mut u8) -> i32;
}

fn start() -> Option<u64> {
    #[cfg(all(target_os = "macos", not(test)))]
    // SAFETY: invoked by the AppKit/egui UI thread; the bridge also checks it.
    let request = unsafe { symphos_color_sample_start() };
    #[cfg(test)]
    let request = 1;
    #[cfg(all(not(target_os = "macos"), not(test)))]
    let request = 0;
    (request != 0).then_some(request)
}

fn poll(request: u64, color: &mut [u8; 3]) -> i32 {
    #[cfg(all(target_os = "macos", not(test)))]
    // SAFETY: main-thread call; color points to exactly three writable bytes.
    unsafe {
        symphos_color_sample_poll(request, color.as_mut_ptr())
    }
    #[cfg(any(not(target_os = "macos"), test))]
    {
        let _ = color;
        if request == 0 { 2 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &Context,
        colors: &mut [[u8; 3]; 2],
        events: Vec<egui::Event>,
    ) -> (egui::FullOutput, Vec<Response>, bool) {
        let mut responses = Vec::new();
        let mut ready = false;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                ready = begin_frame(ctx);
                for (i, color) in colors.iter_mut().enumerate() {
                    ui.push_id(i, |ui| responses.push(color_edit_button_srgb(ui, color)));
                }
            },
        );
        output.textures_delta.clear();
        (output, responses, ready)
    }

    fn click(ctx: &Context, colors: &mut [[u8; 3]; 2], pos: egui::Pos2) {
        for pressed in [true, false] {
            frame(
                ctx,
                colors,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        pressed,
                        button: egui::PointerButton::Primary,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }

    #[test]
    fn popup_offers_eyedropper_and_routes_async_sample_to_only_its_color() {
        let ctx = Context::default();
        let mut colors = [[150, 60, 30], [30, 60, 150]];
        let original = colors;
        let (_, controls, _) = frame(&ctx, &mut colors, vec![]);
        click(&ctx, &mut colors, controls[0].rect.center());
        frame(&ctx, &mut colors, vec![]);
        frame(&ctx, &mut colors, vec![]);
        let sample = ctx
            .data(|d| d.get_temp::<egui::Rect>(controls[0].id.with("test-eyedropper")))
            .expect("every color popup has an eyedropper");
        click(&ctx, &mut colors, sample.center());
        assert_eq!(
            colors, original,
            "starting sampling must not change the color"
        );
        let key = Id::new(SAMPLING);
        let mut pending = ctx.data(|d| d.get_temp::<Sampling>(key)).unwrap();
        assert_eq!(pending.owner, controls[0].id);
        pending.color = Some([8, 210, 90]);
        ctx.data_mut(|d| d.insert_temp(key, pending));
        let (_, controls, ready) = frame(&ctx, &mut colors, vec![]);
        assert!(
            ready,
            "the app can begin an undo transaction before applying the sample"
        );
        assert_eq!(colors, [[8, 210, 90], original[1]]);
        assert!(controls[0].changed());
        assert!(!controls[1].changed());
        assert!(ctx.data(|d| d.get_temp::<Sampling>(key)).is_none());
        assert!(!frame(&ctx, &mut colors, vec![]).1[0].changed());
    }

    #[test]
    fn format_menu_and_text_entry_update_colors_without_changing_them_on_mode_switch() {
        let ctx = Context::default();
        let mut colors = [[150, 60, 30], [30, 60, 150]];
        let (_, controls, _) = frame(&ctx, &mut colors, vec![]);
        click(&ctx, &mut colors, controls[0].rect.center());
        frame(&ctx, &mut colors, vec![]);
        let locate = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text.trim() == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| {
                    panic!(
                        "missing {label}: {:?}",
                        output
                            .shapes
                            .iter()
                            .filter_map(|s| if let egui::Shape::Text(t) = &s.shape {
                                Some(t.galley.job.text.clone())
                            } else {
                                None
                            })
                            .collect::<Vec<_>>()
                    )
                })
        };
        let output = frame(&ctx, &mut colors, vec![]).0;
        click(&ctx, &mut colors, locate(&output, "Hex"));
        frame(&ctx, &mut colors, vec![]);
        let output = frame(&ctx, &mut colors, vec![]).0;
        for label in ["Hex", "RGB", "CSS", "HSL", "HSB"] {
            locate(&output, label);
        }
        click(&ctx, &mut colors, locate(&output, "CSS"));
        let original = colors;
        frame(&ctx, &mut colors, vec![]);
        assert_eq!(colors, original);
        let field = ctx
            .data(|d| d.get_temp::<egui::Rect>(Id::new("test-color-text-field")))
            .unwrap();
        click(&ctx, &mut colors, field.center());
        let select_all = || egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        frame(
            &ctx,
            &mut colors,
            vec![select_all(), egui::Event::Paste("rgb(255 136 0)".into())],
        );
        assert_eq!(colors, [[255, 136, 0], original[1]]);
        frame(
            &ctx,
            &mut colors,
            vec![select_all(), egui::Event::Paste("invalid color".into())],
        );
        assert_eq!(
            colors[0],
            [255, 136, 0],
            "invalid input must not replace the color"
        );
        let output = frame(&ctx, &mut colors, vec![]).0;
        click(&ctx, &mut colors, locate(&output, "CSS"));
        frame(&ctx, &mut colors, vec![]);
        let output = frame(&ctx, &mut colors, vec![]).0;
        click(&ctx, &mut colors, locate(&output, "HSL"));
        let output = frame(&ctx, &mut colors, vec![]).0;
        for label in ["H", "S", "L"] {
            locate(&output, label);
        }
        assert_eq!(colors[0], [255, 136, 0]);
        click(&ctx, &mut colors, egui::pos2(650.0, 650.0));
        assert!(!is_open(&ctx), "a click outside dismisses the picker");
    }

    #[test]
    fn cancellation_and_a_changed_target_keep_existing_colors() {
        let ctx = Context::default();
        let mut colors = [[10, 20, 30], [50, 60, 70]];
        let (_, controls, _) = frame(&ctx, &mut colors, vec![]);
        let key = Id::new(SAMPLING);
        for (request, color, original) in [(0, None, colors[0]), (1, Some([255, 0, 0]), [0, 0, 0])]
        {
            let before = colors;
            let pending = Sampling {
                owner: controls[0].id,
                original,
                request,
                last_seen: ctx.cumulative_frame_nr(),
                color,
            };
            ctx.data_mut(|d| d.insert_temp(key, pending));
            let (_, responses, _) = frame(&ctx, &mut colors, vec![]);
            assert_eq!(colors, before);
            assert!(!responses[0].changed());
            assert!(ctx.data(|d| d.get_temp::<Sampling>(key)).is_none());
        }
    }

    #[test]
    fn hidden_controls_cannot_receive_a_sample_when_reopened() {
        let ctx = Context::default();
        let mut colors = [[10, 20, 30], [50, 60, 70]];
        let (_, controls, _) = frame(&ctx, &mut colors, vec![]);
        let key = Id::new(SAMPLING);
        ctx.data_mut(|d| {
            d.insert_temp(
                key,
                Sampling {
                    owner: controls[0].id,
                    original: colors[0],
                    request: 1,
                    last_seen: 0,
                    color: Some([255, 0, 0]),
                },
            )
        });
        for _ in 0..3 {
            let _ = ctx.run_ui(Default::default(), |_| {});
        }
        assert!(!frame(&ctx, &mut colors, vec![]).2);
        assert_eq!(colors[0], [10, 20, 30]);
    }
}
