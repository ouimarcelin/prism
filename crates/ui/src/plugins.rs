//! The plugin folder, read again every few seconds while its menus show,
//! and the items those menus offer.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::RichText;
use prism_fx::Plugin;

/// How long a listing of the folder is used before it is read again.
const RESCAN: Duration = Duration::from_secs(2);

/// The ISF files of the plugin folder.
#[derive(Debug, Default)]
pub(crate) struct PluginShelf {
    folder: Option<PathBuf>,
    plugins: Vec<Plugin>,
    scanned: Option<Instant>,
    /// Why the folder could not be opened.
    error: Option<String>,
}

impl PluginShelf {
    pub(crate) fn new(folder: Option<PathBuf>) -> Self {
        Self {
            folder,
            ..Self::default()
        }
    }

    /// The plugins, the folder read again when the listing is old.
    fn plugins(&mut self) -> &[Plugin] {
        if self
            .scanned
            .is_none_or(|scanned| scanned.elapsed() >= RESCAN)
        {
            self.plugins = self
                .folder
                .as_deref()
                .map(prism_fx::scan_plugins)
                .unwrap_or_default();
            self.scanned = Some(Instant::now());
        }
        &self.plugins
    }

    /// Items of the plugins that are effects (`filter`) or generators, then
    /// the item opening the folder. Returns the file of the plugin picked.
    pub(crate) fn menu_items(&mut self, ui: &mut egui::Ui, filter: bool) -> Option<PathBuf> {
        let mut picked = None;
        let plugins: Vec<Plugin> = self
            .plugins()
            .iter()
            .filter(|plugin| plugin.filter == filter || plugin.error.is_some())
            .cloned()
            .collect();
        ui.label(RichText::new("Plugins").weak());
        if plugins.is_empty() {
            ui.label(
                RichText::new("Aucun : déposer des fichiers ISF (.fs) dans le dossier").weak(),
            );
        }
        for plugin in &plugins {
            let response = ui.add_enabled(
                plugin.error.is_none(),
                egui::Button::selectable(false, &plugin.name),
            );
            let response = match (&plugin.error, &plugin.description) {
                (Some(error), _) => response.on_disabled_hover_text(error),
                (None, Some(description)) => response.on_hover_text(description),
                (None, None) => response,
            };
            if response.clicked() {
                picked = Some(plugin.path.clone());
            }
        }
        if let Some(folder) = self.folder.clone()
            && ui
                .selectable_label(false, "Ouvrir le dossier des plugins")
                .on_hover_text(folder.display().to_string())
                .clicked()
        {
            self.error = open_folder(&folder).err();
            self.scanned = None;
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        picked
    }
}

/// Creates `folder` if needed and shows it in the file manager.
fn open_folder(folder: &Path) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(folder)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Dossier des plugins : {error}"))
}
