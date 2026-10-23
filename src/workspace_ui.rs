//! Workspace chrome: a growing component library and one searchable preset browser.
use crate::{
    help::HoverHelp,
    icons::{self, Icon},
    modules::ModuleKind,
    theme::AppTheme,
    workspace::{self, Preset, PresetContent, Storage},
};
use eframe::egui::{self, Align, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, Vec2};

pub const NAV_CONTROL_HEIGHT: f32 = 26.0;
pub const NAV_PADDING: f32 = 8.0;
pub const NAV_HEIGHT: f32 = NAV_CONTROL_HEIGHT + 2.0 * NAV_PADDING;

pub const LIBRARY_WIDTH: f32 = workspace::default_library_width();

fn sidebar_blend(a: Color32, b: Color32, amount: f32) -> Color32 {
    let channel = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * amount) as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}
pub fn dock_style(ui: &mut egui::Ui) {
    let panel = ui.visuals().panel_fill;
    let text = ui.visuals().text_color();
    let field = sidebar_blend(panel, egui::Color32::BLACK, 0.32);
    let visuals = ui.visuals_mut();
    visuals.override_text_color = Some(sidebar_blend(text, egui::Color32::WHITE, 0.65));
    visuals.weak_text_color = Some(sidebar_blend(text, egui::Color32::WHITE, 0.15));
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, text.gamma_multiply(0.30));
    visuals.extreme_bg_color = field;
    visuals.widgets.inactive.bg_fill = field;
    visuals.widgets.inactive.weak_bg_fill = field;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, text.gamma_multiply(0.30));
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(4);
    }
}

pub fn rounded_content_corners(ui: &egui::Ui, rect: Rect, radius: f32, color: egui::Color32) {
    let radius = radius
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5)
        .max(0.0);
    let mut mesh = egui::Mesh::default();
    for (corner, direction) in [
        (rect.left_top(), egui::vec2(1.0, 1.0)),
        (rect.right_top(), egui::vec2(-1.0, 1.0)),
        (rect.left_bottom(), egui::vec2(1.0, -1.0)),
        (rect.right_bottom(), egui::vec2(-1.0, -1.0)),
    ] {
        let start = mesh.vertices.len() as u32;
        mesh.colored_vertex(corner, color);
        for i in 0..=12 {
            let angle = std::f32::consts::PI + i as f32 / 12.0 * std::f32::consts::FRAC_PI_2;
            let offset = egui::vec2(radius + radius * angle.cos(), radius + radius * angle.sin());
            mesh.colored_vertex(corner + offset * direction, color);
        }
        for i in 0..12 {
            mesh.add_triangle(start, start + i + 1, start + i + 2);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

pub fn strip_surface(ui: &egui::Ui, rect: Rect, color: egui::Color32, selected: bool) {
    let body = sidebar_blend(
        ui.visuals().panel_fill,
        ui.visuals().text_color(),
        if selected { 0.16 } else { 0.075 },
    );
    ui.painter().rect_filled(rect, 0.0, body);
    let header = Rect::from_min_size(rect.min, egui::vec2(rect.width(), 28.0));
    ui.painter().rect_filled(header, 0.0, color);
    let divider = egui::Stroke::new(
        1.0,
        sidebar_blend(ui.visuals().panel_fill, egui::Color32::BLACK, 0.5),
    );
    ui.painter()
        .vline(rect.right() - 0.5, rect.y_range(), divider);
    ui.painter().hline(rect.x_range(), header.bottom(), divider);
}

pub fn strip_title(ui: &egui::Ui, rect: Rect, text: String, color: egui::Color32) {
    let galley = egui::WidgetText::from(egui::RichText::new(text).size(13.0).strong().color(color))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            rect.width(),
            egui::FontId::proportional(13.0),
        );
    let origin = egui::pos2(rect.left(), rect.center().y - galley.size().y * 0.5);
    let painter = ui.painter_at(rect);
    // The bundled proportional face is light; a subtle second pass adds portable header weight.
    painter.galley(origin, galley.clone(), color);
    painter.galley(origin + egui::vec2(0.35, 0.0), galley, color);
}

pub fn sidebar_style(ui: &mut egui::Ui, theme: &AppTheme) {
    ui.spacing_mut().item_spacing = Vec2::new(5.0, 3.0);
    ui.spacing_mut().slider_width = 90.0;
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(12.0));
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(12.0));
    ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
    ui.spacing_mut().interact_size.y = 22.0;
    ui.spacing_mut().indent = 16.0;
    // Inspector chrome shares the navigation surface in every app theme.
    // Derive its secondary surfaces from that same base instead of panel/card.
    let base = theme.navigation;
    let section = sidebar_blend(base, theme.foreground, 0.045);
    let field = if theme.dark_mode {
        sidebar_blend(base, Color32::BLACK, 0.24)
    } else {
        sidebar_blend(base, theme.foreground, 0.07)
    };
    let visuals = ui.visuals_mut();
    visuals.panel_fill = base;
    visuals.widgets.noninteractive.bg_fill = section;
    visuals.widgets.noninteractive.weak_bg_fill = section;
    visuals.collapsing_header_frame = false;
    visuals.extreme_bg_color = field;
    visuals.widgets.inactive.bg_fill = field;
    visuals.widgets.inactive.weak_bg_fill = field;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme.foreground.gamma_multiply(0.10));
    visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(1.0, theme.foreground.gamma_multiply(0.18));
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(3);
    }
}

