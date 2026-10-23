//! Display and parse the picker's opaque sRGB color representations.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Format {
    #[default]
    Hex,
    Rgb,
    Css,
    Hsl,
    Hsb,
}

impl Format {
    pub const ALL: [Self; 5] = [Self::Hex, Self::Rgb, Self::Css, Self::Hsl, Self::Hsb];
    pub fn label(self) -> &'static str {
        match self {
            Self::Hex => "Hex",
            Self::Rgb => "RGB",
            Self::Css => "CSS",
            Self::Hsl => "HSL",
            Self::Hsb => "HSB",
        }
    }
    pub fn text(self, [r, g, b]: [u8; 3]) -> String {
        if self == Self::Css {
            format!("rgb({r} {g} {b})")
        } else {
            format!("{r:02X}{g:02X}{b:02X}")
        }
    }
    pub fn parse(self, value: &str) -> Option<[u8; 3]> {
        if self == Self::Hex {
            hex(value)
        } else {
            css(value)
        }
    }
}

pub(super) fn hsv(rgb: [u8; 3]) -> [f32; 3] {
    // HSV/HSB editing uses encoded sRGB components, as in common design tools.
    let (h, s, v) = egui::ecolor::hsv_from_rgb(rgb.map(|x| x as f32 / 255.0));
    [h, s, v]
}

pub(super) fn rgb([h, s, v]: [f32; 3]) -> [u8; 3] {
    egui::ecolor::rgb_from_hsv((h, s, v)).map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
}

pub(super) fn hsl([h, s, v]: [f32; 3]) -> [f32; 3] {
    let l = v * (1.0 - s / 2.0);
    let saturation = if l > 0.0 && l < 1.0 {
        (v - l) / l.min(1.0 - l)
    } else {
        s
    };
    [h, saturation, l]
}

pub(super) fn from_hsl([h, s, l]: [f32; 3]) -> [f32; 3] {
    let v = l + s * l.min(1.0 - l);
    [
        h.rem_euclid(1.0),
        if v > 0.0 { 2.0 * (1.0 - l / v) } else { s },
        v,
    ]
}

fn hex(value: &str) -> Option<[u8; 3]> {
    let value = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if !value.is_ascii() || !value.bytes().all(|x| x.is_ascii_hexdigit()) {
        return None;
    }
    match value.len() {
        3 => Some(std::array::from_fn(|i| {
            u8::from_str_radix(&value[i..i + 1], 16).unwrap() * 17
        })),
        4 if value.ends_with(['f', 'F']) => hex(&value[..3]),
        6 => Some(std::array::from_fn(|i| {
            u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).unwrap()
        })),
        8 if value[6..].eq_ignore_ascii_case("ff") => hex(&value[..6]),
        _ => None,
    }
}

fn number(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().filter(|x| x.is_finite())
}

fn percentage(value: &str) -> Option<f32> {
    Some((number(value.strip_suffix('%')?)? / 100.0).clamp(0.0, 1.0))
}

fn hue(value: &str) -> Option<f32> {
    for (suffix, turns) in [
        ("turn", 1.0),
        ("grad", 1.0 / 400.0),
        ("rad", 1.0 / std::f32::consts::TAU),
        ("deg", 1.0 / 360.0),
        ("", 1.0 / 360.0),
    ] {
        if let Some(value) = value.strip_suffix(suffix) {
            return Some((number(value)? * turns).rem_euclid(1.0));
        }
    }
    None
}

fn css(value: &str) -> Option<[u8; 3]> {
    let value = value.trim().to_ascii_lowercase();
    if value.starts_with('#') {
        return hex(&value);
    }
    let named = match value.as_str() {
        "black" => Some([0, 0, 0]),
        "white" => Some([255, 255, 255]),
        "red" => Some([255, 0, 0]),
        "lime" => Some([0, 255, 0]),
        "blue" => Some([0, 0, 255]),
        "yellow" => Some([255, 255, 0]),
        "cyan" | "aqua" => Some([0, 255, 255]),
        "magenta" | "fuchsia" => Some([255, 0, 255]),
        "gray" | "grey" => Some([128, 128, 128]),
        "silver" => Some([192, 192, 192]),
        "maroon" => Some([128, 0, 0]),
        "olive" => Some([128, 128, 0]),
        "green" => Some([0, 128, 0]),
        "purple" => Some([128, 0, 128]),
        "teal" => Some([0, 128, 128]),
        "navy" => Some([0, 0, 128]),
        "orange" => Some([255, 165, 0]),
        "rebeccapurple" => Some([102, 51, 153]),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let (function, rest) = value.split_once('(')?;
    let components = rest.strip_suffix(')')?.replace([',', '/'], " ");
    let parts: Vec<_> = components.split_whitespace().collect();
    if !(3..=4).contains(&parts.len()) {
        return None;
    }
    // Current consumers store opaque colors; never silently discard transparency.
    if parts.len() == 4 {
        let alpha = if parts[3].ends_with('%') {
            number(parts[3].strip_suffix('%')?)? / 100.0
        } else {
            number(parts[3])?
        };
        if alpha != 1.0 {
            return None;
        }
    }
    match function {
        "rgb" | "rgba" => {
            let component = |value: &str| {
                if value.ends_with('%') {
                    percentage(value)
                } else {
                    Some((number(value)? / 255.0).clamp(0.0, 1.0))
                }
            };
            Some(
                [
                    component(parts[0])?,
                    component(parts[1])?,
                    component(parts[2])?,
                ]
                .map(|x| (x * 255.0).round() as u8),
            )
        }
        "hsl" | "hsla" => Some(rgb(from_hsl([
            hue(parts[0])?,
            percentage(parts[1])?,
            percentage(parts[2])?,
        ]))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_text_inputs_parse_in_srgb_and_reject_invalid_or_transparent_values() {
        for value in [
            "#f80",
            "#FF8800",
            "rgb(255, 136, 0)",
            "rgb(255 136 0 / 100%)",
            "hsl(32deg 100% 50%)",
            "hsla(32, 100%, 50%, 1)",
        ] {
            assert_eq!(Format::Css.parse(value), Some([255, 136, 0]), "{value}");
        }
        assert_eq!(Format::Hex.parse("Ff8800"), Some([255, 136, 0]));
        assert_eq!(Format::Css.parse("rgb(100% 0% 0%)"), Some([255, 0, 0]));
        assert_eq!(
            Format::Css.parse("hsl(.5turn 100% 50%)"),
            Some([0, 255, 255])
        );
        assert_eq!(Format::Css.parse("rebeccapurple"), Some([102, 51, 153]));
        for value in [
            "#12",
            "#ggg",
            "#FF880080",
            "rgba(255,0,0,.5)",
            "rgb(NaN 0 0)",
            "hsl(0 50 50)",
            "var(--color)",
            "hsl(inf 100% 50%)",
        ] {
            assert_eq!(Format::Css.parse(value), None, "{value}");
        }
    }
    #[test]
    fn switching_formats_preserves_primary_secondary_gray_and_arbitrary_colors() {
        for r in [0, 13, 128, 241, 255] {
            for g in [0, 32, 128, 200, 255] {
                for b in [0, 66, 128, 220, 255] {
                    let color = [r, g, b];
                    assert_eq!(rgb(hsv(color)), color);
                    assert_eq!(rgb(from_hsl(hsl(hsv(color)))), color);
                    for format in [Format::Hex, Format::Css] {
                        assert_eq!(format.parse(&format.text(color)), Some(color));
                    }
                }
            }
        }
    }
}
