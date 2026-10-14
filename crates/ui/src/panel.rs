//! The control window: menus, clip grid, properties, crossfader and status.

use std::collections::HashSet;
use std::path::PathBuf;

use egui::{Key, KeyboardShortcut, Modifiers};
use prism_core::{ColumnId, MissingMedia, Resolution, Slot};
use prism_engine::{Command, EngineStatus};

use crate::crossfade::AutoFade;
use crate::document::{Document, IMAGE_EXTENSIONS, VIDEO_EXTENSIONS};
use crate::preferences::Preferences;
use crate::shortcuts::Shortcut;

/// A screen that can show the full-screen output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    /// Name given by the system.
    pub name: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Whether it is the main screen.
    pub primary: bool,
}

/// Something the application must do after a frame of the interface.
#[derive(Debug)]
pub enum Action {
    /// Opens the full-screen output on the screen of this index.
    OpenOutput(usize),
    /// Opens the output if closed, closes it otherwise.
    ToggleOutput,
    /// Closes the full-screen output.
    CloseOutput,
    /// Sends a command to the engine.
    Engine(Command),
    /// Quits the application.
    Quit,
}

/// What the interface shows besides the document.
pub struct FrameInfo<'a> {
    /// Engine state after its last frame.
    pub status: &'a EngineStatus,
    /// Program image registered with the renderer, and its size.
    pub preview: Option<(egui::TextureId, Resolution)>,
    /// Screens available for the output.
    pub screens: &'a [Screen],
    /// Screen showing the output, if it is open.
    pub output_screen: Option<usize>,
}

/// What waits for an answer about unsaved changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    New,
    Open,
    OpenPath(PathBuf),
    Quit,
}

#[derive(Default)]
pub(crate) struct Dialogs {
    pub(crate) about: bool,
    pub(crate) shortcuts: bool,
    pub(crate) composition: bool,
    pub(crate) missing: Vec<MissingMedia>,
    pub(crate) message: Option<String>,
    pub(crate) unsaved: Option<Pending>,
}

/// State of the control window around the document being edited.
pub struct ControlPanel {
    pub(crate) document: Document,
    pub(crate) selected: Option<Slot>,
    pub(crate) dialogs: Dialogs,
    pub(crate) actions: Vec<Action>,
    pub(crate) missing_cache: (u64, HashSet<Slot>),
    pub(crate) preferences: Preferences,
    preferences_path: Option<PathBuf>,
    pub(crate) auto_fade: Option<AutoFade>,
}

impl ControlPanel {
    /// Shows `document`; `missing` lists the clips whose media was not found
    /// when it was opened.
    pub fn new(document: Document, missing: Vec<MissingMedia>) -> Self {
        let selected = first_slot(&document);
        Self {
            document,
            selected,
            dialogs: Dialogs {
                missing,
                ..Dialogs::default()
            },
            actions: Vec::new(),
            missing_cache: (u64::MAX, HashSet::new()),
            preferences: Preferences::default(),
            preferences_path: None,
            auto_fade: None,
        }
    }

    /// Uses `preferences`, saved to `path` whenever they change.
    pub fn with_preferences(mut self, preferences: Preferences, path: Option<PathBuf>) -> Self {
        self.preferences = preferences;
        self.preferences_path = path;
        self
    }

