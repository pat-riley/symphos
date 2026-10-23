//! Shared command catalog for the native macOS menu and the desktop fallback.
use crate::{modules::ModuleKind, signals::Method};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    New,
    Open,
    Save,
    SaveAs,
    Settings,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    CopySettings,
    PasteSettings,
    Components,
    Inspector,
    Macros,
    Signals,
    Info,
    PanelFullscreen,
    Fullscreen,
    Balance,
    ZoomIn,
    ZoomOut,
    ActualSize,
    Pause,
    PauseAll,
    ResetComponent,
    CloseComponent,
    ClearHistory,
    ResetMeters,
    NewScene,
    DuplicateScene,
    RenameScene,
    DeleteScene,
    PreviousScene,
    NextScene,
    Presets,
    DisplaySettings,
    ThemeSettings,
    AudioSettings,
    MidiSettings,
    AnalysisSettings,
    Shortcuts,
    Issues,
    FftDiagnostics,
    About,
    Performance,
    Recording,
    #[cfg_attr(not(feature = "development"), allow(dead_code))]
    DesignSystem,
    Add(ModuleKind),
    Signal(Method),
}
impl Action {
    pub fn code(self) -> i32 {
        match self {
            Self::Open => 1,
            Self::Save => 2,
            Self::SaveAs => 3,
            Self::New => 4,
            Self::Settings => 5,
            Self::Undo => 16,
            Self::Redo => 17,
            Self::Cut => 10,
            Self::Copy => 11,
            Self::Paste => 12,
            Self::SelectAll => 13,
            Self::CopySettings => 14,
            Self::PasteSettings => 15,
            Self::Components => 20,
            Self::Inspector => 21,
            Self::Macros => 22,
            Self::Signals => 23,
            Self::Info => 24,
            Self::PanelFullscreen => 25,
            Self::Fullscreen => 26,
            Self::Balance => 27,
            Self::ZoomIn => 28,
            Self::ZoomOut => 29,
            Self::ActualSize => 30,
            Self::Pause => 40,
            Self::PauseAll => 41,
            Self::ResetComponent => 42,
            Self::CloseComponent => 43,
            Self::ClearHistory => 44,
            Self::ResetMeters => 45,
            Self::NewScene => 50,
            Self::DuplicateScene => 51,
            Self::RenameScene => 52,
            Self::DeleteScene => 53,
            Self::PreviousScene => 54,
            Self::NextScene => 55,
            Self::Presets => 56,
            Self::DisplaySettings => 60,
            Self::ThemeSettings => 61,
            Self::AudioSettings => 62,
            Self::MidiSettings => 63,
            Self::AnalysisSettings => 64,
            Self::Shortcuts => 70,
            Self::Issues => 71,
            Self::FftDiagnostics => 72,
            Self::About => 73,
            Self::Performance => 74,
            Self::Recording => 76,
            Self::DesignSystem => 75,
            Self::Add(kind) => 100 + kind as i32,
            Self::Signal(method) => 200 + method as i32,
        }
    }
    #[cfg(any(test, target_os = "macos"))]
    pub fn from_code(code: i32) -> Option<Self> {
        commands()
            .find(|command| command.action.code() == code)
            .map(|c| c.action)
    }
    pub fn append_text_events(self, events: &mut Vec<egui::Event>) -> bool {
        let Some(mut event) = self.text_event() else {
            return false;
        };
        events.push(event.clone());
        // Select All is synthesized as a complete keystroke, so A never remains
        // logically held after choosing the command with the mouse.
        if let egui::Event::Key { pressed, .. } = &mut event {
            *pressed = false;
            events.push(event);
        }
        true
    }
    fn text_event(self) -> Option<egui::Event> {
        match self {
            Self::Undo | Self::Redo => Some(egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: if self == Self::Redo {
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
                } else {
                    egui::Modifiers::COMMAND
                },
            }),
            Self::Cut => Some(egui::Event::Cut),
            Self::Copy => Some(egui::Event::Copy),
            Self::SelectAll => Some(egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }),
            _ => None,
        }
    }
}
// Bridge modifier bits; converted to NSEvent flags in project_menu.m.
const CMD: u32 = 1;
const SHIFT: u32 = 2;
const ALT: u32 = 4;
#[derive(Clone)]
pub struct Command {
    pub action: Action,
    pub title: &'static str,
    pub key: &'static str,
    pub modifiers: u32,
}
pub enum Entry {
    Command(Command),
    Separator,
    Menu(&'static str, Vec<Entry>),
}
fn item(action: Action, title: &'static str, key: &'static str, modifiers: u32) -> Entry {
    Entry::Command(Command {
        action,
        title,
        key,
        modifiers,
    })
}
fn plain(action: Action, title: &'static str) -> Entry {
    item(action, title, "", 0)
}
pub fn menus() -> &'static [Entry] {
    static MENUS: std::sync::OnceLock<Vec<Entry>> = std::sync::OnceLock::new();
    MENUS.get_or_init(|| {
        use Action::*;
        use Entry::{Menu, Separator as Sep};
        #[allow(unused_mut)]
        let mut menus = vec![
            Menu(
                "Application",
                vec![
                    item(Settings, "Settings…", ",", CMD),
                    Menu(
                        "Preferences",
                        vec![
                            plain(AudioSettings, "Audio…"),
                            plain(MidiSettings, "MIDI…"),
                            plain(AnalysisSettings, "Analysis & Meters…"),
                            Sep,
                            plain(DisplaySettings, "Display & Layout…"),
                            plain(ThemeSettings, "Theme & Colors…"),
                        ],
                    ),
                ],
            ),
            Menu(
                "File",
                vec![
                    item(New, "New Project", "n", CMD),
                    item(Open, "Open Project…", "o", CMD),
                    Sep,
                    item(Save, "Save Project", "s", CMD),
                    item(SaveAs, "Save Project As…", "s", CMD | SHIFT),
                    plain(Recording, "Record Visualizer…"),
                    Sep,
                    plain(Presets, "Scene Library…"),
                    Sep,
                    item(CloseComponent, "Close Selected Component", "w", CMD | SHIFT),
                ],
            ),
            Menu(
                "Edit",
                vec![
                    item(Undo, "Undo", "z", CMD),
                    item(Redo, "Redo", "z", CMD | SHIFT),
                    Sep,
                    item(Cut, "Cut", "x", CMD),
                    item(Copy, "Copy", "c", CMD),
                    item(Paste, "Paste", "v", CMD),
                    item(SelectAll, "Select All", "a", CMD),
                    Sep,
                    item(CopySettings, "Copy Component Settings", "c", CMD | SHIFT),
                    item(PasteSettings, "Paste Component Settings", "v", CMD | SHIFT),
                    Sep,
                    plain(ResetComponent, "Reset Component"),
                ],
            ),
            Menu(
                "Create",
                vec![
                    Menu(
                        "Component",
                        ModuleKind::ALL
                            .into_iter()
                            .map(|kind| plain(Add(kind), kind.label()))
                            .collect(),
                    ),
                    Menu(
                        "Signal Channel",
                        [Method::Intensity, Method::Lfo, Method::Midi]
                            .into_iter()
                            .map(|method| {
                                plain(
                                    Signal(method),
                                    match method {
                                        Method::Intensity => "Audio",
                                        Method::Lfo => "LFO",
                                        _ => "MIDI",
                                    },
                                )
                            })
                            .collect(),
                    ),
                    Sep,
                    item(NewScene, "New Empty Scene", "n", CMD | SHIFT),
                    item(DuplicateScene, "Duplicate Scene", "d", CMD | SHIFT),
                ],
            ),
            Menu(
                "Scene",
                vec![
                    // Unicode AppKit function-key equivalents for the arrow keys.
                    item(PreviousScene, "Previous Scene", "\u{f702}", CMD | ALT),
                    item(NextScene, "Next Scene", "\u{f703}", CMD | ALT),
                    Sep,
                    plain(RenameScene, "Rename Scene…"),
                    plain(DeleteScene, "Delete Scene…"),
                ],
            ),
            Menu(
                "Playback",
                vec![
                    item(Pause, "Pause Selected Component", " ", 0),
                    item(PauseAll, "Pause All Components", " ", SHIFT),
                    Sep,
                    plain(ClearHistory, "Clear Captured History"),
                    plain(ResetMeters, "Reset Peak / Clip Hold"),
                ],
            ),
            Menu(
                "View",
                vec![
                    item(Components, "Component Browser", "a", CMD | SHIFT),
                    item(Inspector, "Inspector", "n", 0),
                    item(Macros, "Macros", "b", CMD),
                    item(Signals, "Signals", "b", CMD | SHIFT),
                    item(Info, "Info View", "\u{f704}", 0),
                    Sep,
                    plain(Balance, "Balance Pane Sizes"),
                    item(PanelFullscreen, "Selected Component Fullscreen", "f", 0),
                    item(Fullscreen, "Application Fullscreen", "\u{f70e}", 0),
                    Sep,
                    item(ZoomIn, "Zoom In", "=", CMD),
                    item(ZoomOut, "Zoom Out", "-", CMD),
                    item(ActualSize, "Actual Size", "0", CMD),
                ],
            ),
            Menu(
                "Help",
                vec![
                    item(Shortcuts, "Keyboard Shortcuts…", "k", CMD),
                    plain(Info, "Info View"),
                    Sep,
                    plain(Performance, "Performance…"),
                    plain(FftDiagnostics, "FFT Diagnostics…"),
                    plain(Issues, "Issue Log…"),
                    Sep,
                    plain(About, "About Symphos…"),
                ],
            ),
        ];
        #[cfg(feature = "development")]
        if crate::development::options().is_some() {
            menus.insert(
                menus.len() - 1,
                Menu(
                    "Dev",
                    vec![
                        plain(DesignSystem, "Design System & Release Readiness…"),
                        plain(Performance, "Performance…"),
                        plain(Issues, "Issue Log…"),
                    ],
                ),
            );
        }
        menus
    })
}
fn entry_commands(entries: &[Entry]) -> impl Iterator<Item = &Command> {
    // Menus have at most two levels beneath the root. Recursive collection also
    // accommodates future submenus without assigning duplicate native tags.
    fn collect<'a>(entries: &'a [Entry], output: &mut Vec<&'a Command>) {
        for entry in entries {
            match entry {
                Entry::Command(command) => output.push(command),
                Entry::Menu(_, children) => collect(children, output),
                Entry::Separator => {}
            }
        }
    }
    let mut result = Vec::new();
    collect(entries, &mut result);
    result.into_iter()
}
pub fn commands() -> impl Iterator<Item = &'static Command> {
    entry_commands(menus())
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemState {
    pub enabled: bool,
    pub checked: bool,
}
impl ItemState {
    pub fn enabled(enabled: bool) -> Self {
        Self {
            enabled,
            checked: false,
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn symphos_menu_begin() -> bool;
    fn symphos_menu_submenu(parent: i32, title: *const std::ffi::c_char) -> i32;
    fn symphos_menu_item(
        parent: i32,
        title: *const std::ffi::c_char,
        code: i32,
        key: *const std::ffi::c_char,
        modifiers: u32,
    );
    fn symphos_menu_separator(parent: i32);
    fn symphos_menu_finish();
    fn symphos_menu_state(code: i32, enabled: bool, checked: bool);
    fn symphos_file_menu_take() -> std::ffi::c_int;
    fn symphos_project_choose(
        save: std::ffi::c_int,
        initial: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_char;
    fn symphos_project_path_free(path: *mut std::ffi::c_char);
}
pub fn install() {
    #[cfg(target_os = "macos")]
    {
        use std::{cell::Cell, ffi::CString};
        thread_local! { static INSTALLED: Cell<bool> = const { Cell::new(false) }; }
        INSTALLED.with(|installed| {
            if installed.get() {
                return;
            }
            // SAFETY: eframe creates and updates the app on AppKit's main thread.
            unsafe {
                if !symphos_menu_begin() {
                    return;
                }
                fn append(parent: i32, entries: &[Entry]) {
                    for entry in entries {
                        // SAFETY: main-thread bridge copies each string synchronously.
                        unsafe {
                            match entry {
                                Entry::Separator => symphos_menu_separator(parent),
                                Entry::Command(c) => symphos_menu_item(
                                    parent,
                                    CString::new(c.title).unwrap().as_ptr(),
                                    c.action.code(),
                                    CString::new(c.key).unwrap().as_ptr(),
                                    c.modifiers,
                                ),
                                Entry::Menu(title, children) => {
                                    let id = symphos_menu_submenu(
                                        parent,
                                        CString::new(*title).unwrap().as_ptr(),
                                    );
                                    append(id, children);
                                }
                            }
                        }
                    }
                }
                for entry in menus() {
                    if let Entry::Menu(title, children) = entry {
                        let parent = if *title == "Application" {
                            0
                        } else {
                            symphos_menu_submenu(-1, CString::new(*title).unwrap().as_ptr())
                        };
                        append(parent, children);
                    }
                }
                symphos_menu_finish();
            }
            installed.set(true);
        });
    }
}
pub fn update(mut state: impl FnMut(Action) -> ItemState) {
    for command in commands() {
        let status = state(command.action);
        #[cfg(target_os = "macos")]
        // SAFETY: same main-thread bridge; native side avoids redundant property changes.
        unsafe {
            symphos_menu_state(command.action.code(), status.enabled, status.checked);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = status;
    }
}
pub fn take() -> Option<Action> {
    #[cfg(target_os = "macos")]
    // SAFETY: this removes one command from the main-thread native queue.
    {
        Action::from_code(unsafe { symphos_file_menu_take() })
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}
#[cfg(any(test, not(target_os = "macos")))]
pub fn show(ui: &mut egui::Ui, state: impl Fn(Action) -> ItemState) -> Option<Action> {
    fn entries(
        ui: &mut egui::Ui,
        list: &[Entry],
        state: &impl Fn(Action) -> ItemState,
        action: &mut Option<Action>,
    ) {
        for entry in list {
            match entry {
                Entry::Separator => {
                    ui.separator();
                }
                Entry::Menu(title, children) => {
                    ui.menu_button(*title, |ui| entries(ui, children, state, action));
                }
                Entry::Command(command) => {
                    let s = state(command.action);
                    if ui
                        .add_enabled(
                            s.enabled,
                            egui::Button::selectable(s.checked, command.title),
                        )
                        .clicked()
                    {
                        *action = Some(command.action);
                        ui.close();
                    }
                }
            }
        }
    }
    let mut action = None;
    entries(ui, menus(), &state, &mut action);
    action
}
#[cfg(target_os = "macos")]
pub fn choose(save: bool, initial: &str) -> Option<std::path::PathBuf> {
    let path = std::ffi::CString::new(initial).ok()?;
    // SAFETY: called on the main thread; input lives across the call. The bridge
    // returns null on cancel or an owned NUL-terminated UTF-8 allocation.
    let selected = unsafe { symphos_project_choose(i32::from(save), path.as_ptr()) };
    if selected.is_null() {
        return None;
    }
    let result = unsafe { std::ffi::CStr::from_ptr(selected) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: this is the exact allocation returned by the bridge, freed once.
    unsafe {
        symphos_project_path_free(selected);
    }
    Some(result.into())
}

/// Fixed shortcut reference, generated from the actual command catalog.
pub fn shortcut_label(command: &Command) -> String {
    let key = match command.key {
        " " => "Space",
        "\u{f702}" => "Left",
        "\u{f703}" => "Right",
        "\u{f704}" => "F1",
        "\u{f70e}" => "F11",
        other => other,
    };
    let mut parts = Vec::new();
    if command.modifiers & CMD != 0 {
        parts.push(if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        });
    }
    if command.modifiers & ALT != 0 {
        parts.push(if cfg!(target_os = "macos") {
            "Option"
        } else {
            "Alt"
        });
    }
    if command.modifiers & SHIFT != 0 {
        parts.push("Shift");
    }
    let key = if key.chars().count() == 1 {
        key.to_uppercase()
    } else {
        key.to_string()
    };
    parts.push(&key);
    parts.join(" + ")
}
#[cfg(any(test, not(target_os = "macos")))]
pub fn take_shortcut(
    ctx: &egui::Context,
    mut enabled: impl FnMut(Action) -> bool,
) -> Option<Action> {
    use egui::{Event, Key, Modifiers};
    ctx.input_mut(|input| {
        if !input.focused { return None; }
        for c in commands() {
            // The normal text widget handles its platform clipboard shortcuts.
            if matches!(c.action, Action::Cut | Action::Copy | Action::Paste | Action::SelectAll | Action::Undo | Action::Redo) || !enabled(c.action) { continue; }
            let key = match c.key {
                " " => Some(Key::Space), "\u{f702}" => Some(Key::ArrowLeft), "\u{f703}" => Some(Key::ArrowRight),
                "\u{f704}" => Some(Key::F1), "\u{f70e}" => Some(Key::F11), "=" => Some(Key::Equals),
                "-" => Some(Key::Minus), "," => Some(Key::Comma), "0" => Some(Key::Num0),
                "" => None, key => Key::from_name(&key.to_uppercase()),
            };
            let Some(key) = key else { continue; };
            let mut modifiers = Modifiers::NONE;
            if c.modifiers & CMD != 0 { modifiers |= Modifiers::COMMAND; }
            if c.modifiers & SHIFT != 0 { modifiers |= Modifiers::SHIFT; }
            if c.modifiers & ALT != 0 { modifiers |= Modifiers::ALT; }
            if input.events.iter().any(|event| matches!(event, Event::Key { key: k, pressed: true, repeat: false, modifiers: m, .. }
                if *k == key && m.shift == modifiers.shift && m.alt == modifiers.alt && m.matches_logically(modifiers))) {
                input.consume_key(modifiers, key);
                return Some(c.action);
            }
        }
        None
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_commands_have_unique_ids_and_shortcuts_and_cover_every_source_and_component() {
        let mut ids = std::collections::BTreeMap::new();
        let mut keys = std::collections::BTreeSet::new();
        for command in commands() {
            let code = command.action.code();
            if let Some(previous) = ids.insert(code, command.action) {
                assert_eq!(previous, command.action);
            }
            assert_eq!(Action::from_code(code), Some(command.action));
            if !command.key.is_empty() {
                assert!(
                    keys.insert((command.key, command.modifiers)),
                    "duplicate shortcut: {}",
                    command.title
                );
            }
        }
        for kind in ModuleKind::ALL {
            assert!(commands().any(|c| c.action == Action::Add(kind)));
        }
        for method in [Method::Intensity, Method::Lfo, Method::Midi] {
            assert!(commands().any(|c| c.action == Action::Signal(method)));
        }
        for method in [
            Method::Band,
            Method::BassHits,
            Method::SpectralChange,
            Method::Balance,
        ] {
            assert!(!commands().any(|c| c.action == Action::Signal(method)));
        }
        assert!(Action::from_code(0).is_none());
        assert!(Action::from_code(9999).is_none());
    }
    #[test]
    fn menu_text_commands_act_on_a_real_text_field() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("menu-text");
        let mut text = "hello".to_string();
        for events in [
            vec![],
            vec![Action::SelectAll.text_event().unwrap()],
            vec![Action::Cut.text_event().unwrap()],
        ] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.add(egui::TextEdit::singleline(&mut text).id(id))
                        .request_focus();
                },
            );
            output.textures_delta.clear();
        }
        assert_eq!(text, "");
        let mut events = Vec::new();
        assert!(Action::SelectAll.append_text_events(&mut events));
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                assert!(!ui.input(|i| i.key_down(egui::Key::A)));
            },
        );
        output.textures_delta.clear();
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;
    #[test]
    fn desktop_fallback_renders_and_shortcuts_share_catalog_availability() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            assert!(show(ui, |_| ItemState::enabled(true)).is_none());
        });
        output.textures_delta.clear();
        for enabled in [false, true] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: egui::Key::B,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    }],
                    ..Default::default()
                },
                |ui| {
                    assert_eq!(
                        take_shortcut(ui.ctx(), |_| enabled),
                        enabled.then_some(Action::Signals)
                    )
                },
            );
            output.textures_delta.clear();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: egui::Key::B,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    }],
                    ..Default::default()
                },
                |_| {},
            );
            output.textures_delta.clear();
        }
    }
}