/// Shared skin for every inspector; applies locally to its child UI.
pub fn inspector_style(ui: &mut egui::Ui, theme: &AppTheme) {
    sidebar_style(ui, theme);
    ui.painter()
        .rect_filled(ui.max_rect(), 0.0, theme.navigation);
    ui.spacing_mut().item_spacing = Vec2::new(7.0, 6.0);
    ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
    ui.spacing_mut().scroll.bar_width = 9.0;
    ui.style_mut().override_font_id = Some(FontId::proportional(12.0));
    let widgets = &mut ui.visuals_mut().widgets;
    for widget in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(1);
        widget.expansion = 0.0;
    }
}

/// Untabbed properties below the pinned identity header. Context is restored so
/// flat rows and edge-to-edge rules cannot leak into the mixer or plugin windows.
pub fn inspector_scroll(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    body: impl FnOnce(&mut egui::Ui),
) {
    let bounds_id = egui::Id::new("inspector-bounds");
    let flat_id = egui::Id::new(crate::modules::FLAT_INSPECTOR);
    let bounds = ui.available_rect_before_wrap();
    let previous = ui.data_mut(|d| {
        let previous = (d.get_temp::<Rect>(bounds_id), d.get_temp::<bool>(flat_id));
        d.insert_temp(bounds_id, bounds);
        d.insert_temp(flat_id, true);
        previous
    });
    egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(10, 5))
                .show(ui, body);
        });
    ui.data_mut(|d| {
        d.remove::<Rect>(bounds_id);
        d.remove::<bool>(flat_id);
        if let Some(value) = previous.0 {
            d.insert_temp(bounds_id, value);
        }
        if let Some(value) = previous.1 {
            d.insert_temp(flat_id, value);
        }
    });
}

pub fn sidebar_header(
    ui: &mut egui::Ui,
    icon: Icon,
    title: &str,
    subtitle: &str,
    metadata: &str,
    description: &str,
) -> egui::Response {
    egui::Frame::new()
        .inner_margin(8)
        .show(ui, |ui| {
            let close = ui
                .horizontal_top(|ui| {
                    let (badge, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                    ui.painter().rect_filled(
                        badge,
                        5.0,
                        ui.visuals().widgets.noninteractive.bg_fill,
                    );
                    icons::paint(ui, badge, icon, ui.visuals().text_color());
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.visuals_mut().button_frame = false;
                        let close = icons::sized_button(ui, Icon::Close, false, 24.0, description);
                        close.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, description)
                        });
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(title).size(13.0).strong())
                                    .truncate(),
                            );
                            ui.add(
                                egui::Label::new(RichText::new(subtitle).size(11.0).weak())
                                    .truncate(),
                            );
                        });
                        close
                    })
                    .inner
                })
                .inner;
            ui.add_space(2.0);
            ui.label(RichText::new(metadata).size(11.0).weak());
            close
        })
        .inner
}

