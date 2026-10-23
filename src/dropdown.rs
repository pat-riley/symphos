//! Shared dropdown chrome. Styling is local so fields and other buttons keep their layout.
use egui::{Color32, FontId, InnerResponse, Ui, WidgetText};

pub struct ComboBox(egui::ComboBox);
impl ComboBox {
    pub fn from_id_salt(id: impl egui::AsIdSalt) -> Self {
        Self(egui::ComboBox::from_id_salt(id).truncate())
    }
    pub fn selected_text(mut self, text: impl Into<WidgetText>) -> Self {
        self.0 = self.0.selected_text(text);
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.0 = self.0.width(width);
        self
    }
    pub fn height(mut self, height: f32) -> Self {
        self.0 = self.0.height(height);
        self
    }
    pub fn truncate(mut self) -> Self {
        self.0 = self.0.truncate();
        self
    }
    pub fn show_ui<R>(
        self,
        ui: &mut Ui,
        contents: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<Option<R>> {
        let base = ui.visuals().panel_fill;
        let text = ui.visuals().text_color();
        let accent = ui.visuals().selection.bg_fill;
        ui.scope(|ui| {
            button_style(ui.style_mut(), base, text, accent);
            self.0
                .popup_style(egui::style::StyleModifier::new(move |style| {
                    popup_style(style, base, text, accent);
                }))
                .show_ui(ui, |ui| {
                    ui.with_layout(
                        egui::Layout::top_down_justified(egui::Align::LEFT),
                        contents,
                    )
                    .inner
                })
        })
        .inner
    }
}

fn blend(a: Color32, b: Color32, amount: f32) -> Color32 {
    a.lerp_to_gamma(b, amount)
}
fn button_style(style: &mut egui::Style, base: Color32, text: Color32, accent: Color32) {
    style.override_font_id = Some(FontId::proportional(11.0));
    style.spacing.button_padding = egui::vec2(6.0, 3.0);
    style.spacing.interact_size.y = style.spacing.interact_size.y.clamp(22.0, 26.0);
    let visuals = &mut style.visuals;
    visuals.override_text_color = Some(text);
    for (widget, amount) in [
        (&mut visuals.widgets.inactive, 0.015),
        (&mut visuals.widgets.hovered, 0.055),
        (&mut visuals.widgets.active, 0.085),
        (&mut visuals.widgets.open, 0.055),
    ] {
        widget.bg_fill = blend(base, text, amount);
        widget.weak_bg_fill = widget.bg_fill;
        widget.bg_stroke = egui::Stroke::new(1.0, blend(base, text, 0.18));
        widget.fg_stroke.color = text;
        widget.corner_radius = egui::CornerRadius::same(2);
        widget.expansion = 0.0;
    }
    visuals.selection.bg_fill = blend(base, accent, 0.18);
    visuals.selection.stroke = egui::Stroke::new(1.0, text);
}
pub(crate) fn popup_style(style: &mut egui::Style, base: Color32, text: Color32, accent: Color32) {
    button_style(style, base, text, accent);
    style.spacing.menu_margin = egui::Margin::ZERO;
    style.spacing.interact_size.y = 22.0;
    style.spacing.item_spacing.y = 0.0;
    style.spacing.button_padding = egui::vec2(6.0, 3.0);
    style.compact_menu_style = true;
    style.visuals.window_fill = base;
    style.visuals.window_stroke = egui::Stroke::new(1.0, blend(base, text, 0.2));
    style.visuals.window_corner_radius = egui::CornerRadius::same(2);
    style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
    style.visuals.selection.bg_fill = blend(base, accent, 0.18);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::ZERO;
    }
}

/// Carry the same compact chrome into nested menus as well as the root popup.
pub(crate) fn styled_popup<'a>(ui: &Ui, popup: egui::Popup<'a>) -> egui::Popup<'a> {
    use egui::containers::menu::MenuConfig;
    let base = ui.visuals().panel_fill;
    let text = ui.visuals().text_color();
    let accent = ui.visuals().selection.bg_fill;
    let config = MenuConfig::new().style(egui::style::StyleModifier::new(move |style| {
        popup_style(style, base, text, accent);
    }));
    popup.style(config.style.clone()).info(
        egui::UiStackInfo::new(egui::UiKind::Menu)
            .with_tag_value(MenuConfig::MENU_CONFIG_TAG, config),
    )
}

/// Context menus have comfortable rows and no per-item button frames.
/// Explicit group separators are drawn by `menu_separator`.
pub(crate) fn context_popup<'a>(ui: &Ui, popup: egui::Popup<'a>) -> egui::Popup<'a> {
    use egui::containers::menu::MenuConfig;
    let base = ui.visuals().panel_fill;
    let text = ui.visuals().text_color();
    let accent = ui.visuals().selection.bg_fill;
    let config = MenuConfig::new().style(egui::style::StyleModifier::new(move |style| {
        popup_style(style, base, text, accent);
        style.spacing.menu_margin = egui::Margin::same(6);
        style.spacing.interact_size.y = 25.0;
        style.spacing.button_padding = egui::vec2(8.0, 5.0);
        for widget in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.bg_stroke = egui::Stroke::NONE;
        }
        style.visuals.widgets.inactive.weak_bg_fill = base;
        style.visuals.widgets.noninteractive.weak_bg_fill = base;
    }));
    popup.style(config.style.clone()).info(
        egui::UiStackInfo::new(egui::UiKind::Menu)
            .with_tag_value(MenuConfig::MENU_CONFIG_TAG, config),
    )
}

pub(crate) fn menu_separator(ui: &mut Ui) {
    ui.scope(|ui| {
        ui.visuals_mut().widgets.noninteractive.bg_stroke = egui::Stroke::new(
            1.0,
            blend(ui.visuals().panel_fill, ui.visuals().text_color(), 0.16),
        );
        ui.add(egui::Separator::default().spacing(11.0));
    });
}

/// Parameter assignment editors use the same surface and scale as the inspector.
pub(crate) fn editor_popup<'a>(
    ui: &Ui,
    popup: egui::Popup<'a>,
    theme: &crate::theme::AppTheme,
) -> egui::Popup<'a> {
    popup.frame(
        egui::Frame::popup(ui.style())
            .fill(theme.navigation)
            .stroke(egui::Stroke::new(
                1.0,
                theme.foreground.gamma_multiply(0.18),
            ))
            .corner_radius(3.0)
            .inner_margin(8.0),
    )
}
