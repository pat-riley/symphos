use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use eframe::egui::Color32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppTheme {
    pub name: String,
    pub dark_mode: bool,
    pub background: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub foreground: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_alt: Color32,
    pub selection: Color32,
    pub warning: Color32,
    pub error: Color32,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self {
            name: "Symphos Dark".into(),
            dark_mode: true,
            background: Color32::from_rgb(17, 18, 16),
            panel: Color32::from_rgb(24, 25, 22),
            card: Color32::from_rgb(31, 33, 29),
            foreground: Color32::from_rgb(230, 229, 221),
            muted: Color32::from_rgb(144, 145, 135),
            accent: Color32::from_rgb(54, 161, 102),
            accent_alt: Color32::from_rgb(95, 145, 130),
            selection: Color32::from_rgb(43, 84, 62),
            warning: Color32::from_rgb(194, 181, 60),
            error: Color32::from_rgb(208, 104, 82),
        }
    }
}

impl AppTheme {
    pub fn load_omarchy() -> Self {
        Self::load_current_omarchy()
            .or_else(load_legacy_omarchy)
            .unwrap_or_default()
    }

    /// Load Omarchy's staged active palette. This is intentionally filesystem-only so it is
    /// cheap enough for the UI to poll while running.
    pub fn load_current_omarchy() -> Option<Self> {
        let current = current_state_path()?;
        let name = fs::read_to_string(current.join("theme.name"))
            .ok()
            .map(|name| display_theme_name(name.trim()))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Omarchy".into());
        let contents = fs::read_to_string(current.join("theme/colors.toml")).ok()?;
        Some(Self::from_colors(name, &contents))
    }

    fn from_colors(name: String, contents: &str) -> Self {
        let mut theme = Self::default();
        let declared_mode = text_value(contents, "mode");
        theme.name = name;
        theme.background = value(contents, "background").unwrap_or(theme.background);
        theme.foreground = value(contents, "foreground").unwrap_or(theme.foreground);
        theme.dark_mode = declared_mode.as_deref().map_or_else(
            || relative_luminance(theme.background) < 0.5,
            |mode| mode != "light",
        );
        theme.accent = value(contents, "accent")
            .or_else(|| value(contents, "blue"))
            .or_else(|| value(contents, "color4"))
            .unwrap_or(theme.accent);
        theme.accent_alt = value(contents, "cyan")
            .or_else(|| value(contents, "active_border_color"))
            .or_else(|| value(contents, "color6"))
            .unwrap_or(theme.accent_alt);
        theme.selection = value(contents, "selection")
            .unwrap_or_else(|| mix(theme.background, theme.accent, 0.32));
        theme.warning = value(contents, "yellow")
            .or_else(|| value(contents, "orange"))
            .or_else(|| value(contents, "color11"))
            .or_else(|| value(contents, "color3"))
            .unwrap_or(theme.warning);
        theme.error = value(contents, "red")
            .or_else(|| value(contents, "color9"))
            .or_else(|| value(contents, "color1"))
            .unwrap_or(theme.error);
        theme.muted = value(contents, "muted")
            .or_else(|| value(contents, "color8"))
            .unwrap_or_else(|| mix(theme.foreground, theme.background, 0.5));
        theme.panel = mix(theme.background, theme.foreground, 0.045);
        theme.card = mix(theme.background, theme.foreground, 0.085);
        theme
    }
}

fn load_legacy_omarchy() -> Option<AppTheme> {
    let name = current_theme_name()?;
    let path = theme_path(&name)?;
    let contents = fs::read_to_string(path).ok()?;
    Some(AppTheme::from_colors(name, &contents))
}

fn current_state_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join(".local/state/omarchy/current"))
}

fn current_theme_name() -> Option<String> {
    let output = Command::new("omarchy")
        .args(["theme", "current"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!name.is_empty()).then_some(name)
}

fn theme_path(name: &str) -> Option<PathBuf> {
    let slug = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")))?;
    let user_path = config
        .join("omarchy/themes")
        .join(&slug)
        .join("colors.toml");
    if user_path.is_file() {
        return Some(user_path);
    }
    let stock_path = Path::new("/usr/share/omarchy/themes")
        .join(slug)
        .join("colors.toml");
    stock_path.is_file().then_some(stock_path)
}

fn value(contents: &str, key: &str) -> Option<Color32> {
    text_value(contents, key).and_then(|value| parse_hex(&value))
}

fn text_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        let (candidate, raw) = line.split_once('=')?;
        if candidate.trim() != key {
            return None;
        }
        let raw = raw.trim();
        let quoted = raw
            .strip_prefix('"')
            .and_then(|value| value.split_once('"').map(|(value, _)| value))
            .or_else(|| {
                raw.strip_prefix('\'')
                    .and_then(|value| value.split_once('\'').map(|(value, _)| value))
            })
            .unwrap_or(raw);
        Some(quoted.to_owned())
    })
}

fn display_theme_name(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_hex(value: &str) -> Option<Color32> {
    let value = value.strip_prefix('#')?;
    if value.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&value[0..2], 16).ok()?;
    let green = u8::from_str_radix(&value[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&value[4..6], 16).ok()?;
    Some(Color32::from_rgb(red, green, blue))
}

fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let mix_channel = |left: u8, right: u8| {
        (left as f32 + (right as f32 - left as f32) * amount.clamp(0.0, 1.0)) as u8
    };
    Color32::from_rgb(
        mix_channel(a.r(), b.r()),
        mix_channel(a.g(), b.g()),
        mix_channel(a.b(), b.b()),
    )
}

fn relative_luminance(color: Color32) -> f32 {
    (0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32) / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_hex_with_comments() {
        let colors = r##"
            # theme comment
            background = "#22221b"
            accent = '#36a166' # trailing comment
        "##;
        assert_eq!(
            value(colors, "background"),
            Some(Color32::from_rgb(34, 34, 27))
        );
        assert_eq!(
            value(colors, "accent"),
            Some(Color32::from_rgb(54, 161, 102))
        );
    }

    #[test]
    fn builds_theme_from_current_omarchy_palette() {
        let colors = r##"
            mode = "light"
            background = "#f5f0e8"
            foreground = "#292522"
            accent = "#8c4f34"
            selection = "#dccabe"
            muted = "#796f67"
            red = "#b23b3b"
            yellow = "#9a6b20"
            cyan = "#397878"
        "##;
        let theme = AppTheme::from_colors("Test Theme".into(), colors);
        assert!(!theme.dark_mode);
        assert_eq!(theme.selection, Color32::from_rgb(220, 202, 190));
        assert_eq!(theme.accent_alt, Color32::from_rgb(57, 120, 120));
        assert_eq!(theme.error, Color32::from_rgb(178, 59, 59));
    }

    #[test]
    fn formats_theme_slug_for_display() {
        assert_eq!(display_theme_name("tokyo-night"), "Tokyo Night");
    }
}