pub fn sidebar_rule(ui: &mut egui::Ui, bounds: Rect) {
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    let mut clip = ui.clip_rect();
    clip.min.x = bounds.left();
    clip.max.x = bounds.right();
    ui.painter().with_clip_rect(clip).hline(
        bounds.x_range(),
        row.center().y,
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
}

fn navigation_color(theme: &AppTheme) -> Color32 {
    theme.navigation
}

pub fn navigation_surface(ui: &egui::Ui, rect: Rect, theme: &AppTheme) {
    let color = navigation_color(theme);
    ui.painter().rect_filled(rect, 0.0, color);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, theme.foreground.gamma_multiply(0.14)),
    );
}

pub fn navigation_region(ui: &mut egui::Ui, rect: Rect, id: &str, theme: &AppTheme) -> egui::Ui {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(rect));
    let field = if theme.dark_mode {
        Color32::from_rgb(
            theme.background.r().saturating_sub(4),
            theme.background.g().saturating_sub(4),
            theme.background.b().saturating_sub(4),
        )
    } else {
        theme.background
    };
    child.spacing_mut().interact_size.y = NAV_CONTROL_HEIGHT;
    child.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
    child.spacing_mut().item_spacing.x = 6.0;
    child.style_mut().override_font_id = Some(FontId::proportional(13.0));
    let visuals = child.visuals_mut();
    visuals.widgets.inactive.bg_fill = field;
    visuals.widgets.inactive.weak_bg_fill = field;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme.muted.gamma_multiply(0.65));
    visuals.widgets.inactive.fg_stroke.color = theme.foreground;
    visuals.widgets.hovered.bg_fill = theme.selection;
    visuals.widgets.hovered.weak_bg_fill = theme.selection;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme.accent);
    visuals.widgets.hovered.fg_stroke.color = theme.foreground;
    visuals.widgets.active.bg_fill = theme.selection;
    visuals.widgets.active.weak_bg_fill = theme.selection;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, theme.accent);
    visuals.widgets.active.fg_stroke.color = theme.foreground;
    visuals.selection.bg_fill = theme.selection;
    visuals.selection.stroke = Stroke::new(1.0, theme.foreground);
    child
}

#[derive(Clone, Copy, Debug)]
pub enum DragPayload {
    Component(ModuleKind),
    Pane(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    View,
    Layout,
    Component,
}
impl Scope {
    fn label(self) -> &'static str {
        match self {
            Self::View => "Full views",
            Self::Layout => "Layouts",
            Self::Component => "Components",
        }
    }
    fn matches(self, content: &PresetContent) -> bool {
        matches!(
            (self, content),
            (Self::View, PresetContent::View { .. })
                | (Self::Layout, PresetContent::Layout(_))
                | (Self::Component, PresetContent::Component(_))
        )
    }
}
pub enum Action {
    Apply(String),
    Add(ModuleKind),
    Save(String, Scope),
    Rename(String, String),
    Delete(String),
}
struct SaveDialog {
    name: String,
    scope: Scope,
}
#[derive(Default)]
pub struct Manager {
    presets_open: bool,
    pub library_open: bool,
    preset_search: String,
    scope: Option<Scope>,
    save: Option<SaveDialog>,
    rename: Option<(String, String)>,
    delete: Option<String>,
}
impl Manager {
    pub fn open_presets(&mut self, scope: Scope) {
        self.presets_open = true;
        self.scope = Some(scope);
    }

