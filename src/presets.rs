use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::modules::{ModuleKind, ModuleSettings};

pub const FILE_SUFFIX: &str = ".symphos-preset.json";
const SCHEMA_VERSION: u32 = 1;
const MAX_PRESET_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct PresetDocument {
    schema_version: u32,
    name: String,
    settings: ModuleSettings,
}

impl PresetDocument {
    fn new(name: &str, settings: ModuleSettings) -> Result<Self> {
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            name: validated_name(name)?,
            settings,
        })
    }

    fn validate(mut self) -> Result<Self> {
        if self.schema_version != SCHEMA_VERSION {
            bail!(
                "unsupported preset schema version {}; this build supports version {SCHEMA_VERSION}",
                self.schema_version
            );
        }
        self.name = validated_name(&self.name)?;
        Ok(self)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> ModuleKind {
        self.settings.kind()
    }

    pub fn settings(&self) -> &ModuleSettings {
        &self.settings
    }
}

#[derive(Clone)]
struct UserPreset {
    document: PresetDocument,
    path: PathBuf,
}

pub struct PresetLibrary {
    directory: Option<PathBuf>,
    presets: Vec<UserPreset>,
    warnings: Vec<String>,
}

impl PresetLibrary {
    pub fn load() -> Self {
        Self::from_directory(preset_directory())
    }

    fn from_directory(directory: Option<PathBuf>) -> Self {
        let mut library = Self {
            directory,
            presets: Vec::new(),
            warnings: Vec::new(),
        };
        library.reload();
        library
    }

    fn reload(&mut self) {
        self.presets.clear();
        self.warnings.clear();
        let Some(directory) = &self.directory else {
            self.warnings.push(
                "No user configuration directory is available; user presets cannot be saved."
                    .into(),
            );
            return;
        };
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        let mut paths: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(FILE_SUFFIX))
            })
            .collect();
        paths.sort();
        for path in paths {
            match read_document(&path) {
                Ok(document) => {
                    if self.presets.iter().any(|preset| {
                        preset.document.kind() == document.kind()
                            && preset.document.name() == document.name()
                    }) {
                        self.warnings.push(format!(
                            "Ignored duplicate {} preset '{}' at {}.",
                            document.kind().label(),
                            document.name(),
                            path.display()
                        ));
                    } else {
                        self.presets.push(UserPreset { document, path });
                    }
                }
                Err(error) => self
                    .warnings
                    .push(format!("Could not load {}: {error:#}", path.display())),
            }
        }
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn user_presets(&self, kind: ModuleKind) -> Vec<PresetDocument> {
        let mut presets: Vec<_> = self
            .presets
            .iter()
            .filter(|preset| preset.document.kind() == kind)
            .map(|preset| preset.document.clone())
            .collect();
        presets.sort_by_key(|preset| preset.name.to_lowercase());
        presets
    }

    pub fn contains(&self, kind: ModuleKind, name: &str) -> bool {
        self.presets.iter().any(|preset| {
            preset.document.kind() == kind
                && preset.document.name().eq_ignore_ascii_case(name.trim())
        })
    }

    pub fn save(&mut self, name: &str, settings: ModuleSettings) -> Result<String> {
        let mut document = PresetDocument::new(name, settings)?;
        if let Some(existing) = self.presets.iter().find(|preset| {
            preset.document.kind() == document.kind()
                && preset.document.name().eq_ignore_ascii_case(document.name())
        }) {
            // A case-insensitive match is a replacement, not a second file.
            document.name.clone_from(&existing.document.name);
        }
        let directory = self
            .directory
            .as_ref()
            .context("no user configuration directory is available")?;
        fs::create_dir_all(directory)
            .with_context(|| format!("create preset directory {}", directory.display()))?;
        let path = directory.join(storage_filename(document.kind(), document.name()));
        write_document(&path, &document)?;
        self.presets.retain(|preset| {
            preset.document.kind() != document.kind()
                || !preset.document.name().eq_ignore_ascii_case(document.name())
        });
        self.presets.push(UserPreset {
            document: document.clone(),
            path,
        });
        Ok(document.name)
    }

    pub fn import(&mut self, path: &Path, expected_kind: ModuleKind) -> Result<String> {
        let document = read_document(path)?;
        if document.kind() != expected_kind {
            bail!(
                "this is a {} preset; the current component is {}",
                document.kind().label(),
                expected_kind.label()
            );
        }
        self.save(document.name(), document.settings.clone())
    }

    pub fn delete(&mut self, kind: ModuleKind, name: &str) -> Result<()> {
        let index = self
            .presets
            .iter()
            .position(|preset| {
                preset.document.kind() == kind && preset.document.name().eq_ignore_ascii_case(name)
            })
            .with_context(|| format!("preset '{name}' no longer exists"))?;
        let preset = &self.presets[index];
        fs::remove_file(&preset.path)
            .with_context(|| format!("delete preset file {}", preset.path.display()))?;
        self.presets.remove(index);
        Ok(())
    }

    pub fn export(path: &Path, name: &str, settings: ModuleSettings) -> Result<()> {
        write_document(path, &PresetDocument::new(name, settings)?)
    }
}

