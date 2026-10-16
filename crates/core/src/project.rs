//! Project files: JSON persistence and missing media detection.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Clip, Composition, Slot};

/// Version of the project file format written by this build.
///
/// Increase it on every incompatible change, and keep reading older versions.
pub const FORMAT_VERSION: u32 = 1;

/// File extension of project files.
pub const FILE_EXTENSION: &str = "prism";

const GENERATOR: &str = concat!("Prism ", env!("CARGO_PKG_VERSION"));

/// Error raised while reading or writing a project file.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// The file could not be read or written.
    #[error("cannot access {}", path.display())]
    Io {
        /// File concerned.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// The content is not a valid project.
    #[error("invalid project file")]
    Format(#[from] serde_json::Error),
    /// The file has no `format_version` field.
    #[error("project file has no format version")]
    MissingVersion,
    /// The file was written by a newer version of Prism.
    #[error("project format version {found} is newer than supported version {supported}")]
    UnsupportedVersion {
        /// Version found in the file.
        found: u64,
        /// Highest version this build reads.
        supported: u32,
    },
}

/// A clip whose media file cannot be found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingMedia {
    /// Slot of the clip.
    pub slot: Slot,
    /// Path that does not exist.
    pub path: PathBuf,
}

/// A Prism project: the unit saved to and loaded from a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    /// The composition of the project.
    pub composition: Composition,
}

#[derive(Serialize)]
struct DocumentOut<'a> {
    format_version: u32,
    generator: &'a str,
    composition: &'a Composition,
}

#[derive(Deserialize)]
struct DocumentIn {
    composition: Composition,
}

impl Project {
    /// Creates a project around a composition.
    pub fn new(composition: Composition) -> Self {
        Self { composition }
    }

    /// Serializes the project to pretty-printed JSON. Media and effect file
    /// paths located inside `base_dir` are stored relative to it, with `/`
    /// separators.
    pub fn to_json(&self, base_dir: Option<&Path>) -> Result<String, ProjectError> {
        let mut composition = self.composition.clone();
        if let Some(base) = base_dir {
            for clip in composition.clips_mut() {
                map_media_path(clip, |path| relativize(path, base));
            }
            for effect in composition.all_effects_mut() {
                if let Some(file) = &mut effect.file {
                    *file = relativize(file, base);
                }
            }
        }
        let document = DocumentOut {
            format_version: FORMAT_VERSION,
            generator: GENERATOR,
            composition: &composition,
        };
        Ok(serde_json::to_string_pretty(&document)?)
    }

    /// Parses a project from JSON. Relative media and effect file paths are
    /// resolved against `base_dir`.
    pub fn from_json(json: &str, base_dir: Option<&Path>) -> Result<Self, ProjectError> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        let version = value
            .get("format_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or(ProjectError::MissingVersion)?;
        if version > u64::from(FORMAT_VERSION) {
            return Err(ProjectError::UnsupportedVersion {
                found: version,
                supported: FORMAT_VERSION,
            });
        }
        let mut composition = serde_json::from_value::<DocumentIn>(value)?.composition;
        if let Some(base) = base_dir {
            for clip in composition.clips_mut() {
                map_media_path(clip, |path| resolve(path, base));
            }
            for effect in composition.all_effects_mut() {
                if let Some(file) = &mut effect.file {
                    *file = resolve(file, base);
                }
            }
        }
        Ok(Self { composition })
    }

    /// Writes the project to a file. The write is atomic: on failure, an
    /// existing file at `path` is left intact.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ProjectError> {
        let path = absolute(path.as_ref())?;
        let json = self.to_json(path.parent())?;
        write_atomic(&path, json.as_bytes()).map_err(|source| ProjectError::Io { path, source })
    }

    /// Reads a project from a file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let path = absolute(path.as_ref())?;
        let json = fs::read_to_string(&path).map_err(|source| ProjectError::Io {
            path: path.clone(),
            source,
        })?;
        Self::from_json(&json, path.parent())
    }

    /// Clips whose media file does not exist, in grid order.
    pub fn missing_media(&self) -> Vec<MissingMedia> {
        self.composition
            .clips()
            .filter_map(|(slot, clip)| {
                let path = clip.source.path()?;
                let exists = path.try_exists().unwrap_or(false);
                (!exists).then(|| MissingMedia {
                    slot,
                    path: path.to_path_buf(),
                })
            })
            .collect()
    }
}

fn map_media_path(clip: &mut Clip, f: impl Fn(&Path) -> PathBuf) {
    if let Some(path) = clip.source.path_mut() {
        *path = f(path);
    }
}

/// Path relative to `base` when `path` is absolute and inside `base`,
/// otherwise `path` unchanged.
fn relativize(path: &Path, base: &Path) -> PathBuf {
    match path.strip_prefix(base) {
        Ok(relative) if path.is_absolute() => relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
            .into(),
        _ => path.to_path_buf(),
    }
}

fn resolve(path: &Path, base: &Path) -> PathBuf {
    if path.is_relative() {
        base.join(path)
    } else {
        path.to_path_buf()
    }
}

fn absolute(path: &Path) -> Result<PathBuf, ProjectError> {
    std::path::absolute(path).map_err(|source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes to a temporary file in the same folder, then renames it over `path`.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let temporary = path.with_file_name(format!(".{}.tmp", file_name.to_string_lossy()));
    let result = (|| {
        let mut file = File::create(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relativize_only_inside_base() {
        let base = Path::new("/shows/tour");
        assert_eq!(
            relativize(Path::new("/shows/tour/media/a.mov"), base),
            PathBuf::from("media/a.mov")
        );
        assert_eq!(
            relativize(Path::new("/Volumes/Media/b.mov"), base),
            PathBuf::from("/Volumes/Media/b.mov")
        );
        assert_eq!(relativize(Path::new("c.mov"), base), PathBuf::from("c.mov"));
    }

    #[test]
    fn resolve_keeps_absolute_paths() {
        let base = Path::new("/shows/tour");
        assert_eq!(
            resolve(Path::new("media/a.mov"), base),
            PathBuf::from("/shows/tour/media/a.mov")
        );
        assert_eq!(
            resolve(Path::new("/Volumes/Media/b.mov"), base),
            PathBuf::from("/Volumes/Media/b.mov")
        );
    }
}