    /// Preferences in use.
    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }

    pub(crate) fn save_preferences(&mut self) {
        if let Some(path) = &self.preferences_path
            && let Err(error) = self.preferences.save(path)
        {
            self.dialogs.message = Some(format!(
                "Préférences non enregistrées : {}",
                crate::describe(&error)
            ));
        }
    }

    /// Remembers the screen showing the output, for next time.
    pub fn remember_output_screen(&mut self, name: &str) {
        if self.preferences.output_screen.as_deref() != Some(name) {
            self.preferences.output_screen = Some(name.to_owned());
            self.save_preferences();
        }
    }

    /// Whether something moves on its own and needs frequent redraws.
    pub fn is_animating(&self) -> bool {
        self.auto_fade.is_some()
    }

    pub(crate) fn remember_project(&mut self) {
        if let Some(path) = self.document.path().map(std::path::Path::to_path_buf) {
            self.preferences.add_recent(&path);
            self.save_preferences();
        }
    }

    /// The project being edited.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Window title.
    pub fn title(&self) -> String {
        let changed = if self.document.is_dirty() {
            " (modifié)"
        } else {
            ""
        };
        format!("Prism – {}{changed}", self.document.display_name())
    }

    /// Places files dropped on the window, from the selected slot.
    pub fn drop_files(&mut self, files: &[PathBuf]) {
        let Some(start) = self.selected.or_else(|| first_slot(&self.document)) else {
            return;
        };
        if self.document.import(files, start) == 0 {
            self.dialogs.message = Some(format!(
                "Aucun média reconnu. Formats acceptés : {}, {}, ou un dossier d'images PNG.",
                VIDEO_EXTENSIONS.join(", "),
                IMAGE_EXTENSIONS.join(", ")
            ));
        }
    }

    /// Applies a shortcut typed in a window without interface, such as the
    /// output window.
    pub fn shortcut(&mut self, shortcut: Shortcut) {
        let before = self.document.composition().clone();
        match shortcut {
            Shortcut::TriggerColumn(index) => {
                let column = self
                    .document
                    .composition()
                    .columns()
                    .get(index)
                    .map(|c| c.id());
                if let Some(column) = column {
                    let _ = self.document.composition_mut().trigger_column(column);
                }
            }
            Shortcut::StopAll => self.document.composition_mut().stop_all(),
            Shortcut::ToggleOutput => self.actions.push(Action::ToggleOutput),
            Shortcut::CloseOutput => self.actions.push(Action::CloseOutput),
            Shortcut::Quit => {
                if self.request_quit() {
                    self.actions.push(Action::Quit);
                }
            }
        }
        self.document.commit(&before);
    }

    /// Asks to quit. Returns `true` when the application may quit now;
    /// otherwise the interface asks about unsaved changes and sends
    /// [`Action::Quit`] once answered.
    pub fn request_quit(&mut self) -> bool {
        if self.document.is_dirty() {
            self.dialogs.unsaved = Some(Pending::Quit);
            false
        } else {
            true
        }
    }

    /// Actions requested outside a frame, by [`ControlPanel::shortcut`].
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }

    /// Draws one frame of the interface in `ui`, the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui, frame: &FrameInfo<'_>) -> Vec<Action> {
        let before = self.document.composition().clone();
        self.keyboard(ui.ctx());
        self.refresh_missing();

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui, frame));
        egui::Panel::bottom("transport")
            .resizable(false)
            .show(ui, |ui| self.transport(ui, frame.status));
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(340.0)
            .min_size(260.0)
            .show(ui, |ui| self.inspector(ui, frame));
        egui::CentralPanel::default().show(ui, |ui| self.grid(ui, frame.status));
        self.dialogs(ui.ctx());

        self.document.commit(&before);
        std::mem::take(&mut self.actions)
    }

    /// Recomputes the slots whose media is missing when the document changed.
    fn refresh_missing(&mut self) {
        if self.missing_cache.0 != self.document.revision() {
            let missing = self
                .document
                .missing_media()
                .into_iter()
                .map(|m| m.slot)
                .collect();
            self.missing_cache = (self.document.revision(), missing);
        }
    }

    pub(crate) fn is_missing(&self, slot: Slot) -> bool {
        self.missing_cache.1.contains(&slot)
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        let command = |key| KeyboardShortcut::new(Modifiers::COMMAND, key);
        let command_shift = |key| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, key);
        let (new, open, save_as, save, quit) = ctx.input_mut(|input| {
            (
                input.consume_shortcut(&command(Key::N)),
                input.consume_shortcut(&command(Key::O)),
                input.consume_shortcut(&command_shift(Key::S)),
                input.consume_shortcut(&command(Key::S)),
                input.consume_shortcut(&command(Key::Q)),
            )
        });
        if quit {
            self.shortcut(Shortcut::Quit);
        }
        if new {
            self.new_project();
        }
        if open {
            self.open_project();
        }
        if save_as {
            self.save_project_as();
        } else if save {
            self.save_project();
        }

        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let shortcuts: Vec<Shortcut> = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } if !modifiers.command && !modifiers.alt && !modifiers.ctrl => match key {
                        Key::Space => Some(Shortcut::StopAll),
                        Key::Escape => Some(Shortcut::CloseOutput),
                        Key::F => Some(Shortcut::ToggleOutput),
                        other => digit(*other).map(Shortcut::TriggerColumn),
                    },
                    _ => None,
                })
                .collect()
        });
        for shortcut in shortcuts {
            self.shortcut(shortcut);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui, frame: &FrameInfo<'_>) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Fichier", |ui| {
                if ui
                    .add(egui::Button::new("Nouveau").shortcut_text("Cmd+N"))
                    .clicked()
                {
                    self.new_project();
                }
                if ui
                    .add(egui::Button::new("Ouvrir…").shortcut_text("Cmd+O"))
                    .clicked()
                {
                    self.open_project();
                }
                ui.menu_button("Projets récents", |ui| {
                    if self.preferences.recent_projects.is_empty() {
                        ui.label("Aucun");
                    }
                    for path in self.preferences.recent_projects.clone() {
                        let name = path.file_name().map_or_else(
                            || path.display().to_string(),
                            |name| name.to_string_lossy().into_owned(),
                        );
                        if ui
                            .button(name)
                            .on_hover_text(path.display().to_string())
                            .clicked()
                        {
                            self.open_recent(path);
                        }
                    }
                });
                ui.separator();
                if ui
                    .add(egui::Button::new("Enregistrer").shortcut_text("Cmd+S"))
                    .clicked()
                {
                    self.save_project();
                }
                if ui
                    .add(egui::Button::new("Enregistrer sous…").shortcut_text("Cmd+Maj+S"))
                    .clicked()
                {
                    self.save_project_as();
                }
                ui.separator();
                if ui.button("Importer des médias…").clicked() {
                    self.import_dialog(self.selected.or_else(|| first_slot(&self.document)));
                }
                if ui.button("Importer une séquence d'images…").clicked() {
                    self.import_sequence_dialog(
                        self.selected.or_else(|| first_slot(&self.document)),
                    );
                }
                if ui.button("Médias manquants…").clicked() {
                    self.dialogs.missing = self.document.missing_media();
                    if self.dialogs.missing.is_empty() {
                        self.dialogs.message = Some("Tous les médias sont présents.".into());
                    }
                }
                ui.separator();
                if ui
                    .add(egui::Button::new("Quitter").shortcut_text("Cmd+Q"))
                    .clicked()
                {
                    self.shortcut(Shortcut::Quit);
                }
            });
            ui.menu_button("Composition", |ui| {
                if ui.button("Réglages…").clicked() {
                    self.dialogs.composition = true;
                }
                ui.separator();
                if ui.button("Ajouter une couche").clicked() {
                    let number = self.document.composition().layers().len() + 1;
                    self.document
                        .composition_mut()
                        .add_layer(format!("Couche {number}"));
                }
                if ui.button("Ajouter une colonne").clicked() {
                    let number = self.document.composition().columns().len() + 1;
                    self.document
                        .composition_mut()
                        .add_column(format!("Colonne {number}"));
                }
                ui.separator();
                if ui.button("Tout arrêter").clicked() {
                    self.document.composition_mut().stop_all();
                }
            });
            ui.menu_button("Sortie", |ui| {
                if frame.screens.is_empty() {
                    ui.label("Aucun écran détecté");
                }
                for (index, screen) in frame.screens.iter().enumerate() {
                    let label = format!(
                        "Plein écran sur {}. {} ({} × {}){}",
                        index + 1,
                        screen.name,
                        screen.width,
                        screen.height,
                        if screen.primary { ", principal" } else { "" }
                    );
                    let selected = frame.output_screen == Some(index);
                    if ui.add(egui::Button::selectable(selected, label)).clicked() {
                        self.actions.push(Action::OpenOutput(index));
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(
                        frame.output_screen.is_some(),
                        egui::Button::new("Fermer la sortie").shortcut_text("Échap"),
                    )
                    .clicked()
                {
                    self.actions.push(Action::CloseOutput);
                }
            });
            ui.menu_button("Aide", |ui| {
                if ui.button("Raccourcis clavier").clicked() {
                    self.dialogs.shortcuts = true;
                }
                if ui.button("À propos de Prism").clicked() {
                    self.dialogs.about = true;
                }
            });
        });
    }

    fn transport(&mut self, ui: &mut egui::Ui, status: &EngineStatus) {
        let dt = ui.input(|input| input.stable_dt).min(0.1);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let seconds = self.preferences.crossfade_seconds;
            let start = self.document.composition().crossfader.position.get();
            let mut fade_to = None;
            if ui
                .button("◀ A")
                .on_hover_text(format!("Crossfader vers A en {seconds:.1} s"))
                .clicked()
            {
                fade_to = Some(0.0);
            }
            ui.label(egui::RichText::new("A").strong());
            let crossfader = &mut self.document.composition_mut().crossfader;
            let mut position = crossfader.position.get();
            let slider = egui::Slider::new(&mut position, 0.0..=1.0).show_value(false);
            let response = ui
                .add_sized([320.0, 20.0], slider)
                .on_hover_text("Crossfader : double-clic pour centrer");
            if response.double_clicked() {
                position = 0.5;
            }
            // Touching the crossfader stops an automatic move.
            if response.dragged() || response.double_clicked() {
                self.auto_fade = None;
            }
            if let Some(fade) = &mut self.auto_fade {
                let (next, done) = fade.step(dt);
                position = next;
                if done {
                    self.auto_fade = None;
                }
            }
            crossfader.position = prism_core::Unit::new(position);
            ui.label(egui::RichText::new("B").strong());
            if ui
                .button("B ▶")
                .on_hover_text(format!("Crossfader vers B en {seconds:.1} s"))
                .clicked()
            {
                fade_to = Some(1.0);
            }
            egui::ComboBox::from_id_salt("crossfader curve")
                .selected_text(crate::labels::curve(crossfader.curve))
                .show_ui(ui, |ui| {
                    for curve in [
                        prism_core::CrossfaderCurve::FullCenter,
                        prism_core::CrossfaderCurve::Linear,
                        prism_core::CrossfaderCurve::EqualPower,
                    ] {
                        ui.selectable_value(
                            &mut crossfader.curve,
                            curve,
                            crate::labels::curve(curve),
                        );
                    }
                });
            let mut seconds = self.preferences.crossfade_seconds;
            let duration = ui
                .add(
                    egui::DragValue::new(&mut seconds)
                        .range(0.0..=60.0)
                        .speed(0.05)
                        .suffix(" s"),
                )
                .on_hover_text("Durée du fondu automatique");
            if duration.changed() {
                self.preferences.crossfade_seconds = seconds;
            }
            if duration.drag_stopped() || duration.lost_focus() {
                self.save_preferences();
            }
            if let Some(target) = fade_to {
                self.auto_fade = Some(AutoFade::new(start, target, seconds));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(status_line(status)).on_hover_text(STATUS_HELP);
            });
        });
        ui.add_space(4.0);
    }

    pub(crate) fn trigger_column(&mut self, column: ColumnId) {
        let _ = self.document.composition_mut().trigger_column(column);
    }

    // File operations.

    fn new_project(&mut self) {
        if self.document.is_dirty() {
            self.dialogs.unsaved = Some(Pending::New);
            return;
        }
        self.replace_document(Document::default(), Vec::new());
    }

    fn open_project(&mut self) {
        if self.document.is_dirty() {
            self.dialogs.unsaved = Some(Pending::Open);
            return;
        }
        self.pick_and_open();
    }

    /// Asks for a project file and opens it, replacing the document.
    fn pick_and_open(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Projet Prism", &[prism_core::FILE_EXTENSION])
            .pick_file()
        else {
            return;
        };
        self.open_path(path);
    }

    /// Opens a project from the recent list, after asking about unsaved
    /// changes.
    fn open_recent(&mut self, path: PathBuf) {
        if self.document.is_dirty() {
            self.dialogs.unsaved = Some(Pending::OpenPath(path));
            return;
        }
        self.open_path(path);
    }

    fn open_path(&mut self, path: PathBuf) {
        match Document::open(&path) {
            Ok((document, missing)) => {
                self.replace_document(document, missing);
                self.remember_project();
            }
            Err(error) => {
                self.dialogs.message = Some(format!(
                    "Impossible d'ouvrir {} : {}",
                    path.display(),
                    crate::describe(&error)
                ));
            }
        }
    }

    fn replace_document(&mut self, document: Document, missing: Vec<MissingMedia>) {
        // Keep the revision increasing, so the engine sees the new project.
        let revision = self.document.revision();
        self.document = document;
        self.document.bump_revision_past(revision);
        self.selected = first_slot(&self.document);
        self.dialogs.missing = missing;
    }

    /// Saves; returns whether the document is saved afterwards.
    pub(crate) fn save_project(&mut self) -> bool {
        match self.document.save() {
            Ok(true) => {
                self.remember_project();
                true
            }
            Ok(false) => self.save_project_as(),
            Err(error) => {
                self.dialogs.message = Some(format!(
                    "Enregistrement impossible : {}",
                    crate::describe(&error)
                ));
                false
            }
        }
    }

    pub(crate) fn save_project_as(&mut self) -> bool {
        let name = format!(
            "{}.{}",
            self.document
                .path()
                .and_then(|path| path.file_stem())
                .map_or_else(|| "Sans titre".into(), |stem| stem.to_string_lossy()),
            prism_core::FILE_EXTENSION
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Projet Prism", &[prism_core::FILE_EXTENSION])
            .set_file_name(name)
            .save_file()
        else {
            return false;
        };
        match self.document.save_as(&path) {
            Ok(()) => {
                self.remember_project();
                true
            }
            Err(error) => {
                self.dialogs.message = Some(format!(
                    "Enregistrement impossible : {}",
                    crate::describe(&error)
                ));
                false
            }
        }
    }

    pub(crate) fn import_dialog(&mut self, start: Option<Slot>) {
        let Some(start) = start else {
            return;
        };
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("Vidéos et images", &media_extensions())
            .pick_files()
        {
            self.document.import(&files, start);
            self.selected = Some(start);
        }
    }

    pub(crate) fn import_sequence_dialog(&mut self, start: Option<Slot>) {
        let Some(start) = start else {
            return;
        };
        if let Some(folder) = rfd::FileDialog::new()
            .set_title("Dossier d'images PNG")
            .pick_folder()
        {
            self.document.import(&[folder], start);
            self.selected = Some(start);
        }
    }

    pub(crate) fn relocate_dialog(&mut self, slot: Slot) {
        let sequence = matches!(
            self.document
                .composition()
                .clip(slot)
                .map(|clip| &clip.source),
            Some(prism_core::MediaSource::ImageSequence { .. })
        );
        let path = if sequence {
            rfd::FileDialog::new()
                .set_title("Dossier d'images PNG")
                .pick_folder()
        } else {
            rfd::FileDialog::new()
                .add_filter("Vidéos et images", &media_extensions())
                .pick_file()
        };
        if let Some(path) = path {
            self.document.relink(slot, path);
        }
    }

    /// Continues what waited for an answer about unsaved changes.
    pub(crate) fn resume(&mut self, pending: Pending) {
        match pending {
            Pending::New => self.replace_document(Document::default(), Vec::new()),
            Pending::Open => self.pick_and_open(),
            Pending::OpenPath(path) => self.open_path(path),
            Pending::Quit => self.actions.push(Action::Quit),
        }
    }
}

