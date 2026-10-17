//! Opt-in local development state. Compiled only with `--features development`.
use crate::theme::AppTheme;
pub use crate::workspace::Session as Workspace;
use eframe::egui::{self, Color32};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Options {
    pub dir: PathBuf,
    pub demo: bool,
}
pub fn options() -> Option<&'static Options> {
    static OPTIONS: OnceLock<Option<Options>> = OnceLock::new();
    OPTIONS
        .get_or_init(|| {
            let args: Vec<_> = std::env::args().collect();
            if !args.iter().any(|arg| arg == "--dev") {
                return None;
            }
            let dir = std::env::var_os("SYMPHOS_DEV_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target/dev"));
            let demo = if args.iter().any(|arg| arg == "--system-audio") {
                false
            } else if args.iter().any(|arg| arg == "--demo-audio") {
                true
            } else {
                Workspace::load(&dir).is_none_or(|state| state.demo)
            };
            Some(Options { dir, demo })
        })
        .as_ref()
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LiveTheme {
    name: String,
    dark_mode: bool,
    ui_scale: f32,
    background: String,
    panel: String,
    card: String,
    navigation: String,
    foreground: String,
    muted: String,
    accent: String,
    accent_alt: String,
    selection: String,
    warning: String,
    error: String,
}
impl Default for LiveTheme {
    fn default() -> Self {
        Self {
            name: "Symphos Dev".into(),
            dark_mode: true,
            ui_scale: 1.0,
            background: "#111210".into(),
            panel: "#181916".into(),
            card: "#1f211d".into(),
            navigation: "#1f1f1f".into(),
            foreground: "#e6e5dd".into(),
            muted: "#909187".into(),
            accent: "#36a166".into(),
            accent_alt: "#5f9182".into(),
            selection: "#2b543e".into(),
            warning: "#c2b53c".into(),
            error: "#d06852".into(),
        }
    }
}
fn color(text: &str) -> Result<Color32, String> {
    let hex = text
        .strip_prefix('#')
        .ok_or_else(|| format!("Expected #RRGGBB, got {text:?}"))?;
    if hex.len() != 6 || !hex.is_ascii() {
        return Err(format!("Expected #RRGGBB, got {text:?}"));
    }
    let value = u32::from_str_radix(hex, 16).map_err(|_| format!("Invalid color {text:?}"))?;
    Ok(Color32::from_rgb(
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ))
}
fn parse_theme(bytes: &[u8]) -> Result<(AppTheme, f32), String> {
    let theme: LiveTheme = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if !(0.6..=2.0).contains(&theme.ui_scale) {
        return Err("ui_scale must be 0.6–2.0".into());
    }
    Ok((
        AppTheme {
            name: theme.name,
            dark_mode: theme.dark_mode,
            background: color(&theme.background)?,
            panel: color(&theme.panel)?,
            card: color(&theme.card)?,
            navigation: color(&theme.navigation)?,
            foreground: color(&theme.foreground)?,
            muted: color(&theme.muted)?,
            accent: color(&theme.accent)?,
            accent_alt: color(&theme.accent_alt)?,
            selection: color(&theme.selection)?,
            warning: color(&theme.warning)?,
            error: color(&theme.error)?,
        },
        theme.ui_scale,
    ))
}

pub struct Development {
    pub options: Options,
    pub demo: bool,
    last_poll: Instant,
    last_theme: Vec<u8>,
    last_control: Vec<u8>,
}
impl Development {
    pub fn new() -> Option<Self> {
        let options = options()?.clone();
        let _ = std::fs::create_dir_all(&options.dir);
        let theme = options.dir.join("theme.json");
        if !theme.exists() {
            let _ = std::fs::write(theme, include_str!("../assets/development/theme.json"));
        }
        let last_control = std::fs::read(options.dir.join("control.json")).unwrap_or_default();
        Some(Self {
            demo: options.demo,
            options,
            last_poll: Instant::now() - Duration::from_secs(1),
            last_theme: Vec::new(),
            last_control,
        })
    }
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<(AppTheme, f32)> {
        if self.last_poll.elapsed() < Duration::from_millis(200) {
            return None;
        }
        self.last_poll = Instant::now();
        if let Ok(bytes) = std::fs::read(self.options.dir.join("control.json"))
            && bytes != self.last_control
        {
            self.last_control = bytes.clone();
            if serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .is_some_and(|value| value["action"] == "close")
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        let bytes = std::fs::read(self.options.dir.join("theme.json")).ok()?;
        if bytes == self.last_theme {
            return None;
        }
        self.last_theme = bytes.clone();
        match parse_theme(&bytes) {
            Ok(theme) => Some(theme),
            Err(error) => {
                crate::issues::record("Live theme", &error);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_theme_rejects_invalid_colors_scale_and_unknown_keys() {
        assert!(parse_theme(br##"{"accent":"#abcdef","ui_scale":1.1}"##).is_ok());
        for text in [
            r##"{"accent":"#nothex"}"##,
            r##"{"ui_scale":0}"##,
            r##"{"accnet":"#abcdef"}"##,
        ] {
            assert!(parse_theme(text.as_bytes()).is_err());
        }
    }
}
