//! The control window: menus, clip grid, properties, crossfader and status.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use egui::{Key, KeyboardShortcut, Modifiers};
use prism_clock::{BEATS_PER_BAR, TapTempo, beat_in_bar};
use prism_control::{ControlMessage, Controls, Outcome};
use prism_core::{
    BAND_COUNT, Band, Bpm, ColumnId, MAX_BPM, MIN_BPM, MissingMedia, Resolution, Slot,
};
use prism_engine::{Command, EngineStatus};

use crate::crossfade::AutoFade;
use crate::document::{Document, IMAGE_EXTENSIONS, VIDEO_EXTENSIONS};
use crate::learn::Learn;
use crate::preferences::Preferences;
use crate::presets::PresetStore;
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
    /// Opens a full-screen output on the screen of this index, or closes it
    /// if one is open there.
    ToggleScreen(usize),
    /// Opens the outputs if none is open, closes them otherwise.
    ToggleOutput,
    /// Closes every full-screen output.
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
    /// Screens showing a full-screen output.
    pub output_screens: &'a [usize],
    /// Audio input in use.
    pub audio: AudioStatus,
    /// MIDI inputs.
    pub midi: MidiStatus,
}

/// MIDI inputs, for the interface.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MidiStatus {
    /// Inputs present on the computer.
    pub ports: Vec<String>,
    /// Inputs listened to.
    pub connected: Vec<String>,
    /// Inputs that could not be opened, with why.
    pub errors: Vec<String>,
    /// Port listened on for OSC, if listening.
    pub osc_port: Option<u16>,
    /// Why OSC cannot be listened to or sent.
    pub osc_errors: Vec<String>,
}

