use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
const MAX_PRESET: usize = 2 * 1024 * 1024;
const MAX_TEXTURES: usize = 8 * 1024 * 1024;
// Leave room for other components and scenes within the project format's limit.
pub const MAX_PRESETS: usize = 2048;
pub const MAX_LIBRARY_BYTES: usize = 24 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub favorite: bool,
    /// Hex keeps embedded binary assets compact and independent of machine paths.
    #[serde(default)]
    pub textures: BTreeMap<String, String>,
}
impl Preset {
    pub fn storage_bytes(&self) -> usize {
        self.source.len()
            + self.name.len()
            + self
                .textures
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() || self.name.len() > 240 {
            return Err("Preset name must contain 1–240 bytes".into());
        }
        if self.source.len() > MAX_PRESET
            || self.source.contains('\0')
            || !self
                .source
                .lines()
                .any(|l| l.trim().eq_ignore_ascii_case("[preset00]"))
        {
            return Err("Expected a .milk file with a [preset00] section (maximum 2 MB)".into());
        }
        let mut total = 0;
        for (name, data) in &self.textures {
            if !safe_name(name)
                || data.len() % 2 != 0
                || !data.bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err("Invalid embedded texture".into());
            }
            total += data.len() / 2;
        }
        if total > MAX_TEXTURES {
            return Err("Preset textures exceed 8 MB".into());
        }
        Ok(())
    }
    pub fn import(path: &Path) -> Result<Self, String> {
        if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_PRESET as u64 {
            return Err("Preset exceeds 2 MB".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        // MilkDrop-era files often use Windows-1252. Equations/shaders are ASCII;
        // retain extended comment/name bytes as Unicode instead of rejecting them.
        let source = String::from_utf8(bytes.clone()).unwrap_or_else(|_| {
            const CP1252: [char; 32] = [
                '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}',
                'Ž', '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›',
                'œ', '\u{9d}', 'ž', 'Ÿ',
            ];
            bytes
                .into_iter()
                .map(|b| {
                    if (0x80..0xa0).contains(&b) {
                        CP1252[(b - 0x80) as usize]
                    } else {
                        char::from(b)
                    }
                })
                .collect()
        });
        let source = source.trim_start_matches('\u{feff}').to_owned();
        let mut p = Self {
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            source,
            favorite: false,
            textures: BTreeMap::new(),
        };
        p.validate()?;
        // Include neighboring texture files and the conventional textures folder.
        let parent = path.parent().unwrap_or(Path::new("."));
        p.import_textures(parent)?;
        if parent.join("textures").is_dir() {
            p.import_textures(&parent.join("textures"))?;
        }
        p.validate()?;
        Ok(p)
    }
    pub fn import_textures(&mut self, directory: &Path) -> Result<(), String> {
        let mut textures = self.textures.clone();
        let mut total: usize = textures.values().map(|s| s.len() / 2).sum();
        let mut files = std::fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect::<Vec<_>>();
        files.sort();
        for path in files {
            if !path.is_file()
                || !matches!(
                    path.extension()
                        .and_then(|x| x.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .as_str(),
                    "png" | "jpg" | "jpeg" | "bmp" | "tga" | "dds"
                )
            {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if !safe_name(&name) {
                continue;
            }
            let len = std::fs::metadata(&path).map_err(|e| e.to_string())?.len() as usize;
            total = total.saturating_sub(textures.get(&name).map_or(0, |s| s.len() / 2));
            if total.saturating_add(len) > MAX_TEXTURES {
                return Err("Texture folder exceeds 8 MB. Use a folder containing only this preset’s textures.".into());
            }
            total += len;
            textures.insert(name, hex(&std::fs::read(path).map_err(|e| e.to_string())?));
        }
        self.textures = textures;
        Ok(())
    }
    pub fn export(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        // Keep texture assets beside the preset; existing textures are never overwritten.
        let parent = path.parent().unwrap_or(Path::new("."));
        for (name, content) in &self.textures {
            let dest = parent.join(name);
            let bytes = unhex(content)?;
            if dest.exists() {
                if std::fs::read(&dest).map_err(|e| e.to_string())? != bytes {
                    return Err(format!(
                        "Texture {name} already exists with different contents. Export to a new folder."
                    ));
                }
            } else {
                std::fs::write(dest, bytes).map_err(|e| e.to_string())?;
            }
        }
        std::fs::write(path, &self.source).map_err(|e| e.to_string())
    }
    pub fn materialize(&self, directory: &Path) -> Result<(), String> {
        self.validate()?;
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        for (name, data) in &self.textures {
            std::fs::write(directory.join(name), unhex(data)?).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    /// Edit only literal saved values; never rewrite expression/shader lines.
    pub fn scalars(&self) -> Vec<(String, f64)> {
        self.source
            .lines()
            .filter_map(|line| {
                let (key, value) = line.split_once('=')?;
                let key = key.trim();
                if key.is_empty()
                    || key.contains("per_frame")
                    || key.contains("per_pixel")
                    || key.contains("per_point")
                    || key.starts_with("warp_")
                    || key.starts_with("comp_")
                {
                    return None;
                }
                let value = value.trim().parse::<f64>().ok()?;
                value.is_finite().then(|| (key.to_owned(), value))
            })
            .collect()
    }
    pub fn set_scalar(&mut self, key: &str, value: f64) {
        if !value.is_finite() {
            return;
        }
        let newline = if self.source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        self.source = self
            .source
            .lines()
            .map(|line| {
                if line
                    .split_once('=')
                    .is_some_and(|(k, v)| k.trim() == key && v.trim().parse::<f64>().is_ok())
                {
                    format!("{key}={value}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(newline)
            + newline;
    }
}
pub fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 240
        && !name.contains(['/', '\\', ':', '\0'])
        && name != "."
        && name != ".."
}
fn hex(bytes: &[u8]) -> String {
    const H: &[u8] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 15) as usize] as char);
    }
    s
}
fn unhex(s: &str) -> Result<Vec<u8>, String> {
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| "Invalid texture encoding".into())
        })
        .collect()
}
pub fn builtins() -> Vec<Preset> {
    [
        (
            "Prismatic feedback",
            include_str!("../../assets/milkdrop/prismatic.milk"),
        ),
        (
            "Luminous ribbons",
            include_str!("../../assets/milkdrop/ribbons.milk"),
        ),
        (
            "Shader kaleidoscope",
            include_str!("../../assets/milkdrop/kaleidoscope.milk"),
        ),
    ]
    .into_iter()
    .map(|(name, source)| Preset {
        name: name.into(),
        source: source.into(),
        favorite: false,
        textures: BTreeMap::new(),
    })
    .collect()
}

pub fn choose(save: bool, initial: &str, folder: bool) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn symphos_milk_choose(
                save: i32,
                initial: *const std::ffi::c_char,
                folder: i32,
            ) -> *mut std::ffi::c_char;
            fn symphos_project_path_free(p: *mut std::ffi::c_char);
        }
        let initial = std::ffi::CString::new(initial).ok()?;
        // SAFETY: inspector calls on the AppKit main thread. Returned owned allocation is freed once.
        unsafe {
            let p = symphos_milk_choose(save as i32, initial.as_ptr(), folder as i32);
            if p.is_null() {
                return None;
            }
            let s = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
            symphos_project_path_free(p);
            Some(s.into())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut cmd = std::process::Command::new("zenity");
        cmd.arg("--file-selection");
        if folder {
            cmd.arg("--directory");
        } else {
            cmd.arg("--file-filter=MilkDrop presets | *.milk");
        }
        if save {
            cmd.args(["--save", "--confirm-overwrite"]);
        }
        cmd.arg(format!("--filename={initial}"));
        let out = cmd.output().ok()?;
        out.status
            .success()
            .then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
    }
}