    pub fn modal_open(&self) -> bool {
        self.save.is_some() || self.rename.is_some() || self.delete.is_some()
    }
    pub fn preset_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        storage: &Storage,
        selected_kind: ModuleKind,
        has_panes: bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let current = storage.presets.iter().find(|p| p.id == storage.active);
        let current_scope = self.scope.unwrap_or_else(|| {
            current
                .map(|p| match p.content {
                    PresetContent::Layout(_) => Scope::Layout,
                    PresetContent::View { .. } => Scope::View,
                    PresetContent::Component(_) => Scope::Component,
                })
                .unwrap_or(Scope::View)
        });
        let cycling: Vec<_> = storage
            .presets
            .iter()
            .filter(|p| {
                current_scope.matches(&p.content)
                    && match &p.content {
                        PresetContent::Component(settings) => {
                            has_panes && settings.kind() == selected_kind
                        }
                        _ => true,
                    }
            })
            .collect();
        let save_width = ui
            .painter()
            .layout_no_wrap(
                "Save".into(),
                egui::TextStyle::Button.resolve(ui.style()),
                ui.visuals().text_color(),
            )
            .size()
            .x
            + 2.0 * ui.spacing().button_padding.x;
        let save_width = save_width.max(45.0);
        let spacing = 3.0 * ui.spacing().item_spacing.x;
        let arrows_width = 2.0 * NAV_CONTROL_HEIGHT;
        let picker_width =
            (ui.available_width() - arrows_width - save_width - spacing).clamp(110.0, 340.0);
        let row_width = picker_width + arrows_width + save_width + spacing;
        // Center the actual visible controls, rather than their wider container.
        let row = Rect::from_center_size(
            ui.max_rect().center(),
            Vec2::new(row_width, ui.available_height()),
        );
        let mut row_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("preset-controls")
                .max_rect(row),
        );
        row_ui.horizontal_centered(|ui| {
            if ui.add_enabled(!cycling.is_empty(), egui::Button::new("‹").min_size(Vec2::splat(NAV_CONTROL_HEIGHT))).help_text("Previous preset in this category.").clicked() {
                actions.push(Action::Apply(cycle(&cycling, &storage.active, -1)));
            }
            let label = format!("{}{}", current.map_or("Custom view", |p| p.name.as_str()), if storage.modified { " *" } else { "" });
            let button = ui.add_sized([picker_width, NAV_CONTROL_HEIGHT], egui::Button::new(label).truncate()).help_text("Browse full views, layouts, and component presets. * means the current view has edits.");
            let arrow = button.rect.right_center() - Vec2::new(11.0, 0.0);
            ui.painter().add(egui::Shape::convex_polygon(vec![arrow + Vec2::new(-3.5, -2.0), arrow + Vec2::new(3.5, -2.0), arrow + Vec2::new(0.0, 2.5)], ui.visuals().text_color(), Stroke::NONE));
            if button.clicked() { self.scope = Some(current_scope); }
            egui::Popup::menu(&button).show(|ui| {
                ui.set_width(390.0);
                let scope = self.scope.get_or_insert(current_scope);
                ui.horizontal(|ui| {
                    for tab in [Scope::View, Scope::Layout, Scope::Component] { ui.selectable_value(scope, tab, tab.label()); }
                });
                ui.add(egui::TextEdit::singleline(&mut self.preset_search).hint_text("Search presets…").desired_width(f32::INFINITY));
                ui.separator();
                let query = self.preset_search.trim().to_lowercase();
                let candidates: Vec<_> = storage.presets.iter().filter(|p| scope.matches(&p.content) && p.name.to_lowercase().contains(&query)).collect();
                egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                    if candidates.is_empty() { ui.weak("No saved presets in this category. Use Save to create one."); }
                    for preset in candidates {
                        let compatible = match &preset.content { PresetContent::Component(settings) => has_panes && settings.kind() == selected_kind, _ => true };
                        ui.push_id(&preset.id, |ui| {
                            ui.horizontal(|ui| {
                                if let Some(tree) = preset.content.tree() {
                                    let (_, rect) = ui.allocate_space(Vec2::new(44.0, 30.0));
                                    thumbnail(ui, tree, rect, ui.visuals().text_color());
                                }
                                let mut text = preset.name.clone();
                                if let PresetContent::Component(settings) = &preset.content { text.push_str(&format!(" · {}", settings.kind().label())); }
                                let width = ui.available_width() - if preset.factory() { 0.0 } else { 34.0 };
                                let response = ui.add_enabled_ui(compatible, |ui| ui.add_sized([width, 30.0], egui::Button::selectable(storage.active == preset.id, text).truncate())).inner;
                                if response.clicked() { actions.push(Action::Apply(preset.id.clone())); ui.close(); }
                                if !compatible { response.help_text("Select a pane with this component type to apply its preset."); }
                                if !preset.factory() && ui.small_button("···").clicked() { self.rename = Some((preset.id.clone(), preset.name.clone())); ui.close(); }
                            });
                        });
                    }
                });
                ui.separator();
                ui.small(match scope {
                    Scope::Layout => "Pane count, placement, and sizing. Keeps current components.",
                    Scope::View => "Layout, component choices, and their settings.",
                    Scope::Component => "Settings for the selected component. Keeps the layout.",
                });
            });
            if ui.add_enabled(!cycling.is_empty(), egui::Button::new("›").min_size(Vec2::splat(NAV_CONTROL_HEIGHT))).help_text("Next preset in this category.").clicked() {
                actions.push(Action::Apply(cycle(&cycling, &storage.active, 1)));
            }
            if ui.add_enabled(has_panes, egui::Button::new("Save").min_size(Vec2::new(save_width, NAV_CONTROL_HEIGHT))).clicked() {
                self.save = Some(SaveDialog { name: String::new(), scope: current_scope });
            }
        });
        actions
    }
    pub fn library(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        theme: &AppTheme,
        can_add: bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        ui.painter().rect_filled(rect, 0.0, theme.navigation);
        ui.painter().vline(
            rect.right(),
            rect.y_range(),
            Stroke::new(1.0, theme.foreground.gamma_multiply(0.12)),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("component-library")
                .max_rect(rect),
        );
        // Reserve the boundary for the resize handle so a row's click/drag hit
        // area cannot add or drag a component when grabbing the drawer edge.
        let mut body_clip = rect.intersect(ui.clip_rect());
        body_clip.max.x -= 4.0;
        child.set_clip_rect(body_clip);
        sidebar_style(&mut child, theme);
        let close = egui::Frame::new()
            .inner_margin(12)
            .show(&mut child, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Components").strong());
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.visuals_mut().button_frame = false;
                        icons::sized_button(
                            ui,
                            Icon::Close,
                            false,
                            24.0,
                            "Close the component browser",
                        )
                    })
                    .inner
                })
                .inner
            })
            .inner;
        if close.clicked() {
            self.library_open = false;
        }
        egui::ScrollArea::vertical().id_salt("component-results").show(&mut child, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            // Keep spatial instruments first, then include every registered component.
            let spatial = [ModuleKind::Waterfall, ModuleKind::Sphere, ModuleKind::Cylinder, ModuleKind::Rings];
            for kind in spatial.into_iter().chain(ModuleKind::ALL.into_iter().filter(|kind| !spatial.contains(kind))) {
                if matches!(kind, ModuleKind::Waterfall | ModuleKind::Veroni) {
                    egui::Frame::new().inner_margin(12).show(ui, |ui| {
                        ui.label(egui::RichText::new(if kind == ModuleKind::Waterfall { "SCIENTIFIC INSTRUMENTS" } else { "VISUALIZERS" }).size(10.0).color(theme.muted));
                    });
                }
                ui.push_id(kind.label(), |ui| {
                    sidebar_rule(ui, rect);
                    let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click_and_drag());
                    let response = response.on_hover_cursor(egui::CursorIcon::Grab);
                    if response.hovered() || response.dragged() { ui.painter().rect_filled(row, 0.0, theme.card); }
                    let badge = Rect::from_center_size(egui::pos2(row.left() + 24.0, row.center().y), Vec2::splat(22.0));
                    icons::paint(ui, badge, kind.icon(), theme.foreground);
                    let labels = Rect::from_min_max(egui::pos2(badge.right() + 9.0, row.top()),
                        egui::pos2(row.right() - 32.0, row.bottom()));
                    // Paint labels without child widgets so the whole row keeps
                    // its click/drag interaction, including directly over text.
                    let mut job = egui::text::LayoutJob::simple(kind.label().into(), FontId::proportional(13.0), theme.foreground, labels.width());
                    job.wrap.max_rows = 1;
                    let galley = ui.painter().layout_job(job);
                    let position = egui::pos2(labels.left(), labels.center().y - galley.size().y * 0.5);
                    ui.painter_at(labels).galley(position, galley, theme.foreground);
                    paint_drag_grip(ui, Rect::from_center_size(egui::pos2(row.right() - 18.0, row.center().y), Vec2::splat(16.0)), theme.muted);
                    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Add {}", kind.label())));
                    response.dnd_set_drag_payload(DragPayload::Component(kind));
                    if response.clicked() && can_add { actions.push(Action::Add(kind)); }
                    response.help_text(format!("{} Drag to a pane edge to add, or its center to replace. Click to add beside the selected pane.", kind.description()));
                });
            }
            sidebar_rule(ui, rect);
            egui::Frame::new().inner_margin(12).show(ui, |ui| {
                ui.small(if can_add { "Drag to a pane edge to split, or its center to replace." }
                    else { "16 panes are open. Close one to add, or drag to replace." });
            });
        });
        actions
    }
    pub fn dialogs(
        &mut self,
        ctx: &egui::Context,
        storage: &Storage,
        selected_kind: ModuleKind,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let mut open = self.presets_open;
        if open {
            egui::Window::new("Visual preset library")
                .open(&mut open)
                .default_width(560.0)
                .show(ctx, |ui| {
                    ui.label("Reusable visual settings for the current scene.");
                    ui.small("Signal channels are preserved. Use scenes for complete setups.");
                    ui.allocate_ui(egui::vec2(ui.available_width(), NAV_HEIGHT), |ui| {
                        actions.extend(self.preset_toolbar(ui, storage, selected_kind, true));
                    });
                });
        }
        self.presets_open = open;

        let mut finish = false;
        if let Some(save) = &mut self.save {
            let response =
                egui::Modal::new(egui::Id::new("save-workspace-preset")).show(ctx, |ui| {
                    ui.set_width(360.0);
                    ui.heading("Save preset");
                    ui.add(
                        egui::TextEdit::singleline(&mut save.name)
                            .hint_text("Preset name")
                            .desired_width(f32::INFINITY),
                    );
                    ui.radio_value(
                        &mut save.scope,
                        Scope::Layout,
                        "Layout only — pane placement and sizes",
                    );
                    ui.radio_value(
                        &mut save.scope,
                        Scope::View,
                        "Full view — layout and component settings",
                    );
                    ui.radio_value(
                        &mut save.scope,
                        Scope::Component,
                        format!("Component — {} settings", selected_kind.label()),
                    );
                    let name = save.name.trim();
                    let duplicate = storage.presets.iter().any(|p| {
                        p.name.eq_ignore_ascii_case(name) && save.scope.matches(&p.content)
                    });
                    if duplicate {
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            "This name is already used in that category.",
                        );
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                workspace::valid_name(name)
                                    && !duplicate
                                    && storage.presets.iter().filter(|p| !p.factory()).count()
                                        < 256,
                                egui::Button::new("Save preset"),
                            )
                            .clicked()
                        {
                            actions.push(Action::Save(name.into(), save.scope));
                            finish = true;
                        }
                        if ui.button("Cancel").clicked() {
                            finish = true;
                        }
                    });
                });
            if response.should_close() {
                finish = true;
            }
        }
        if finish {
            self.save = None;
        }
        finish = false;
        if let Some((id, name)) = &mut self.rename {
            let response =
                egui::Modal::new(egui::Id::new("manage-workspace-preset")).show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.heading("Manage preset");
                    ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                    let same_scope = storage
                        .presets
                        .iter()
                        .find(|p| p.id == *id)
                        .map(|p| p.content.label());
                    let duplicate = storage.presets.iter().any(|p| {
                        p.id != *id
                            && p.name.eq_ignore_ascii_case(name.trim())
                            && Some(p.content.label()) == same_scope
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                workspace::valid_name(name.trim()) && !duplicate,
                                egui::Button::new("Rename"),
                            )
                            .clicked()
                        {
                            actions.push(Action::Rename(id.clone(), name.trim().into()));
                            finish = true;
                        }
                        if ui.button("Delete…").clicked() {
                            self.delete = Some(id.clone());
                            finish = true;
                        }
                        if ui.button("Cancel").clicked() {
                            finish = true;
                        }
                    });
                });
            if response.should_close() {
                finish = true;
            }
        }
        if finish {
            self.rename = None;
        }
        finish = false;
        if let Some(id) = &self.delete {
            let response =
                egui::Modal::new(egui::Id::new("delete-workspace-preset")).show(ctx, |ui| {
                    ui.heading("Delete preset?");
                    ui.label("The current workspace will stay as it is.");
                    ui.horizontal(|ui| {
                        if ui.button("Delete preset").clicked() {
                            actions.push(Action::Delete(id.clone()));
                            finish = true;
                        }
                        if ui.button("Cancel").clicked() {
                            finish = true;
                        }
                    });
                });
            if response.should_close() {
                finish = true;
            }
        }
        if finish {
            self.delete = None;
        }
        actions
    }
}
fn cycle(presets: &[&Preset], active: &str, delta: isize) -> String {
    let current = presets.iter().position(|p| p.id == active).unwrap_or(0);
    let next = (current as isize + delta).rem_euclid(presets.len() as isize) as usize;
    presets[next].id.clone()
}
fn thumbnail(ui: &egui::Ui, tree: &workspace::Tile, rect: Rect, color: Color32) {
    for (_, cell) in tree.geometry(rect).0 {
        ui.painter()
            .rect_filled(cell, 1.0, color.gamma_multiply(0.18));
        ui.painter()
            .rect_stroke(cell, 1.0, Stroke::new(0.7, color), egui::StrokeKind::Inside);
    }
}