pub fn suggested_filename(name: &str) -> String {
    let mut stem = String::new();
    let mut separator = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            stem.push(character.to_ascii_lowercase());
            separator = false;
        } else if !separator && !stem.is_empty() {
            stem.push('-');
            separator = true;
        }
    }
    while stem.ends_with('-') {
        stem.pop();
    }
    if stem.is_empty() {
        stem.push_str("symphos-preset");
    }
    format!("{stem}{FILE_SUFFIX}")
}

fn preset_directory() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|root| root.join("symphos").join("presets"))
}

fn validated_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("enter a preset name");
    }
    if name.chars().count() > 64 {
        bail!("preset names are limited to 64 characters");
    }
    if name.chars().any(char::is_control) {
        bail!("preset names cannot contain control characters");
    }
    Ok(name.to_owned())
}

fn storage_filename(kind: ModuleKind, name: &str) -> String {
    let encoded = name
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{}-{encoded}{FILE_SUFFIX}", kind.slug())
}

fn read_document(path: &Path) -> Result<PresetDocument> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("read preset metadata from {}", path.display()))?;
    if metadata.len() > MAX_PRESET_BYTES {
        bail!("preset is larger than the 1 MiB limit");
    }
    let contents =
        fs::read_to_string(path).with_context(|| format!("read preset from {}", path.display()))?;
    serde_json::from_str::<PresetDocument>(&contents)
        .with_context(|| format!("parse preset JSON from {}", path.display()))?
        .validate()
}

fn write_document(path: &Path, document: &PresetDocument) -> Result<()> {
    let mut contents = serde_json::to_string_pretty(document).context("serialize preset")?;
    contents.push('\n');
    fs::write(path, contents).with_context(|| format!("write preset to {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::modules::ModulePane;

    fn temp_directory() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("symphos-presets-{}-{unique}", std::process::id()))
    }

    #[test]
    fn every_factory_preset_round_trips_through_the_versioned_document() {
        for kind in ModuleKind::ALL {
            let pane = ModulePane::new(kind);
            assert!((3..=4).contains(&pane.factory_preset_names().len()));
            for (index, name) in pane.factory_preset_names().iter().enumerate() {
                let document =
                    PresetDocument::new(name, pane.factory_preset(index).unwrap()).unwrap();
                let json = serde_json::to_string_pretty(&document).unwrap();
                assert!(!json.contains("\"reference\""));
                assert!(!json.contains("\"pinned\""));
                assert!(!json.contains("\"loop_cursor\""));
                let restored: PresetDocument = serde_json::from_str(&json).unwrap();
                assert_eq!(restored.validate().unwrap(), document);
                assert_eq!(document.kind(), kind);
            }
        }
    }

    #[test]
    fn user_presets_save_replace_reload_and_delete_by_module() {
        let directory = temp_directory();
        let mut library = PresetLibrary::from_directory(Some(directory.clone()));
        let spectrum = ModulePane::new(ModuleKind::Spectrum)
            .factory_preset(1)
            .unwrap();
        let waveform = ModulePane::new(ModuleKind::Waveform)
            .factory_preset(2)
            .unwrap();
        library.save("Studio", spectrum.clone()).unwrap();
        library.save("Motion", waveform).unwrap();
        library.save(" studio ", spectrum.clone()).unwrap();
        assert_eq!(library.user_presets(ModuleKind::Spectrum).len(), 1);

        let mut restored = PresetLibrary::from_directory(Some(directory.clone()));
        assert_eq!(
            restored.user_presets(ModuleKind::Spectrum)[0].settings(),
            &spectrum
        );
        assert_eq!(restored.user_presets(ModuleKind::Waveform).len(), 1);
        restored.delete(ModuleKind::Spectrum, "STUDIO").unwrap();
        assert!(restored.user_presets(ModuleKind::Spectrum).is_empty());
        assert_eq!(restored.user_presets(ModuleKind::Waveform).len(), 1);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn import_rejects_the_wrong_module_and_future_schema() {
        let directory = temp_directory();
        fs::create_dir_all(&directory).unwrap();
        let source = directory.join("source.json");
        PresetLibrary::export(
            &source,
            "Wave",
            ModulePane::new(ModuleKind::Waveform).copy_settings(),
        )
        .unwrap();
        let mut library = PresetLibrary::from_directory(Some(directory.clone()));
        let error = library.import(&source, ModuleKind::Spectrum).unwrap_err();
        assert!(error.to_string().contains("Waveform preset"));

        let contents = fs::read_to_string(&source)
            .unwrap()
            .replace("\"schema_version\": 1", "\"schema_version\": 999");
        fs::write(&source, contents).unwrap();
        assert!(
            read_document(&source)
                .unwrap_err()
                .to_string()
                .contains("unsupported preset schema")
        );

        fs::remove_dir_all(directory).unwrap();
    }
}
