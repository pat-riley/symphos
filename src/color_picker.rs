//! Shared color popup with an asynchronous, explicitly invoked screen sampler.
use std::time::Duration;

use egui::{Color32, Context, Id, Popup, PopupCloseBehavior, Response, Ui};

use crate::{
    help::HoverHelp,
    icons::{self, Icon},
};

const SAMPLING: &str = "screen-color-sampling";

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
    Popup::menu(&response)
        .id(popup_id)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.spacing_mut().slider_width = 275.0;
            let mut selected = Color32::from_rgb(color[0], color[1], color[2]);
            if egui::color_picker::color_picker_color32(
                ui,
                &mut selected,
                egui::color_picker::Alpha::Opaque,
            ) {
                *color = [selected.r(), selected.g(), selected.b()];
                response.mark_changed();
            }
            ui.add_space(4.0);
            let sample = ui.add_enabled(
                cfg!(target_os = "macos") || cfg!(test),
                egui::Button::new("      Sample screen color").min_size(egui::vec2(275.0, 26.0)),
            );
            let icon_rect = egui::Rect::from_center_size(
                sample.rect.left_center() + egui::vec2(16.0, 0.0),
                egui::Vec2::splat(18.0),
            );
            icons::paint(
                ui,
                icon_rect,
                Icon::Eyedropper,
                ui.style().interact(&sample).fg_stroke.color,
            );
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
        let (output, _, _) = frame(&ctx, &mut colors, vec![]);
        let sample = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text.trim() == "Sample screen color" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("every color popup has an eyedropper");
        click(&ctx, &mut colors, sample);
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