pub fn drag_grip(ui: &mut egui::Ui, id: usize) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(18.0, 24.0), Sense::drag());
    let color = ui.visuals().weak_text_color();
    paint_drag_grip(ui, rect, color);
    response.dnd_set_drag_payload(DragPayload::Pane(id));
    response.on_hover_cursor(egui::CursorIcon::Grab).help_text(
        "Drag this pane to an edge to move it, or onto another pane's center to swap positions.",
    )
}

fn paint_drag_grip(ui: &egui::Ui, rect: Rect, color: Color32) {
    for x in [-2.5, 2.5] {
        for y in [-5.0, 0.0, 5.0] {
            ui.painter()
                .circle_filled(rect.center() + Vec2::new(x, y), 1.1, color);
        }
    }
}
pub fn drop_hint(ui: &egui::Ui, rect: Rect, message: &str, accent: Color32) {
    let painter = ui.painter_at(ui.clip_rect());
    painter.rect_filled(rect, 6.0, accent.gamma_multiply(0.22));
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(2.0, accent),
        egui::StrokeKind::Inside,
    );
    let label = painter.layout_no_wrap(message.into(), FontId::proportional(12.0), Color32::WHITE);
    let size = label.size() + Vec2::new(16.0, 10.0);
    let backing = Rect::from_center_size(rect.center(), size);
    painter.rect_filled(backing, 4.0, Color32::from_black_alpha(225));
    painter.galley(backing.min + Vec2::new(8.0, 5.0), label, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Pos2;
    #[test]
    fn component_drawer_exposes_every_registered_visualizer() {
        let ctx = egui::Context::default();
        let mut manager = Manager::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 1800.0));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ui| {
                manager.library(ui, rect, &AppTheme::default(), true);
            },
        );
        for kind in ModuleKind::ALL {
            assert_eq!(output.shapes.iter().filter(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == kind.label())).count(), 1, "{} is available exactly once", kind.label());
        }
        output.textures_delta.clear();
    }
    #[test]
    fn preset_navigation_wraps_within_the_filtered_category() {
        let presets = workspace::factory_presets();
        let views: Vec<_> = presets
            .iter()
            .filter(|p| Scope::View.matches(&p.content))
            .collect();
        assert_eq!(cycle(&views, &views[0].id, -1), views.last().unwrap().id);
        assert_eq!(cycle(&views, &views.last().unwrap().id, 1), views[0].id);
    }
    #[test]
    fn drop_targets_are_unambiguous_at_edges_and_center() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0));
        for (point, expected) in [
            (Pos2::new(5.0, 200.0), workspace::Edge::Left),
            (Pos2::new(395.0, 200.0), workspace::Edge::Right),
            (Pos2::new(200.0, 5.0), workspace::Edge::Top),
            (Pos2::new(200.0, 395.0), workspace::Edge::Bottom),
            (rect.center(), workspace::Edge::Center),
        ] {
            assert_eq!(workspace::drop_zone(rect, point).0, expected);
        }
    }
}
