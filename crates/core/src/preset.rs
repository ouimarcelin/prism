//! Presets: effects and layer settings saved apart from any project, to
//! use again in any project.
//!
//! One JSON file per preset, named after it, with a format version like
//! project files.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project::write_atomic;
use crate::{Animation, BlendMode, CrossfaderSide, Effect, Layer, ProjectError, Unit};

/// Version of the preset file format written by this build.
pub const PRESET_FORMAT_VERSION: u32 = 1;

/// File extension of presets.
pub const PRESET_EXTENSION: &str = "json";

/// What a preset holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PresetContent {
    /// A chain of effects.
    Effects {
        /// The effects, in order.
        effects: Vec<Effect>,
    },
    /// The mixing settings and effects of a layer.
    Layer(LayerPreset),
}

/// The settings of a layer a preset keeps: not its name, clips or state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerPreset {
    /// Opacity.
    pub opacity: Unit,
    /// Blend mode.
    #[serde(default)]
    pub blend_mode: BlendMode,
    /// Crossfader side.
    #[serde(default)]
    pub crossfader_side: Option<CrossfaderSide>,
    /// Fade between clips, in milliseconds.
    #[serde(default)]
    pub transition_ms: u32,
    /// Effects of the layer.
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// Animation of the opacity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity_animation: Option<Animation>,
}

impl LayerPreset {
    /// The settings of `layer`.
    pub fn of(layer: &Layer) -> Self {
        Self {
            opacity: layer.opacity,
            blend_mode: layer.blend_mode,
            crossfader_side: layer.crossfader_side,
            transition_ms: layer.transition_ms,
            effects: layer.effects.clone(),
            opacity_animation: layer.opacity_animation,
        }
    }

    /// Gives `layer` these settings; its name, clips and state stay.
    pub fn apply(&self, layer: &mut Layer) {
        layer.opacity = self.opacity;
        layer.blend_mode = self.blend_mode;
        layer.crossfader_side = self.crossfader_side;
        layer.transition_ms = self.transition_ms;
        layer.effects = self.effects.clone();
        layer.opacity_animation = self.opacity_animation;
    }
}

/// A named preset.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// Name, also the name of its file.
    pub name: String,
    /// What it holds.
    pub content: PresetContent,
}

#[derive(Serialize)]
struct PresetOut<'a> {
    format_version: u32,
    #[serde(flatten)]
    content: &'a PresetContent,
}

#[derive(Deserialize)]
struct PresetIn {
    format_version: Option<u64>,
    #[serde(flatten)]
    content: PresetContent,
}

/// A file name for a preset named `name`: characters that file systems
/// refuse replaced, never empty.
pub fn preset_file_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim_start_matches('.');
    let stem = if cleaned.is_empty() {
        "Préréglage"
    } else {
        cleaned
    };
    format!("{stem}.{PRESET_EXTENSION}")
}

impl Preset {
    /// Writes the preset in `folder`, created if needed, replacing a preset
    /// of the same name. Returns the file written.
    pub fn save(&self, folder: &Path) -> Result<PathBuf, ProjectError> {
        let io = |source| ProjectError::Io {
            path: folder.to_path_buf(),
            source,
        };
        fs::create_dir_all(folder).map_err(io)?;
        let path = folder.join(preset_file_name(&self.name));
        let json = serde_json::to_string_pretty(&PresetOut {
            format_version: PRESET_FORMAT_VERSION,
            content: &self.content,
        })?;
        write_atomic(&path, json.as_bytes()).map_err(|source| ProjectError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }

    /// Reads the preset of the file at `path`, named after the file.
    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        let json = fs::read_to_string(path).map_err(|source| ProjectError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let read: PresetIn = serde_json::from_str(&json)?;
        match read.format_version {
            None => return Err(ProjectError::MissingVersion),
            Some(found) if found > u64::from(PRESET_FORMAT_VERSION) => {
                return Err(ProjectError::UnsupportedVersion {
                    found,
                    supported: PRESET_FORMAT_VERSION,
                });
            }
            Some(_) => {}
        }
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self {
            name,
            content: read.content,
        })
    }

    /// Every preset readable in `folder`, by name. Files that are not
    /// presets are skipped; a missing folder holds none.
    pub fn list(folder: &Path) -> Vec<Self> {
        let Ok(entries) = fs::read_dir(folder) else {
            return Vec::new();
        };
        let mut presets: Vec<Self> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == PRESET_EXTENSION))
            .filter_map(|path| Self::load(&path).ok())
            .collect();
        presets.sort_by_key(|preset| sort_key(&preset.name));
        presets
    }
}

/// `name` for sorting: case and the accents of French letters ignored.
fn sort_key(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Composition;

    #[test]
    fn presets_round_trip_through_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut blur = Effect::new("blur");
        blur.set_param("radius", 12.0);
        let effects = Preset {
            name: "Flou doux".into(),
            content: PresetContent::Effects {
                effects: vec![blur, Effect::new("mirror")],
            },
        };
        let path = effects.save(dir.path()).unwrap();
        assert_eq!(path.file_name().unwrap(), "Flou doux.json");
        let json = fs::read_to_string(&path).unwrap();
        assert!(json.contains(r#""kind": "effects""#), "{json}");
        assert!(json.contains(r#""format_version": 1"#), "{json}");
        assert_eq!(Preset::load(&path).unwrap(), effects);

        let mut comp = Composition::default();
        let id = comp.layers()[0].id();
        let layer = comp.layer_mut(id).unwrap();
        layer.opacity = Unit::new(0.5);
        layer.blend_mode = BlendMode::Screen;
        layer.transition_ms = 300;
        layer.effects.push(Effect::new("invert"));
        let layer_preset = Preset {
            name: "Écran: moitié".into(),
            content: PresetContent::Layer(LayerPreset::of(layer)),
        };
        let path = layer_preset.save(dir.path()).unwrap();
        assert_eq!(path.file_name().unwrap(), "Écran- moitié.json");

        // Listed by name; other files skipped.
        fs::write(dir.path().join("notes.json"), "{}").unwrap();
        fs::write(dir.path().join("readme.txt"), "").unwrap();
        let names: Vec<String> = Preset::list(dir.path())
            .into_iter()
            .map(|preset| preset.name)
            .collect();
        assert_eq!(names, ["Écran- moitié", "Flou doux"]);
        assert!(Preset::list(&dir.path().join("none")).is_empty());

        // Applied to another layer: settings, not the name.
        let other = comp.layers()[1].id();
        let PresetContent::Layer(settings) = &Preset::load(&path).unwrap().content else {
            panic!("a layer preset");
        };
        settings.apply(comp.layer_mut(other).unwrap());
        let other = comp.layer(other).unwrap();
        assert_eq!(other.blend_mode, BlendMode::Screen);
        assert_eq!(other.effects.len(), 1);
        assert_ne!(other.name, comp.layers()[0].name);
    }

    #[test]
    fn newer_or_unversioned_presets_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.json");
        fs::write(&path, r#"{"kind":"effects","effects":[]}"#).unwrap();
        assert!(matches!(
            Preset::load(&path),
            Err(ProjectError::MissingVersion)
        ));
        fs::write(
            &path,
            r#"{"format_version":99,"kind":"effects","effects":[]}"#,
        )
        .unwrap();
        assert!(matches!(
            Preset::load(&path),
            Err(ProjectError::UnsupportedVersion { found: 99, .. })
        ));
        assert_eq!(preset_file_name("  "), "Préréglage.json");
        assert_eq!(preset_file_name("../x"), "-x.json");
    }
}