/// State of the audio input, for the interface.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioStatus {
    /// Name of the input measured, if one is.
    pub device: Option<String>,
    /// Why the input asked for could not start.
    pub error: Option<String>,
    /// Levels of the bands, from 0 to 1, in the order of [`Band::ALL`].
    pub levels: [f32; BAND_COUNT],
    /// Output playing the sound of the clips, if one is.
    pub output: Option<String>,
    /// Why the sound of the clips cannot be played.
    pub output_error: Option<String>,
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
    pub(crate) mappings: bool,
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
    tap: TapTempo,
    /// Input devices, read when the audio menu opens.
    audio_devices: Option<Vec<String>>,
    pub(crate) presets: PresetStore,
    pub(crate) learn: Learn,
    controls: Controls,
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
            tap: TapTempo::default(),
            audio_devices: None,
            presets: PresetStore::default(),
            learn: Learn::default(),
            controls: Controls::default(),
        }
    }

    /// Uses `preferences`, saved to `path` whenever they change.
    pub fn with_preferences(mut self, preferences: Preferences, path: Option<PathBuf>) -> Self {
        self.preferences = preferences;
        // Presets live next to the preferences.
        self.presets = PresetStore::new(
            path.as_ref()
                .and_then(|path| path.parent())
                .map(|folder| folder.join("Préréglages")),
        );
        self.preferences_path = path;
        self
    }

    /// Selects a slot of the grid: its layer and clip show in the
    /// inspector.
    pub fn select(&mut self, slot: Option<Slot>) {
        self.selected = slot;
    }

    /// Turns the MIDI learn mode on or off.
    pub fn set_midi_learn(&mut self, on: bool) {
        self.learn.active = on;
        self.learn.selected = None;
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

    /// Remembers the screens showing the output, for next time.
    pub fn remember_output_screens(&mut self, names: &[String]) {
        if self.preferences.output_screens != names {
            self.preferences.output_screens = names.to_vec();
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
            Shortcut::TapTempo => self.tap_tempo(),
            Shortcut::Resync => self.resync(),
        }
        self.document.commit(&before);
    }

    /// One tap of the tap tempo. From the second tap of a series on, the
    /// tempo follows the taps; every tap puts a beat on itself.
    pub(crate) fn tap_tempo(&mut self) {
        let now = Instant::now();
        let bpm = self
            .tap
            .tap(now)
            .map_or(self.document.composition().tempo, Bpm::new);
        self.document.composition_mut().tempo = bpm;
        self.actions.push(Action::Engine(Command::Tap {
            at: now,
            bpm: bpm.get(),
        }));
    }

    /// Starts a bar now.
    pub(crate) fn resync(&mut self) {
        self.actions
            .push(Action::Engine(Command::Resync { at: Instant::now() }));
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

    /// Applies messages of controllers: through the mappings, or, in learn
    /// mode with a control picked, to map the first control moved.
    pub fn control(&mut self, messages: impl IntoIterator<Item = ControlMessage>) -> Vec<Action> {
        let before = self.document.composition().clone();
        for message in messages {
            self.apply_control(message);
        }
        self.document.commit(&before);
        std::mem::take(&mut self.actions)
    }

    /// Applies one message; returns whether it was learned or mapped.
    fn apply_control(&mut self, message: ControlMessage) -> bool {
        // A released note or key does not learn; a fader or an OSC message
        // does, whatever its value.
        let moved = message.pressed
            || !matches!(
                message.source,
                prism_core::ControlSource::MidiNote { .. } | prism_core::ControlSource::Key { .. }
            );
        if self.learn.active
            && moved
            && let Some(target) = self.learn.selected.take()
        {
            prism_control::learn(self.document.composition_mut(), message.source, target);
            return true;
        }
        let mapped = self
            .document
            .composition()
            .mappings
            .iter()
            .any(|mapping| mapping.source == message.source);
        let outcomes = self
            .controls
            .apply(self.document.composition_mut(), &message);
        for outcome in outcomes {
            match outcome {
                Outcome::Changed => {}
                Outcome::Tap => self.tap_tempo(),
                Outcome::Resync => self.resync(),
            }
        }
        mapped
    }

    /// A key pressed over another window (the output): when it is learned
    /// or mapped, it acts and the actions to do are returned.
    pub fn press_key(&mut self, key: &str) -> Option<Vec<Action>> {
        let before = self.document.composition().clone();
        let acted = self.key(key, true);
        self.document.commit(&before);
        acted.then(|| std::mem::take(&mut self.actions))
    }

    /// A key went down or up: learned or mapped keys act; returns whether
    /// the key did.
    fn key(&mut self, key: &str, pressed: bool) -> bool {
        self.apply_control(ControlMessage {
            source: prism_core::ControlSource::Key {
                key: key.to_owned(),
            },
            value: f32::from(u8::from(pressed)),
            pressed,
        })
    }

    /// Draws one frame of the interface in `ui`, the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui, frame: &FrameInfo<'_>) -> Vec<Action> {
        let before = self.document.composition().clone();
        self.learn.refresh(&self.document.composition().mappings);
        self.keyboard(ui.ctx());
        self.refresh_missing();

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui, frame));
        egui::Panel::bottom("transport")
            .resizable(false)
            .show(ui, |ui| self.transport(ui, frame.status, &frame.audio));
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
        let keys: Vec<(Key, bool)> = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed,
                        repeat: false,
                        modifiers,
                        ..
                    } if !modifiers.command && !modifiers.alt && !modifiers.ctrl => {
                        Some((*key, *pressed))
                    }
                    _ => None,
                })
                .collect()
        });
        for (key, pressed) in keys {
            // Learned keys go first; the others keep their usual action.
            if self.key(key.name(), pressed) || !pressed {
                continue;
            }
            let shortcut = match key {
                Key::Space => Some(Shortcut::StopAll),
                Key::Escape => Some(Shortcut::CloseOutput),
                Key::F => Some(Shortcut::ToggleOutput),
                Key::T => Some(Shortcut::TapTempo),
                Key::R => Some(Shortcut::Resync),
                other => digit(other).map(Shortcut::TriggerColumn),
            };
            if let Some(shortcut) = shortcut {
                self.shortcut(shortcut);
            }
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
                    let mut open = frame.output_screens.contains(&index);
                    if ui.checkbox(&mut open, label).clicked() {
                        self.actions.push(Action::ToggleScreen(index));
                    }
                }
                ui.separator();
                let label = if frame.output_screens.len() > 1 {
                    "Fermer les sorties"
                } else {
                    "Fermer la sortie"
                };
                if ui
                    .add_enabled(
                        !frame.output_screens.is_empty(),
                        egui::Button::new(label).shortcut_text("Échap"),
                    )
                    .clicked()
                {
                    self.actions.push(Action::CloseOutput);
                }
                if cfg!(target_os = "macos") {
                    ui.separator();
                    let mut syphon = self.preferences.syphon_output;
                    if ui
                        .checkbox(&mut syphon, "Publier sur Syphon")
                        .on_hover_text("L'image programme, sous le nom « Prism », pour les autres applications du Mac")
                        .changed()
                    {
                        self.preferences.syphon_output = syphon;
                        self.save_preferences();
                    }
                }
                ui.separator();
                let status = prism_engine::ndi_status();
                let mut ndi = self.preferences.ndi_output && status.is_ok();
                let hover = match &status {
                    Ok(version) => format!(
                        "L'image programme, sous le nom « Prism », pour les machines du réseau\nBibliothèque : {version}\nNDI® est une marque déposée de Vizrt NDI AB."
                    ),
                    Err(reason) => format!("Indisponible : {reason}"),
                };
                if ui
                    .add_enabled(status.is_ok(), egui::Checkbox::new(&mut ndi, "Publier en NDI®"))
                    .on_hover_text(&hover)
                    .on_disabled_hover_text(&hover)
                    .changed()
                {
                    self.preferences.ndi_output = ndi;
                    self.save_preferences();
                }
            });
            ui.menu_button("Contrôle", |ui| self.control_menu(ui, &frame.midi));
            let audio = ui.menu_button("Audio", |ui| self.audio_menu(ui, &frame.audio));
            if audio.inner.is_none() {
                // Read the devices again next time the menu opens.
                self.audio_devices = None;
            }
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

    /// MIDI learn, mappings, MIDI inputs.
    fn control_menu(&mut self, ui: &mut egui::Ui, midi: &MidiStatus) {
        if ui
            .checkbox(&mut self.learn.active, "Apprentissage MIDI")
            .on_hover_text("Cliquer une commande, puis bouger un bouton ou un fader du contrôleur")
            .changed()
        {
            self.learn.selected = None;
        }
        if ui.button("Correspondances…").clicked() {
            self.dialogs.mappings = true;
            ui.close();
        }
        ui.separator();
        ui.label(egui::RichText::new("Entrées MIDI").strong());
        if midi.ports.is_empty() {
            ui.label(egui::RichText::new("Aucune entrée MIDI").weak());
        }
        let mut preferences = self.preferences.clone();
        for port in &midi.ports {
            let mut on = !preferences.midi_disabled.contains(port);
            if ui.checkbox(&mut on, port).changed() {
                if on {
                    preferences.midi_disabled.retain(|name| name != port);
                } else {
                    preferences.midi_disabled.push(port.clone());
                }
            }
        }
        for error in &midi.errors {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.checkbox(
            &mut preferences.midi_feedback,
            "Renvoyer l'état au contrôleur",
        )
        .on_hover_text("Allume les pads des clips qui jouent, place les faders motorisés");
        ui.separator();
        ui.label(egui::RichText::new("OSC").strong());
        ui.horizontal(|ui| {
            ui.checkbox(&mut preferences.osc_input, "Écouter sur le port");
            ui.add(egui::DragValue::new(&mut preferences.osc_port).range(1024..=65535));
        })
        .response
        .on_hover_text("Adresses /prism/… (voir l'aide) ou apprises comme une commande MIDI");
        ui.horizontal(|ui| {
            ui.checkbox(&mut preferences.osc_output, "Envoyer vers");
            ui.add(egui::TextEdit::singleline(&mut preferences.osc_target).desired_width(140.0))
                .on_hover_text("IP:port de la tablette ou du logiciel qui reçoit");
        });
        if let Some(port) = midi.osc_port {
            let addresses = local_addresses();
            ui.label(egui::RichText::new(format!("À l'écoute : {addresses} port {port}")).weak());
        }
        for error in &midi.osc_errors {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if preferences != self.preferences {
            self.preferences = preferences;
            self.save_preferences();
        }
    }

    /// Audio input: on or off, device, sensitivity.
    fn audio_menu(&mut self, ui: &mut egui::Ui, status: &AudioStatus) {
        let devices = self
            .audio_devices
            .get_or_insert_with(prism_audio::input_devices)
            .clone();
        let mut preferences = self.preferences.clone();
        ui.checkbox(&mut preferences.clip_sound, "Son des clips")
            .on_hover_text("Jouer le son des vidéos sur la sortie audio du système");
        ui.add_enabled_ui(preferences.clip_sound, |ui| {
            ui.horizontal(|ui| {
                ui.label("Volume");
                ui.add(egui::Slider::new(
                    &mut preferences.clip_sound_volume,
                    0.0..=1.0,
                ));
            });
        });
        match (&status.output, &status.output_error) {
            (_, Some(error)) => {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            (Some(device), None) if preferences.clip_sound => {
                ui.label(egui::RichText::new(format!("Sortie : {device}")).weak());
            }
            _ => {}
        }
        ui.separator();
        ui.checkbox(&mut preferences.audio_input, "Écouter une entrée audio")
            .on_hover_text(
                "Les réglages reliés à une bande (graves, médiums, aigus, niveau) suivent le son",
            );
        ui.add_enabled_ui(preferences.audio_input, |ui| {
            ui.separator();
            ui.radio_value(
                &mut preferences.audio_device,
                None,
                "Entrée par défaut du système",
            );
            for device in devices {
                ui.radio_value(&mut preferences.audio_device, Some(device.clone()), device);
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Sensibilité");
                ui.add(
                    egui::Slider::new(
                        &mut preferences.audio_gain_db,
                        0.0..=prism_audio::MAX_GAIN_DB,
                    )
                    .suffix(" dB"),
                );
            });
        });
        match (&status.device, &status.error) {
            (_, Some(error)) => {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            (Some(device), None) => {
                ui.label(egui::RichText::new(format!("À l'écoute : {device}")).weak());
            }
            (None, None) => {}
        }
        if preferences != self.preferences {
            self.preferences = preferences;
            self.save_preferences();
        }
    }

    /// Tempo, tap tempo, resync and the beat of the bar.
    fn tempo(&mut self, ui: &mut egui::Ui, status: &EngineStatus) {
        let mut bpm = self.document.composition().tempo.get();
        let field = ui
            .add(
                egui::DragValue::new(&mut bpm)
                    .range(MIN_BPM..=MAX_BPM)
                    .speed(0.1)
                    .fixed_decimals(1)
                    .suffix(" BPM"),
            )
            .on_hover_text("Tempo de la composition");
        if field.changed() {
            self.document.composition_mut().tempo = Bpm::new(bpm);
        }
        // A tap counts when the button goes down, not when it comes up.
        let tap = ui
            .button("Tap")
            .on_hover_text("Tap tempo : frapper en rythme (touche T)");
        if !self.learn.active
            && tap.contains_pointer()
            && ui.input(|input| input.pointer.primary_pressed())
        {
            self.tap_tempo();
        }
        self.learn
            .cover(ui, tap.rect, prism_core::ControlTarget::Tap);
        let resync = ui
            .button("Resync")
            .on_hover_text("La mesure commence maintenant (touche R)");
        if resync.clicked() {
            self.resync();
        }
        self.learn
            .cover(ui, resync.rect, prism_core::ControlTarget::Resync);
        beat_lights(ui, status.beat);
    }

    /// Levels of the bands of the sound, while an input is measured.
    fn audio_meter(&self, ui: &mut egui::Ui, audio: &AudioStatus) {
        if audio.device.is_some() {
            audio_meter(ui, audio.levels);
        }
    }

    /// Duration of the automatic crossfade, as the interface says it.
    fn crossfade_text(&self, seconds: f32) -> String {
        if self.preferences.crossfade_on_beats {
            format!(
                "{} temps ({seconds:.1} s)",
                self.preferences.crossfade_beats
            )
        } else {
            format!("{seconds:.1} s")
        }
    }

    /// Duration of the automatic crossfade, in seconds or in beats.
    fn crossfade_duration(&mut self, ui: &mut egui::Ui) {
        let preferences = &mut self.preferences;
        let field = if preferences.crossfade_on_beats {
            ui.add(
                egui::DragValue::new(&mut preferences.crossfade_beats)
                    .range(1.0..=64.0)
                    .speed(0.1)
                    .fixed_decimals(0)
                    .suffix(" temps"),
            )
        } else {
            ui.add(
                egui::DragValue::new(&mut preferences.crossfade_seconds)
                    .range(0.0..=60.0)
                    .speed(0.05)
                    .suffix(" s"),
            )
        }
        .on_hover_text("Durée du fondu automatique");
        let unit = ui
            .selectable_label(preferences.crossfade_on_beats, "au tempo")
            .on_hover_text("Compter le fondu automatique en temps, au tempo de la composition");
        if unit.clicked() {
            preferences.crossfade_on_beats = !preferences.crossfade_on_beats;
        }
        if field.drag_stopped() || field.lost_focus() || unit.clicked() {
            self.save_preferences();
        }
    }

    fn transport(&mut self, ui: &mut egui::Ui, status: &EngineStatus, audio: &AudioStatus) {
        let dt = ui.input(|input| input.stable_dt).min(0.1);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            self.tempo(ui, status);
            self.audio_meter(ui, audio);
            ui.separator();
            let bpm = self.document.composition().tempo.get();
            let seconds = self.preferences.crossfade_duration(bpm);
            let duration = self.crossfade_text(seconds);
            let start = self.document.composition().crossfader.position.get();
            let mut fade_to = None;
            if ui
                .button("◀ A")
                .on_hover_text(format!("Crossfader vers A en {duration}"))
                .clicked()
            {
                fade_to = Some(0.0);
            }
            ui.label(egui::RichText::new("A").strong());
            let crossfader = &mut self.document.composition_mut().crossfader;
            let mut position = crossfader.position.get();
            // A slider takes its length from the spacing, not from its size.
            ui.spacing_mut().slider_width = 320.0;
            let slider = egui::Slider::new(&mut position, 0.0..=1.0).show_value(false);
            let response = ui
                .add(slider)
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
            self.learn
                .cover(ui, response.rect, prism_core::ControlTarget::Crossfader);
            ui.label(egui::RichText::new("B").strong());
            if ui
                .button("B ▶")
                .on_hover_text(format!("Crossfader vers B en {duration}"))
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
            self.crossfade_duration(ui);
            if let Some(target) = fade_to {
                self.auto_fade = Some(AutoFade::new(start, target, seconds));
            }
        });
        // On its own line, so the controls above never push it out.
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(status_line(status))
                    .small()
                    .color(ui.visuals().weak_text_color()),
            )
            .on_hover_text(STATUS_HELP);
        });
        ui.add_space(2.0);
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

