//! Flux's normalized performance controls and transient runtime; ready for a future MIDI adapter.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Macro {
    Brightness,
    Speed,
    Expansion,
    Impact,
    Swirl,
    Turbulence,
    Magnetism,
    Shape,
    Depth,
    Trails,
    Glow,
    Color,
}
impl Macro {
    pub const ALL: [Self; 12] = [
        Self::Brightness,
        Self::Speed,
        Self::Expansion,
        Self::Impact,
        Self::Swirl,
        Self::Turbulence,
        Self::Magnetism,
        Self::Shape,
        Self::Depth,
        Self::Trails,
        Self::Glow,
        Self::Color,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Brightness => "brightness",
            Self::Speed => "speed",
            Self::Expansion => "expansion",
            Self::Impact => "impact",
            Self::Swirl => "swirl",
            Self::Turbulence => "turbulence",
            Self::Magnetism => "magnetism",
            Self::Shape => "orbit-shape",
            Self::Depth => "depth",
            Self::Trails => "trails",
            Self::Glow => "glow",
            Self::Color => "color-drift",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Brightness => "Brightness",
            Self::Speed => "Speed",
            Self::Expansion => "Expansion",
            Self::Impact => "Impact",
            Self::Swirl => "Swirl",
            Self::Turbulence => "Turbulence",
            Self::Magnetism => "Magnetism",
            Self::Shape => "Shape",
            Self::Depth => "Depth",
            Self::Trails => "Trails",
            Self::Glow => "Glow",
            Self::Color => "Color drift",
        }
    }
    fn help(self) -> &'static str {
        match self {
            Self::Brightness => {
                "Master visual light level. Music still modulates illumination. Zero blacks out Flux without changing the audio."
            }
            Self::Speed => {
                "Base particle pacing; music adds movement. Zero stops particles while light can still react."
            }
            Self::Expansion => {
                "Resting size of the formation. Audio adds expansion on top, leaving room for a drop."
            }
            Self::Impact => {
                "Emphasize rising musical energy with short outward, brightness, and momentum bursts. Zero removes this extra accent; normal mappings still react."
            }
            Self::Swirl => {
                "Reverse, slow, or accelerate circulation. Middle is neutral; audio adds currents."
            }
            Self::Turbulence => {
                "Smooth paths to turbulent filaments; high-frequency audio adds fine motion."
            }
            Self::Magnetism => {
                "Free 3D cloud to tightly held orbital paths. Shape defines the orbit you pull toward."
            }
            Self::Shape => {
                "Morph continuously through Rings, Helix, Knot, Vortex, and Petals. Raise Magnetism to reveal the formation."
            }
            Self::Depth => "Flatten the view or stretch it into a deeper volume.",
            Self::Trails => {
                "Length of the particle streaks. Impact briefly extends them on musical accents."
            }
            Self::Glow => "Soft halo around particles and luminous cores.",
            Self::Color => {
                "Rate of palette travel through the cloud. Music still affects color and light."
            }
        }
    }
    fn range(self) -> (f32, f32) {
        match self {
            Self::Brightness | Self::Speed | Self::Impact => (0.0, 2.0),
            Self::Expansion => (0.35, 2.0),
            Self::Swirl => (-3.0, 3.0),
            Self::Turbulence => (0.0, 2.0),
            Self::Depth => (0.2, 2.0),
            _ => (0.0, 1.0),
        }
    }
    pub fn raw(self, s: &FluxSettings) -> f32 {
        match self {
            Self::Brightness => s.brightness,
            Self::Speed => s.speed,
            Self::Expansion => s.expansion,
            Self::Impact => s.impact,
            Self::Swirl => s.swirl,
            Self::Turbulence => s.turbulence,
            Self::Magnetism => s.orbit_magnetism,
            Self::Shape => s.orbit_shape,
            Self::Depth => s.depth,
            Self::Trails => s.trails,
            Self::Glow => s.glow,
            Self::Color => s.color_drift,
        }
    }
    pub fn normalized(self, s: &FluxSettings) -> f32 {
        let (a, b) = self.range();
        ((self.raw(s) - a) / (b - a)).clamp(0.0, 1.0)
    }
    pub fn set_normalized(self, s: &mut FluxSettings, v: f32) {
        let (a, b) = self.range();
        let value = a + finite(v, self.normalized(s)).clamp(0.0, 1.0) * (b - a);
        let field = match self {
            Self::Brightness => &mut s.brightness,
            Self::Speed => &mut s.speed,
            Self::Expansion => &mut s.expansion,
            Self::Impact => &mut s.impact,
            Self::Swirl => &mut s.swirl,
            Self::Turbulence => &mut s.turbulence,
            Self::Magnetism => &mut s.orbit_magnetism,
            Self::Shape => &mut s.orbit_shape,
            Self::Depth => &mut s.depth,
            Self::Trails => &mut s.trails,
            Self::Glow => &mut s.glow,
            Self::Color => &mut s.color_drift,
        };
        *field = value;
    }
    pub fn target(self) -> Option<Target> {
        match self {
            Self::Brightness => Some(Target::Brightness),
            Self::Speed => Some(Target::Motion),
            Self::Expansion | Self::Impact => Some(Target::Expansion),
            Self::Swirl => Some(Target::Swirl),
            Self::Turbulence => Some(Target::Turbulence),
            _ => None,
        }
    }
}
fn finite(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v } else { fallback }
}

