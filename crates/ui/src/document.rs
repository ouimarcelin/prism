//! The project being edited: composition, file, unsaved changes, media
//! import and relinking.

use std::path::{Path, PathBuf};

use prism_core::{
    Clip, Composition, FILE_EXTENSION, MediaSource, MissingMedia, Project, ProjectError, Slot,
};

/// File extensions accepted as video clips.
pub const VIDEO_EXTENSIONS: [&str; 3] = ["mov", "mp4", "m4v"];

/// How deep [`Document::relink_missing`] searches inside the chosen folder.
const RELINK_DEPTH: usize = 8;

/// The project being edited.
#[derive(Clone, Debug)]
pub struct Document {
    composition: Composition,
    path: Option<PathBuf>,
    /// Content as last saved or opened, to tell unsaved changes.
    saved: Composition,
    /// Increases on every change, playback included, to tell when the
    /// engine needs the new composition.
    revision: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self::new(Composition::default())
    }
}

impl Document {
    /// A new, unsaved project.
    pub fn new(composition: Composition) -> Self {
        Self {
            saved: composition.clone(),
            composition,
            path: None,
            revision: 0,
        }
    }

    /// Opens a project file. Also returns the clips whose media is missing.
    pub fn open(path: &Path) -> Result<(Self, Vec<MissingMedia>), ProjectError> {
        let project = Project::load(path)?;
        let missing = project.missing_media();
        let mut document = Self::new(project.composition);
        document.path = Some(path.to_path_buf());
        Ok((document, missing))
    }

    /// Saves to the current file. Without one, nothing happens and `false`
    /// is returned: ask for a path and use [`Document::save_as`].
    pub fn save(&mut self) -> Result<bool, ProjectError> {
        let Some(path) = self.path.clone() else {
            return Ok(false);
        };
        self.save_as(&path)?;
        Ok(true)
    }

    /// Saves to `path`, adding the project extension if missing, and makes
    /// it the current file.
    pub fn save_as(&mut self, path: &Path) -> Result<(), ProjectError> {
        let path = if path.extension().is_some_and(|ext| ext == FILE_EXTENSION) {
            path.to_path_buf()
        } else {
            path.with_extension(FILE_EXTENSION)
        };
        let mut composition = self.composition.clone();
        composition.stop_all();
        Project::new(composition).save(&path)?;
        self.saved = self.composition.clone();
        self.path = Some(path);
        Ok(())
    }

    /// The composition.
    pub fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The composition, for editing. Call [`Document::commit`] after the
    /// edits of a frame.
    pub fn composition_mut(&mut self) -> &mut Composition {
        &mut self.composition
    }

    /// Records the edits made since `before`, a copy taken earlier.
    pub fn commit(&mut self, before: &Composition) {
        if &self.composition != before {
            self.revision += 1;
        }
    }

    /// Increases on every change, playback included.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Makes the revision greater than `revision`, so a replaced document is
    /// always seen as new.
    pub fn bump_revision_past(&mut self, revision: u64) {
        self.revision = self.revision.max(revision + 1);
    }

    /// Whether the content differs from the saved file.
    pub fn is_dirty(&self) -> bool {
        !self.composition.same_content(&self.saved)
    }

    /// Current file, if the project was saved or opened.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// File name for window titles.
    pub fn display_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .map_or_else(
                || "Sans titre".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            )
    }

    /// Places video `files` from `start`, one per slot to the right on the
    /// same layer, replacing what is there and adding columns when needed.
    /// Other files are ignored. Returns the number of clips placed.
    pub fn import(&mut self, files: &[PathBuf], start: Slot) -> usize {
        let videos: Vec<&PathBuf> = files.iter().filter(|path| is_video(path)).collect();
        let Some(mut column) = self.composition.column_index(start.column) else {
            return 0;
        };
        let mut placed = 0;
        for file in videos {
            while column >= self.composition.columns().len() {
                let number = self.composition.columns().len() + 1;
                self.composition.add_column(format!("Colonne {number}"));
            }
            let slot = Slot::new(start.layer, self.composition.columns()[column].id());
            if self
                .composition
                .set_clip(slot, Some(Clip::from_file(file.clone())))
                .is_ok()
            {
                placed += 1;
            }
            column += 1;
        }
        if placed > 0 {
            self.revision += 1;
        }
        placed
    }

    /// Points the clip of `slot` to `path`.
    pub fn relink(&mut self, slot: Slot, path: PathBuf) {
        if let Some(clip) = self.composition.clip_mut(slot) {
            clip.source = MediaSource::File { path };
            self.revision += 1;
        }
    }

    /// Looks in `folder` and its subfolders for files with the names of the
    /// missing media, and relinks the clips found. Returns how many.
    pub fn relink_missing(&mut self, folder: &Path) -> usize {
        let missing = Project::new(self.composition.clone()).missing_media();
        if missing.is_empty() {
            return 0;
        }
        let mut found = Vec::new();
        collect_files(folder, RELINK_DEPTH, &mut found);
        let mut relinked = 0;
        for media in missing {
            let Some(name) = media.path.file_name() else {
                continue;
            };
            if let Some(path) = found.iter().find(|path| path.file_name() == Some(name)) {
                self.relink(media.slot, path.clone());
                relinked += 1;
            }
        }
        relinked
    }

    /// Clips whose media file cannot be found.
    pub fn missing_media(&self) -> Vec<MissingMedia> {
        Project::new(self.composition.clone()).missing_media()
    }
}