/// Addresses of this computer on the local network, to point a tablet at.
fn local_addresses() -> String {
    // The address used to reach outside, without sending anything.
    std::net::UdpSocket::bind(("0.0.0.0", 0))
        .and_then(|socket| {
            socket.connect(("192.0.2.1", 9))?;
            socket.local_addr()
        })
        .map_or_else(
            |_| "cet ordinateur".to_owned(),
            |address| address.ip().to_string(),
        )
}

/// One bar per band of the sound, from 0 to 1.
fn audio_meter(ui: &mut egui::Ui, levels: [f32; BAND_COUNT]) {
    let (width, gap, height) = (5.0, 2.0, 14.0);
    let size = egui::vec2((width + gap) * BAND_COUNT as f32, height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let visuals = ui.visuals();
    for (index, level) in levels.iter().enumerate() {
        let left = rect.left() + (width + gap) * index as f32;
        let bar = egui::Rect::from_min_max(
            egui::pos2(left, rect.top()),
            egui::pos2(left + width, rect.bottom()),
        );
        ui.painter()
            .rect_filled(bar, 1.0, visuals.widgets.inactive.bg_fill);
        let filled = egui::Rect::from_min_max(
            egui::pos2(left, rect.bottom() - height * level.clamp(0.0, 1.0)),
            bar.max,
        );
        ui.painter()
            .rect_filled(filled, 1.0, visuals.selection.bg_fill);
    }
    response.on_hover_text(
        Band::ALL
            .iter()
            .zip(levels)
            .map(|(band, level)| {
                format!(
                    "{} : {:.0} %",
                    crate::effects::band_label(*band),
                    level * 100.0
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// One light per beat of the bar: the current beat is lit, the first beat
/// of the bar in the warning color.
fn beat_lights(ui: &mut egui::Ui, beat: f64) {
    let spacing = 10.0;
    let size = egui::vec2(spacing * BEATS_PER_BAR as f32, 12.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let current = beat_in_bar(beat);
    let visuals = ui.visuals();
    for index in 0..BEATS_PER_BAR {
        let center = egui::pos2(
            rect.left() + spacing * (index as f32 + 0.5),
            rect.center().y,
        );
        let color = match (index == current, index) {
            (true, 0) => visuals.warn_fg_color,
            (true, _) => visuals.strong_text_color(),
            (false, _) => visuals.widgets.inactive.bg_fill,
        };
        ui.painter().circle_filled(center, 3.5, color);
    }
    response.on_hover_text("Temps de la mesure");
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::{ControlSource, ControlTarget};
    use prism_engine::LayerStatus;

    #[test]
    fn learned_controls_drive_the_composition() {
        let mut panel = ControlPanel::new(
            Document::new(prism_core::Composition::default()),
            Vec::new(),
        );
        let layer = panel.document.composition().layers()[0].id();
        let fader = |value: f32| ControlMessage {
            source: ControlSource::MidiControl {
                channel: 0,
                number: 7,
            },
            value,
            pressed: value >= 0.5,
        };
        let pad = |pressed| ControlMessage {
            source: ControlSource::MidiNote {
                channel: 9,
                note: 36,
            },
            value: 1.0,
            pressed,
        };
        // Learn: the first control moved is mapped to the control picked.
        panel.learn.active = true;
        panel.learn.selected = Some(ControlTarget::LayerOpacity { layer });
        panel.control([fader(0.3)]);
        assert_eq!(panel.learn.selected, None);
        assert_eq!(panel.document.composition().mappings.len(), 1);
        panel.learn.selected = Some(ControlTarget::Tap);
        // A note off does not learn.
        panel.control([pad(false)]);
        assert_eq!(panel.document.composition().mappings.len(), 1);
        panel.control([pad(true)]);
        panel.learn.active = false;

        // Played: the fader moves the opacity, the pad taps the tempo.
        let revision = panel.document.revision();
        panel.control([fader(0.25)]);
        assert_eq!(
            panel
                .document
                .composition()
                .layer(layer)
                .unwrap()
                .opacity
                .get(),
            0.25
        );
        assert!(panel.document.revision() > revision);
        let actions = panel.control([pad(true)]);
        assert!(matches!(
            actions.as_slice(),
            [Action::Engine(Command::Tap { .. })]
        ));
    }

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
