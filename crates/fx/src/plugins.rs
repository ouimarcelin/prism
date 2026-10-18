//! Plugins: ISF files dropped in a folder, offered as effects and
//! generators without being picked one by one (ADR 0036).

use std::path::{Path, PathBuf};

/// File extensions of ISF shaders, lowercase.
pub const PLUGIN_EXTENSIONS: [&str; 3] = ["fs", "isf", "frag"];

/// Subfolders looked into, below the plugin folder.
const MAX_DEPTH: usize = 2;

/// An ISF file of the plugin folder.
#[derive(Clone, Debug, PartialEq)]
pub struct Plugin {
    /// The file.
    pub path: PathBuf,
    /// Name shown: the file name without extension, after its subfolders
    /// (« Glitch / Décalage »).
    pub name: String,
    /// Whether it reads an image (an effect) rather than drawing its own (a
    /// generator).
    pub filter: bool,
    /// Description from the header, if any.
    pub description: Option<String>,
    /// Why it cannot be used, if its header cannot be read. The code itself
    /// is compiled when the plugin is used.
    pub error: Option<String>,
}

/// The ISF files of `folder` and its subfolders, sorted by name. A missing
/// folder gives none.
pub fn scan_plugins(folder: &Path) -> Vec<Plugin> {
    let mut plugins = Vec::new();
    scan(folder, &[], &mut plugins);
    plugins.sort_by_key(|plugin| plugin.name.to_lowercase());
    plugins
}

fn scan(folder: &Path, parents: &[String], plugins: &mut Vec<Plugin>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
        else {
            continue;
        };
        if stem.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if parents.len() < MAX_DEPTH {
                let mut inner = parents.to_vec();
                inner.push(stem);
                scan(&path, &inner, plugins);
            }
            continue;
        }
        let is_isf = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                PLUGIN_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
            });
        if !is_isf {
            continue;
        }
        let mut name = parents.join(" / ");
        if !name.is_empty() {
            name.push_str(" / ");
        }
        name.push_str(&stem);
        let header = std::fs::read_to_string(&path)
            .map_err(|error| crate::IsfError::Io(error.to_string()))
            .and_then(|source| crate::isf::summary(&source));
        plugins.push(match header {
            Ok((filter, description)) => Plugin {
                path,
                name,
                filter,
                description,
                error: None,
            },
            Err(error) => Plugin {
                path,
                name,
                filter: false,
                description: None,
                error: Some(error.to_string()),
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILTER: &str = r#"/*{ "DESCRIPTION": "Inverse", "INPUTS": [ { "NAME": "inputImage", "TYPE": "image" } ] }*/
void main() { gl_FragColor = vec4(1.0) - IMG_THIS_PIXEL(inputImage); }"#;
    const GENERATOR: &str = r#"/*{ "INPUTS": [] }*/
void main() { gl_FragColor = vec4(1.0); }"#;

    #[test]
    fn plugins_are_found_with_their_kind() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("inverse.fs"), FILTER).unwrap();
        std::fs::create_dir(dir.path().join("Fonds")).unwrap();
        std::fs::write(dir.path().join("Fonds/blanc.FS"), GENERATOR).unwrap();
        std::fs::write(dir.path().join("Fonds/cassé.fs"), "pas d'en-tête").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "rien").unwrap();
        std::fs::write(dir.path().join(".cache.fs"), GENERATOR).unwrap();

        let plugins = scan_plugins(dir.path());
        let names: Vec<&str> = plugins.iter().map(|plugin| plugin.name.as_str()).collect();
        assert_eq!(names, ["Fonds / blanc", "Fonds / cassé", "inverse"]);
        assert!(!plugins[0].filter && plugins[0].error.is_none());
        assert!(plugins[1].error.is_some());
        assert!(plugins[2].filter);
        assert_eq!(plugins[2].description.as_deref(), Some("Inverse"));
    }

    #[test]
    fn a_missing_folder_has_no_plugins() {
        assert!(scan_plugins(Path::new("/nowhere/prism-plugins")).is_empty());
    }
}