/// Whether a path names a supported video file.
pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Files under `folder`, visible ones only, down to `depth` levels.
fn collect_files(folder: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if hidden {
            continue;
        }
        match entry.file_type() {
            Ok(kind) if kind.is_dir() && depth > 0 => collect_files(&path, depth - 1, found),
            Ok(kind) if kind.is_file() => found.push(path),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_slot(document: &Document) -> Slot {
        let composition = document.composition();
        Slot::new(composition.layers()[0].id(), composition.columns()[0].id())
    }

    #[test]
    fn import_fills_to_the_right_and_adds_columns() {
        let mut document = Document::default();
        let columns = document.composition().columns().len();
        let start = Slot::new(
            document.composition().layers()[1].id(),
            document.composition().columns()[columns - 1].id(),
        );
        let files = [
            PathBuf::from("/m/a.mov"),
            PathBuf::from("/m/notes.txt"),
            PathBuf::from("/m/b.MP4"),
        ];
        assert_eq!(document.import(&files, start), 2);
        assert_eq!(document.composition().columns().len(), columns + 1);
        let last = document.composition().columns()[columns].id();
        let clip = document
            .composition()
            .clip(Slot::new(start.layer, last))
            .unwrap();
        assert_eq!(clip.name, "b");
        assert!(document.is_dirty());
        assert_eq!(document.revision(), 1);
    }

    #[test]
    fn playback_changes_are_not_unsaved_changes() {
        let mut document = Document::default();
        let slot = first_slot(&document);
        document.import(&[PathBuf::from("/m/a.mov")], slot);
        let dir = tempfile::tempdir().unwrap();
        document.save_as(&dir.path().join("show")).unwrap();
        assert!(!document.is_dirty());
        assert_eq!(document.path().unwrap().extension().unwrap(), "prism");
        assert_eq!(document.display_name(), "show.prism");

        let before = document.composition().clone();
        document.composition_mut().trigger_clip(slot).unwrap();
        document.commit(&before);
        assert!(!document.is_dirty());
        assert_eq!(document.revision(), 2);
    }

    #[test]
    fn saved_files_do_not_record_playing_clips() {
        let mut document = Document::default();
        let slot = first_slot(&document);
        document.import(&[PathBuf::from("/m/a.mov")], slot);
        document.composition_mut().trigger_clip(slot).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("show.prism");
        document.save_as(&path).unwrap();
        // Still playing in the editor.
        assert!(document.composition().active_clip(slot.layer).is_some());
        let (opened, _) = Document::open(&path).unwrap();
        assert!(opened.composition().active_clip(slot.layer).is_none());
    }

    #[test]
    fn missing_media_is_found_by_name_in_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("shows").join("tour");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("a.mov"), b"").unwrap();
        std::fs::create_dir_all(dir.path().join(".cache")).unwrap();
        std::fs::write(dir.path().join(".cache").join("b.mov"), b"").unwrap();

        let mut document = Document::default();
        let slot = first_slot(&document);
        document.import(
            &[PathBuf::from("/gone/a.mov"), PathBuf::from("/gone/b.mov")],
            slot,
        );
        assert_eq!(document.missing_media().len(), 2);
        assert_eq!(document.relink_missing(dir.path()), 1);
        let remaining = document.missing_media();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].path, PathBuf::from("/gone/b.mov"));
        let clip = document.composition().clip(slot).unwrap();
        assert_eq!(clip.source.path(), Some(nested.join("a.mov").as_path()));
    }

    #[test]
    fn save_without_a_file_asks_for_one() {
        let mut document = Document::default();
        assert!(!document.save().unwrap());
        assert_eq!(document.display_name(), "Sans titre");
    }
}
