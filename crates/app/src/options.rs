//! Command line and initial composition.

use std::path::PathBuf;

use prism_core::{Clip, Composition, FILE_EXTENSION, Project, Slot};

/// Usage text, shown by `--help`.
pub const USAGE: &str = "\
Prism – VJing et projection mapping

Utilisation :
  prism [FICHIER.prism]
  prism [--sortie N] VIDEO.mov [VIDEO.mov ...]

Options :
  --sortie N   ouvre tout de suite la sortie plein écran sur l'écran N
  --help       affiche cette aide

Les vidéos sont placées dans la première colonne, une par couche, puis dans
les colonnes suivantes, et la première colonne démarre.

Raccourcis :
  1 à 9    déclenche la colonne correspondante
  Espace   arrête toutes les couches
  F        ouvre ou ferme la sortie plein écran
  Échap    ferme la sortie plein écran";

/// What the command line asks for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Project file to open.
    pub project: Option<PathBuf>,
    /// Video files to place in the grid.
    pub files: Vec<PathBuf>,
    /// Screen, numbered from 1, on which to open the output at start.
    pub output_screen: Option<usize>,
    /// Whether to print the usage and stop.
    pub help: bool,
}

impl Options {
    /// Reads the command line arguments (without the program name).
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => options.help = true,
                "--sortie" | "--output" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{arg} attend un numéro d'écran"))?;
                    let screen = value
                        .parse::<usize>()
                        .ok()
                        .filter(|screen| *screen >= 1)
                        .ok_or_else(|| format!("numéro d'écran invalide : {value}"))?;
                    options.output_screen = Some(screen);
                }
                flag if flag.starts_with("--") => return Err(format!("option inconnue : {flag}")),
                path => {
                    let path = PathBuf::from(path);
                    if path.extension().is_some_and(|ext| ext == FILE_EXTENSION) {
                        if options.project.replace(path).is_some() {
                            return Err("un seul projet à la fois".into());
                        }
                    } else {
                        options.files.push(path);
                    }
                }
            }
        }
        if options.project.is_some() && !options.files.is_empty() {
            return Err("ouvrez un projet ou des vidéos, pas les deux".into());
        }
        Ok(options)
    }

    /// The composition to start with, and warnings to show.
    pub fn composition(&self) -> Result<(Composition, Vec<String>), String> {
        if let Some(path) = &self.project {
            let project = Project::load(path)
                .map_err(|error| format!("impossible d'ouvrir {} : {error}", path.display()))?;
            let warnings = project
                .missing_media()
                .into_iter()
                .map(|missing| format!("média introuvable : {}", missing.path.display()))
                .collect();
            return Ok((project.composition, warnings));
        }

        let mut composition = Composition::default();
        let layers = composition.layers().len();
        let capacity = layers * composition.columns().len();
        if self.files.len() > capacity {
            return Err(format!("{capacity} vidéos au plus"));
        }
        for (index, file) in self.files.iter().enumerate() {
            let slot = Slot::new(
                composition.layers()[index % layers].id(),
                composition.columns()[index / layers].id(),
            );
            composition
                .set_clip(slot, Some(Clip::from_file(absolute(file))))
                .map_err(|error| error.to_string())?;
        }
        if !self.files.is_empty() {
            let first = composition.columns()[0].id();
            composition
                .trigger_column(first)
                .map_err(|error| error.to_string())?;
        }
        Ok((composition, Vec::new()))
    }
}

fn absolute(path: &std::path::Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn files_and_output_screen() {
        let options = parse(&["--sortie", "2", "a.mov", "b.mov"]).unwrap();
        assert_eq!(options.output_screen, Some(2));
        assert_eq!(
            options.files,
            [PathBuf::from("a.mov"), PathBuf::from("b.mov")]
        );
        assert_eq!(options.project, None);
    }

    #[test]
    fn project_is_recognised_by_its_extension() {
        let options = parse(&["show.prism"]).unwrap();
        assert_eq!(options.project, Some(PathBuf::from("show.prism")));
        assert!(parse(&["a.prism", "b.prism"]).is_err());
        assert!(parse(&["a.prism", "b.mov"]).is_err());
    }

    #[test]
    fn invalid_arguments_are_refused() {
        assert!(parse(&["--sortie"]).is_err());
        assert!(parse(&["--sortie", "0"]).is_err());
        assert!(parse(&["--sortie", "deux"]).is_err());
        assert!(parse(&["--inconnue"]).is_err());
        assert!(parse(&["--help"]).unwrap().help);
    }

    #[test]
    fn files_fill_the_first_column_then_the_next() {
        let names: Vec<String> = (0..6).map(|i| format!("/media/{i}.mov")).collect();
        let options = Options::parse(names).unwrap();
        let (composition, warnings) = options.composition().unwrap();
        assert!(warnings.is_empty());
        let clip_name = |layer: usize, column: usize| {
            let slot = Slot::new(
                composition.layers()[layer].id(),
                composition.columns()[column].id(),
            );
            composition.clip(slot).map(|clip| clip.name.clone())
        };
        assert_eq!(clip_name(0, 0).as_deref(), Some("0"));
        assert_eq!(clip_name(3, 0).as_deref(), Some("3"));
        assert_eq!(clip_name(0, 1).as_deref(), Some("4"));
        assert_eq!(clip_name(1, 1).as_deref(), Some("5"));
        // The first column plays.
        let first = composition.columns()[0].id();
        assert!(
            composition
                .layers()
                .iter()
                .all(|layer| layer.active_column() == Some(first))
        );
    }

    #[test]
    fn too_many_files_are_refused() {
        let names: Vec<String> = (0..33).map(|i| format!("{i}.mov")).collect();
        assert!(Options::parse(names).unwrap().composition().is_err());
    }
}