/// Extensions offered by the file dialogs.
fn media_extensions() -> Vec<&'static str> {
    VIDEO_EXTENSIONS
        .iter()
        .chain(&IMAGE_EXTENSIONS)
        .copied()
        .collect()
}

/// First slot of the top layer.
fn first_slot(document: &Document) -> Option<Slot> {
    let composition = document.composition();
    let layer = composition.layers().last()?;
    let column = composition.columns().first()?;
    Some(Slot::new(layer.id(), column.id()))
}

fn digit(key: Key) -> Option<usize> {
    let digits = [
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
    ];
    digits.iter().position(|candidate| *candidate == key)
}

/// Explanation of the counters of the status line.
const STATUS_HELP: &str = "Saccades : images montrées plus d'une image et demie après la précédente. \
Sautées : le GPU était encore occupé par les images d'avant. \
En retard : l'image voulue n'était pas décodée à temps. \
Compté depuis le lancement.";

/// Frame rate, work per frame and the problems counted since the start.
fn status_line(status: &EngineStatus) -> String {
    let late: u64 = status.layers.iter().map(|layer| layer.late_frames).sum();
    format!(
        "{:.1} i/s · {:.1} ms par image · saccades : {} · sautées : {} · en retard : {late}",
        status.fps, status.frame_time_ms, status.hitches, status.gpu_skips
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_engine::LayerStatus;

    #[test]
    fn status_line_counts_every_kind_of_problem() {
        let status = EngineStatus {
            fps: 59.94,
            frame_time_ms: 2.25,
            hitches: 3,
            gpu_skips: 2,
            layers: vec![
                LayerStatus {
                    late_frames: 1,
                    ..LayerStatus::default()
                },
                LayerStatus {
                    late_frames: 4,
                    ..LayerStatus::default()
                },
            ],
            ..EngineStatus::default()
        };
        assert_eq!(
            status_line(&status),
            "59.9 i/s · 2.2 ms par image · saccades : 3 · sautées : 2 · en retard : 5"
        );
    }
}
