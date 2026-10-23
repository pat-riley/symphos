//! A development-only living reference, built from production controls.
use crate::{
    icons::{self, Icon},
    parameter::Parameter,
    theme::AppTheme,
};
use egui::{self, RichText};

pub struct Gallery {
    pub open: bool,
    tab: usize,
    value: f32,
    toggle: bool,
    checklist: [bool; 7],
}
impl Default for Gallery {
    fn default() -> Self {
        Self {
            open: false,
            tab: 0,
            value: 0.5,
            toggle: true,
            checklist: [false; 7],
        }
    }
}
impl Gallery {
    pub fn show(&mut self, ctx: &egui::Context, theme: &AppTheme) {
        if !self.open {
            return;
        }
        egui::Window::new("Design System & Release Readiness").open(&mut self.open).default_size([760.0,620.0]).resizable(true).show(ctx,|ui|{
            ui.heading("Symphos · UI reference");
            ui.weak("Development only · production controls, theme tokens and unfinished release assets.");
            ui.horizontal_wrapped(|ui|{for (i,label) in ["Foundations","Controls","Iconography","Brand assets","Release checklist"].iter().enumerate(){ui.selectable_value(&mut self.tab,i,*label);}});
            ui.separator();
            egui::ScrollArea::vertical().show(ui,|ui|match self.tab {
                0=>{
                    ui.heading("Color roles");
                    for (name,color) in [("Window / navigation",theme.navigation),("Workspace",theme.background),("Panel",theme.panel),("Card",theme.card),("Foreground",theme.foreground),("Secondary text",theme.muted),("Accent",theme.accent),("Selection",theme.selection),("Warning",theme.warning),("Error",theme.error)] {
                        ui.horizontal(|ui|{let (rect,_)=ui.allocate_exact_size(egui::vec2(36.0,24.0),egui::Sense::hover());ui.painter().rect_filled(rect,4.0,color);ui.label(name);ui.monospace(format!("#{:02X}{:02X}{:02X}",color.r(),color.g(),color.b()));});
                    }
                    ui.separator();ui.heading("Information hierarchy");
                    ui.label("Component identity > Section > Group > Parameter > Help.");
                    ui.weak("Presets, copy/paste and destructive resets belong in Component actions. Values stay aligned. Visualizers share Geometry, Appearance and Camera tabs under a pinned identity and audio source. Flat views omit Camera. Signals, insert editors and Macros use the same skin and pinned header, with untabbed collapsible property groups. Help explains behavior and units.");
                    ui.separator();ui.heading("Typography & spacing");
                    ui.label(RichText::new("Section title").strong());ui.label("Parameter label");ui.monospace("0.125 · numeric value");ui.weak("Secondary description / hint");
                    ui.label("Inspectors: 12 pt labels, 22 pt property rows, 10 pt side insets, 6 pt row gaps, aligned values and compact panel-colored dropdowns. Workspace controls: 24–28 pt; pane corners: 7 pt.");
                },
                1=>{
                    ui.heading("Numeric controls");
                    ui.add(Parameter::new(&mut self.value,0.0..=1.0,0.5).text("Example amount"));
                    ui.weak("Drag to adjust; Shift for precision; click value for exact entry; double-click to reset. Inspector labels are draggable macro assignments.");
                    ui.checkbox(&mut self.toggle,"Enabled");
                    ui.horizontal(|ui|{let _=ui.button("Primary action");ui.add_enabled(false,egui::Button::new("Unavailable"));ui.menu_button("Secondary actions",|ui|{ui.label("Group infrequent actions here.");});});
                    ui.separator();ui.heading("State examples");
                    ui.colored_label(theme.accent,"Active / assigned");ui.weak("Inactive / unassigned");ui.colored_label(theme.warning,"Needs attention");ui.colored_label(theme.error,"Action failed — explain how to recover");
                    ui.separator();ui.heading("Empty state");
                    egui::Frame::group(ui.style()).show(ui,|ui|{ui.set_min_height(80.0);ui.label("No visualizers yet");ui.weak("Click + and drag a visualizer into the workspace.");});
                },
                2=>{
                    ui.heading("Shared outline icons");
                    ui.horizontal_wrapped(|ui|{for (icon,label) in [(Icon::Plus,"Add"),(Icon::Inspector,"Inspector"),(Icon::Save,"Save"),(Icon::Play,"Play"),(Icon::Pause,"Pause"),(Icon::Camera,"Camera"),(Icon::Clock,"Motion"),(Icon::Settings,"Settings"),(Icon::Palette,"Appearance"),(Icon::Knob,"Macros"),(Icon::Levels,"Signals"),(Icon::GridView,"Grid"),(Icon::ListView,"List"),(Icon::Close,"Close")] {let _=icons::sized_button(ui,icon,false,28.0,label);}});
                    ui.weak("Use the shared icon painter. Avoid mixing filled toolbar buttons and outline sidebar toggles for equivalent actions.");
                },
                3=>{
                    ui.heading("Brand assets · placeholders");
                    for (title,description) in [("Wordmark & identity","Placeholder: approved logo, typography, spacing and light/dark variants."),("Application icon","Placeholder: final macOS icon artwork and full resolution export set. Current development icon is provisional."),("Splash / launch experience","Placeholder: decide whether a launch screen is useful; prepare artwork and accessible progress treatment if needed."),("About & credits","Placeholder: final version details, website, contributor credits and third-party notices."),("Store / release imagery","Placeholder: screenshots, product description, demo video and downloadable press assets.")] {
                        egui::Frame::group(ui.style()).show(ui,|ui|{ui.strong(title);ui.weak(description);});ui.add_space(6.0);
                    }
                },
                _=>{
                    ui.heading("Release readiness · planning only");
                    ui.weak("Session-only checklist; these marks do not certify a release.");
                    for (done,label) in self.checklist.iter_mut().zip(["Final brand and icon assets","Keyboard navigation and accessibility pass","Project compatibility and recovery verification","120 FPS frame-budget audit on supported hardware","Audio permissions and MIDI device recovery","Recording audio/video sync and long-session tests","Signing, notarization, licensing and installer verification"]) {ui.checkbox(done,label);}
                }
            });
        });
    }
}