#[derive(Default)]
pub(super) struct Performance {
    current: Option<[f32; 12]>,
    pub selected: Option<Macro>,
    pub pulse: f32,
    previous_rise: f32,
    fast: f32,
    slow: f32,
    cooldown: f32,
    pub impulse: f32,
    pub manual_burst: bool,
}
impl Performance {
    pub fn value(&self, s: &FluxSettings, m: Macro) -> f32 {
        self.current.map_or_else(|| m.raw(s), |v| v[m as usize])
    }
    pub fn advance(&mut self, s: &FluxSettings, input: f32, dt: f32) {
        let target = Macro::ALL.map(|m| m.raw(s));
        let current = self.current.get_or_insert(target);
        for (v, t) in current.iter_mut().zip(target) {
            *v += (t - *v) * (1.0 - (-dt / 0.065).exp());
            if (*v - t).abs() < 0.0001 {
                *v = t;
            }
        }
        self.fast += (input - self.fast) * (1.0 - (-dt / 0.018).exp());
        self.slow += (input - self.slow) * (1.0 - (-dt / 0.45).exp());
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.pulse *= (-dt / 0.32).exp();
        self.impulse = 0.0;
        let rise = self.fast - self.slow;
        if self.manual_burst
            || (self.cooldown == 0.0 && input > 0.08 && rise > 0.05 && self.previous_rise <= 0.05)
        {
            let strength = if self.manual_burst {
                1.0
            } else {
                (input * 0.9 + rise * 0.5).clamp(0.0, 1.0)
            };
            self.pulse = self.pulse.max(strength);
            self.impulse = strength;
            self.cooldown = 0.18;
            self.manual_burst = false;
        }
        self.previous_rise = rise;
    }
}
pub(super) fn orbit_position(shape: f32, a: f32, home: [f32; 3]) -> [f32; 3] {
    let forms = |i| match i {
        0 => [a.cos() * 0.85, 0.0, a.sin() * 0.85],
        1 => {
            let theta = a + home[1] * 3.2;
            [theta.cos() * 0.65, home[1] * 1.1, theta.sin() * 0.65]
        }
        2 => {
            let r = 0.65 + 0.24 * (3.0 * a).cos();
            [
                r * (2.0 * a).cos(),
                0.24 * (3.0 * a).sin(),
                r * (2.0 * a).sin(),
            ]
        }
        3 => {
            let y = home[1];
            let r = 0.15 + (y + 1.0) * 0.5;
            let theta = a + y * 3.0;
            [r * theta.cos(), y, r * theta.sin()]
        }
        _ => {
            let r = 0.65 + 0.25 * (5.0 * a).cos();
            [r * a.cos(), 0.32 * (3.0 * a).sin(), r * a.sin()]
        }
    };
    let x = shape.clamp(0.0, 1.0) * 4.0;
    let i = (x.floor() as usize).min(3);
    let blend = x - i as f32;
    if blend <= 0.00001 {
        return forms(i);
    }
    if blend >= 0.99999 {
        return forms(i + 1);
    }
    let from = forms(i);
    let to = forms(i + 1);
    std::array::from_fn(|k| from[k] + (to[k] - from[k]) * (x - i as f32))
}
pub(super) fn shape_label(shape: f32) -> String {
    let names = ["Rings", "Helix", "Knot", "Vortex", "Petals"];
    let x = shape.clamp(0.0, 1.0) * 4.0;
    let i = x.floor() as usize;
    if x.fract() < 0.025 || i == 4 {
        names[i].into()
    } else {
        format!("{} / {}", names[i], names[i + 1])
    }
}
pub(crate) fn bar_height(width: f32, open: bool) -> f32 {
    if !open {
        30.0
    } else if width >= 900.0 {
        156.0
    } else {
        258.0
    }
}
impl Flux {
    pub(crate) fn toggle_performance(&mut self) {
        self.settings.performance_open = !self.settings.performance_open;
    }
    pub(crate) fn performance_height(&self, width: f32) -> f32 {
        bar_height(width, self.settings.performance_open)
    }
    pub(crate) fn performance_bar(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        theme: &AppTheme,
        pane: usize,
    ) -> bool {
        ui.painter().rect_filled(rect, 0.0, theme.panel);
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            egui::Stroke::new(1.0, theme.foreground.gamma_multiply(0.18)),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("flux-performance", pane))
                .max_rect(rect.shrink2(Vec2::new(10.0, 5.0))),
        );
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        child.spacing_mut().item_spacing = Vec2::new(6.0, 3.0);
        child.spacing_mut().interact_size.y = 22.0;
        child.style_mut().override_font_id = Some(egui::FontId::proportional(12.0));
        let mut inspect = false;
        child.horizontal(|ui| {
            if ui.button(if self.settings.performance_open{"Flux macros"}else{"Show Flux macros"}).clicked() {self.settings.performance_open = !self.settings.performance_open;}
            ui.label(format!("Panel {}",pane+1));
            if self.settings.performance_open {
                let formation = if self.performance.value(&self.settings, Macro::Magnetism) < 0.05 { "Cloud".into() } else { shape_label(self.performance.value(&self.settings,Macro::Shape)) };
                ui.menu_button(formation, |ui| {
                    if ui.button("Cloud").clicked() {Macro::Magnetism.set_normalized(&mut self.settings,0.0);ui.close();}
                    for (i,name) in ["Rings","Helix","Knot","Vortex","Petals"].into_iter().enumerate() {
                        if ui.button(name).clicked() {Macro::Shape.set_normalized(&mut self.settings,i as f32/4.0);Macro::Magnetism.set_normalized(&mut self.settings,0.9);self.performance.selected=Some(Macro::Shape);ui.close();}
                    }
                }).response.help_text("Choose a formation. Orbital choices raise Magnetism and smoothly reshape the same particles. Cloud releases them again.");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                    if ui.button("Hide").clicked(){self.settings.performance_open=false;}
                    if ui.button("Inspect").help_text("Open the right inspector for the last-touched macro or its audio mapping.").clicked(){inspect=true;}
                    if ui.button("Burst").help_text("Launch a short accent using the Impact fader, without waiting for an audio onset.").clicked(){self.performance.manual_burst=true;}
                });
            }
        });
        if !self.settings.performance_open {
            return false;
        }
        let cols = if rect.width() >= 900.0 { 12 } else { 6 };
        let width = (child.available_width() - (cols - 1) as f32 * 6.0) / cols as f32;
        for row in Macro::ALL.chunks(cols) {
            child.horizontal(|ui| {
                for &m in row {
                    ui.push_id(m.id(), |ui| {
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            let mut v = m.normalized(&self.settings);
                            let activity = if m == Macro::Impact {
                                self.performance.pulse
                            } else {
                                m.target()
                                    .map_or(0.0, |t| self.modulator.value(t).abs().min(1.0))
                            };
                            let response = macro_control(ui, m, &mut v, width, theme, activity);
                            if response.clicked() || response.dragged() {
                                self.performance.selected = Some(m);
                            }
                            if response.changed() {
                                m.set_normalized(&mut self.settings, v);
                                self.performance.selected = Some(m);
                            }
                        });
                    });
                }
            });
        }
        child.label(egui::RichText::new("Drag to play · Shift: fine · Double-click: reset · music accents continue underneath").color(theme.muted));
        if inspect
            && let Some(m) = self.performance.selected
            && let Some(t) = m.target()
        {
            self.modulator
                .select_target(t, self.settings.modulation.as_ref().unwrap());
        }
        inspect
    }
    pub(crate) fn performance_section(&self) -> super::super::SettingsSection {
        use super::super::SettingsSection;
        match self.performance.selected {
            Some(Macro::Depth) => SettingsSection::Geometry,
            Some(Macro::Trails | Macro::Glow) => SettingsSection::Particles,
            Some(Macro::Color) => SettingsSection::Appearance,
            Some(Macro::Magnetism | Macro::Shape) => SettingsSection::Flow,
            _ => SettingsSection::Modulation,
        }
    }
}
fn macro_control(
    ui: &mut egui::Ui,
    m: Macro,
    v: &mut f32,
    width: f32,
    theme: &AppTheme,
    activity: f32,
) -> egui::Response {
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, 94.0), egui::Sense::click_and_drag());
    let response_id = response.id;
    response = response
        .on_hover_cursor(egui::CursorIcon::ResizeVertical)
        .help_text(m.help());
    let before = *v;
    if response.double_clicked() {
        *v = m.normalized(&FluxSettings::default());
    } else if response.dragged() {
        *v = (*v
            - response.drag_delta().y
                / if ui.input(|i| i.modifiers.shift) {
                    1000.0
                } else {
                    130.0
                })
        .clamp(0.0, 1.0);
    }
    if response.clicked() {
        response.request_focus();
        if (m as usize) < 4
            && let Some(pos) = response.interact_pointer_pos()
        {
            let top = rect.top() + 22.0;
            let bottom = rect.top() + 66.0;
            if (top..=bottom).contains(&pos.y) {
                *v = ((bottom - pos.y) / 44.0).clamp(0.0, 1.0);
            }
        }
    }
    if response.has_focus() {
        let step = if ui.input(|i| i.modifiers.shift) {
            0.002
        } else {
            0.02
        };
        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowRight)) {
            *v = (*v + step).min(1.0);
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowLeft))
        {
            *v = (*v - step).max(0.0);
        }
    }
    let p = ui.painter_at(rect);
    let center = Pos2::new(rect.center().x, rect.top() + 44.0);
    p.text(
        Pos2::new(rect.center().x, rect.top()),
        egui::Align2::CENTER_TOP,
        m.label(),
        egui::FontId::proportional(12.0),
        theme.foreground,
    );
    if (m as usize) < 4 {
        let track = Rect::from_center_size(center, Vec2::new(5.0, 44.0));
        p.rect_filled(track, 2.0, theme.foreground.gamma_multiply(0.12));
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(track.left(), track.bottom() - *v * track.height()),
                track.max,
            ),
            2.0,
            theme.accent.gamma_multiply(0.65),
        );
        let y = track.bottom() - *v * track.height();
        p.rect_filled(
            Rect::from_center_size(Pos2::new(center.x, y), Vec2::new(22.0, 6.0)),
            2.0,
            theme.foreground,
        );
        p.rect_filled(
            Rect::from_min_size(
                Pos2::new(center.x + 12.0, track.bottom() - activity * 44.0),
                Vec2::new(2.0, activity * 44.0),
            ),
            0.0,
            theme.accent,
        );
    } else {
        let start = 2.35;
        let sweep = 4.72;
        let arc = |fraction: f32, r: f32| {
            (0..=32)
                .map(|i| center + Vec2::angled(start + sweep * fraction * i as f32 / 32.0) * r)
                .collect::<Vec<_>>()
        };
        p.circle_filled(center, 17.0, theme.card);
        p.add(egui::Shape::line(
            arc(1.0, 21.0),
            egui::Stroke::new(2.0, theme.foreground.gamma_multiply(0.14)),
        ));
        p.add(egui::Shape::line(
            arc(*v, 21.0),
            egui::Stroke::new(2.0, theme.accent),
        ));
        let direction = Vec2::angled(start + sweep * *v);
        p.line_segment(
            [center + direction * 6.0, center + direction * 14.0],
            egui::Stroke::new(2.0, theme.foreground),
        );
        if activity > 0.01 {
            p.add(egui::Shape::line(
                arc(activity, 25.0),
                egui::Stroke::new(1.0, theme.accent_alt.gamma_multiply(0.65)),
            ));
        }
    }
    let field = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.bottom() - 14.0),
        Vec2::new(width.min(62.0), 22.0),
    );
    let mut value_ui = ui.new_child(egui::UiBuilder::new().id_salt(response_id).max_rect(field));
    let mut percent = *v * 100.0;
    let edit = value_ui.add(
        egui::DragValue::new(&mut percent)
            .range(0.0..=100.0)
            .speed(0.2)
            .max_decimals(1)
            .suffix("%"),
    );
    if edit.changed() {
        *v = percent / 100.0;
    }
    if before != *v {
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::slider(true, f64::from(*v * 100.0), m.label()));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_controls_round_trip_and_save_without_transient_state() {
        let mut settings = FluxSettings::default();
        let mut ids = std::collections::HashSet::new();
        for m in Macro::ALL {
            assert!(ids.insert(m.id()));
            m.set_normalized(&mut settings, 0.72);
            assert!((m.normalized(&settings) - 0.72).abs() < 1e-5);
        }
        settings.sanitize();
        let encoded = serde_json::to_string(&settings).unwrap();
        let restored: FluxSettings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(settings, restored);
        assert!(
            !encoded.contains("pulse")
                && !encoded.contains("current")
                && !encoded.contains("manual_burst")
        );
        for m in Macro::ALL {
            m.set_normalized(&mut settings, f32::NAN);
            assert!(m.raw(&settings).is_finite());
        }
        let mut legacy = serde_json::to_value(&settings).unwrap();
        for field in [
            "brightness",
            "expansion",
            "impact",
            "orbit_shape",
            "color_drift",
            "performance_open",
        ] {
            legacy.as_object_mut().unwrap().remove(field);
        }
        let legacy: FluxSettings = serde_json::from_value(legacy).unwrap();
        assert!(legacy.performance_open);
        assert_eq!(legacy.brightness, 1.0);
        assert_eq!(legacy.expansion, 1.0);
    }
    #[test]
    fn sustained_input_produces_one_accent_then_new_onsets_and_manual_burst_retrigger() {
        let settings = FluxSettings::default();
        let mut p = Performance::default();
        let mut accents = 0;
        for _ in 0..480 {
            p.advance(&settings, 0.8, STEP);
            if p.impulse > 0.0 {
                accents += 1;
            }
        }
        assert_eq!(accents, 1);
        assert!(p.pulse < 0.0001);
        for _ in 0..120 {
            p.advance(&settings, 0.0, STEP);
        }
        for _ in 0..60 {
            p.advance(&settings, 0.8, STEP);
            if p.impulse > 0.0 {
                accents += 1;
            }
        }
        assert_eq!(accents, 2);
        p.manual_burst = true;
        p.advance(&settings, 0.0, STEP);
        assert_eq!(p.pulse, 1.0);
        assert_eq!(p.impulse, 1.0);
        p.advance(&settings, 0.0, STEP);
        assert_eq!(p.impulse, 0.0);
        assert!(p.pulse < 1.0);
    }
    #[test]
    fn live_values_slew_and_speed_reaches_zero_then_recovers() {
        let mut settings = FluxSettings::default();
        let mut p = Performance::default();
        p.advance(&settings, 0.0, STEP);
        settings.speed = 0.0;
        p.advance(&settings, 0.0, STEP);
        assert!(p.value(&settings, Macro::Speed) > 0.0 && p.value(&settings, Macro::Speed) < 1.0);
        for _ in 0..120 {
            p.advance(&settings, 0.0, STEP);
        }
        assert_eq!(p.value(&settings, Macro::Speed), 0.0);
        settings.speed = 1.0;
        p.advance(&settings, 0.0, STEP);
        assert!(p.value(&settings, Macro::Speed) > 0.0);
    }
    #[test]
    fn orbital_shapes_are_distinct_bounded_and_continuous_when_morphed() {
        let home = [0.3, 0.6, -0.4];
        let shapes: Vec<Vec<_>> = [0.0, 0.25, 0.5, 0.75, 1.0]
            .into_iter()
            .map(|shape| {
                (0..96)
                    .map(|i| orbit_position(shape, i as f32 * TAU / 96.0, home))
                    .collect()
            })
            .collect();
        for a in 0..5 {
            for b in a + 1..5 {
                let difference: f32 = shapes[a]
                    .iter()
                    .zip(&shapes[b])
                    .map(|(a, b)| length(std::array::from_fn(|k| a[k] - b[k])))
                    .sum();
                assert!(
                    difference > 20.0,
                    "forms {a} and {b} should have distinct geometry"
                );
            }
        }
        for i in 0..1000 {
            let shape = i as f32 / 1000.0;
            let a = orbit_position(shape, 1.23, home);
            let b = orbit_position(shape + 0.0001, 1.23, home);
            assert!(a.iter().all(|v| v.is_finite()));
            assert!(length(a) < 1.8);
            assert!(length(std::array::from_fn(|k| a[k] - b[k])) < 0.002);
        }
    }
    #[test]
    fn bar_fits_one_and_two_rows_and_exposes_all_controls() {
        for width in [490.0, 760.0, 1280.0] {
            let ctx = egui::Context::default();
            let mut flux = Flux::default();
            let theme = AppTheme::default();
            let rect = Rect::from_min_size(
                Pos2::new(8.0, 8.0),
                Vec2::new(width, bar_height(width, true)),
            );
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(width + 16.0, 400.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    flux.performance_bar(ui, rect, &theme, 0);
                },
            );
            for m in Macro::ALL {
                let text = out
                    .shapes
                    .iter()
                    .find_map(|shape| {
                        if let egui::Shape::Text(t) = &shape.shape {
                            (t.galley.job.text == m.label()).then_some(t)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                assert!(
                    rect.contains_rect(Rect::from_min_size(text.pos, text.galley.size())),
                    "{} at {width}",
                    m.label()
                );
            }
            out.textures_delta.clear();
        }
    }
    #[test]
    fn fader_and_knob_drag_change_performance_values() {
        for m in [Macro::Brightness, Macro::Swirl] {
            let ctx = egui::Context::default();
            let theme = AppTheme::default();
            let mut v = 0.2;
            let mut render = |events| {
                let mut control = Rect::NOTHING;
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 300.0))),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        control = macro_control(ui, m, &mut v, 80.0, &theme, 0.5).rect;
                    },
                );
                out.textures_delta.clear();
                control
            };
            let rect = render(vec![]);
            let a = rect.min + Vec2::new(40.0, 45.0);
            let b = a - Vec2::new(0.0, 25.0);
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            render(vec![egui::Event::PointerMoved(a), button(a, true)]);
            render(vec![egui::Event::PointerMoved(b)]);
            render(vec![button(b, false)]);
            assert!(v > 0.35);
        }
    }
}