/// Recursively import a bounded library, ignoring symlinks and reporting rejected files.
pub fn import_folder(
    directory: &Path,
    capacity: usize,
    mut remaining_bytes: usize,
) -> Result<(Vec<Preset>, Vec<String>), String> {
    fn collect(
        dir: &Path,
        depth: usize,
        files: &mut Vec<PathBuf>,
        limit: usize,
    ) -> Result<(), String> {
        if depth > 8 || files.len() >= limit {
            return Ok(());
        }
        let mut entries = std::fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if files.len() >= limit {
                break;
            }
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() {
                collect(&entry.path(), depth + 1, files, limit)?;
            } else if kind.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("milk"))
            {
                files.push(entry.path());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    collect(directory, 0, &mut files, capacity + 1)?;
    let mut errors = Vec::new();
    if files.len() > capacity {
        errors.push(format!(
            "Imported the first {capacity} presets; this component holds up to {MAX_PRESETS}."
        ));
        files.truncate(capacity);
    }
    let mut presets = Vec::new();
    for path in files {
        match Preset::import(&path) {
            Ok(p) if p.storage_bytes() <= remaining_bytes => {
                remaining_bytes -= p.storage_bytes();
                presets.push(p);
            }
            Ok(_) => {
                errors.push("Stopped importing: this component's embedded preset library has reached 24 MB. Remove presets or use fewer texture assets before importing more.".into());
                break;
            }
            Err(e) => errors.push(format!(
                "{}: {e}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )),
        }
    }
    Ok((presets, errors))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folder_import_respects_embedded_asset_budget() {
        let dir =
            std::env::temp_dir().join(format!("symphos-milkdrop-budget-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = builtins().remove(0);
        p.export(&dir.join("one.milk")).unwrap();
        p.export(&dir.join("two.milk")).unwrap();
        let (presets, errors) = import_folder(&dir, 256, p.storage_bytes() + 128).unwrap();
        assert_eq!(presets.len(), 1);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("24 MB"));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn scalar_edits_keep_shader_and_equation_code() {
        let mut p = builtins().remove(2);
        let code: Vec<_> = p
            .source
            .lines()
            .filter(|s| s.starts_with("per_") || s.starts_with("warp_") || s.starts_with("comp_"))
            .map(str::to_owned)
            .collect();
        p.set_scalar("fDecay", 0.83);
        assert!(p.scalars().contains(&("fDecay".into(), 0.83)));
        let after: Vec<_> = p
            .source
            .lines()
            .filter(|s| s.starts_with("per_") || s.starts_with("warp_") || s.starts_with("comp_"))
            .map(str::to_owned)
            .collect();
        assert_eq!(after, code);
    }
    #[test]
    fn embedded_texture_validation_rejects_paths_and_invalid_data() {
        let mut p = builtins().remove(0);
        for name in ["../x.png", "/tmp/x.png", "a\\b.png", "..", "C:x.png"] {
            p.textures.clear();
            p.textures.insert(name.into(), "0102".into());
            assert!(p.validate().is_err());
        }
        p.textures.clear();
        p.textures.insert("valid.png".into(), "123".into());
        assert!(p.validate().is_err());
        p.textures.insert("valid.png".into(), "zz00".into());
        assert!(p.validate().is_err());
    }
    #[test]
    fn portable_preset_roundtrip_includes_assets() {
        let mut p = builtins().remove(2);
        p.textures.insert("custom.png".into(), hex(&[0, 127, 255]));
        let json = serde_json::to_vec(&p).unwrap();
        let restored: Preset = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored, p);
        assert_eq!(
            unhex(&restored.textures["custom.png"]).unwrap(),
            [0, 127, 255]
        );
    }
    #[test]
    fn malformed_files_are_rejected_without_modifying_state() {
        let mut p = builtins().remove(0);
        p.source = "not a preset".into();
        assert!(p.validate().is_err());
        p.source = "[preset00]\nwarp_1=\0".into();
        assert!(p.validate().is_err());
    }
    #[test]
    fn import_export_preserves_code_and_avoids_texture_overwrite() {
        let dir =
            std::env::temp_dir().join(format!("symphos-milkdrop-roundtrip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut p = builtins().remove(2);
        p.textures.insert("test.png".into(), "0102".into());
        let path = dir.join("Shader kaleidoscope.milk");
        p.export(&path).unwrap();
        assert_eq!(Preset::import(&path).unwrap(), p);
        std::fs::write(dir.join("test.png"), [3, 4]).unwrap();
        assert!(p.export(&path).is_err());
        assert_eq!(std::fs::read(dir.join("test.png")).unwrap(), [3, 4]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
