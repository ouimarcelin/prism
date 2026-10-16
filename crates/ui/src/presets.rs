//! Presets of effect chains and layers, kept in a folder next to the
//! preferences, and the menus to use and save them.

use std::path::PathBuf;

use egui::RichText;
use prism_core::{Effect, Layer, LayerPreset, Preset, PresetContent};

/// The presets on disk, read at start and after each save.
#[derive(Debug, Default)]
pub(crate) struct PresetStore {
    /// Folder of the presets; without one, nothing can be saved.
    folder: Option<PathBuf>,
    effects: Vec<Preset>,
    layers: Vec<Preset>,
    /// Why the last save failed.
    error: Option<String>,
}

impl PresetStore {
    /// The presets of `folder`.
    pub(crate) fn new(folder: Option<PathBuf>) -> Self {
        let mut store = Self {
            folder,
            ..Self::default()
        };
        store.reload();
        store
    }

    fn effects_folder(&self) -> Option<PathBuf> {
        self.folder.as_ref().map(|folder| folder.join("Effets"))
    }

    fn layers_folder(&self) -> Option<PathBuf> {
        self.folder.as_ref().map(|folder| folder.join("Couches"))
    }

    /// Reads the folders again.
    pub(crate) fn reload(&mut self) {
        self.effects = self
            .effects_folder()
            .map(|folder| Preset::list(&folder))
            .unwrap_or_default();
        self.layers = self
            .layers_folder()
            .map(|folder| Preset::list(&folder))
            .unwrap_or_default();
    }

    fn save(&mut self, preset: Preset) {
        let folder = match &preset.content {
            PresetContent::Effects { .. } => self.effects_folder(),
            PresetContent::Layer(_) => self.layers_folder(),
        };
        self.error = match folder {
            Some(folder) => preset
                .save(&folder)
                .err()
                .map(|error| crate::describe(&error)),
            None => Some("aucun dossier de préréglages".into()),
        };
        self.reload();
    }

    /// Menu of the presets of effect chains: add one to `effects`, or save
    /// `effects` as a new one.
    pub(crate) fn effects_menu(
        &mut self,
        ui: &mut egui::Ui,
        id: egui::Id,
        effects: &mut Vec<Effect>,
    ) {
        ui.menu_button("Préréglages…", |ui| {
            if self.effects.is_empty() {
                ui.label(RichText::new("Aucun préréglage d'effets").weak());
            }
            for preset in &self.effects {
                if let PresetContent::Effects { effects: saved } = &preset.content
                    && ui
                        .button(&preset.name)
                        .on_hover_text("Ajouter ces effets à la fin de la chaîne")
                        .clicked()
                {
                    effects.extend(saved.iter().cloned());
                    ui.close();
                }
            }
            ui.separator();
            if let Some(name) =
                self.name_field(ui, id, !effects.is_empty(), "Enregistrer la chaîne")
            {
                self.save(Preset {
                    name,
                    content: PresetContent::Effects {
                        effects: effects.clone(),
                    },
                });
            }
        });
    }

    /// Menu of the presets of layers: give one to `layer`, or save the
    /// settings of `layer` as a new one.
    pub(crate) fn layer_menu(&mut self, ui: &mut egui::Ui, id: egui::Id, layer: &mut Layer) {
        ui.menu_button("Préréglages…", |ui| {
            if self.layers.is_empty() {
                ui.label(RichText::new("Aucun préréglage de couche").weak());
            }
            for preset in &self.layers {
                if let PresetContent::Layer(settings) = &preset.content
                    && ui
                        .button(&preset.name)
                        .on_hover_text(
                            "Remplacer les réglages et les effets de la couche (ses clips restent)",
                        )
                        .clicked()
                {
                    settings.apply(layer);
                    ui.close();
                }
            }
            ui.separator();
            if let Some(name) = self.name_field(ui, id, true, "Enregistrer la couche") {
                self.save(Preset {
                    name,
                    content: PresetContent::Layer(LayerPreset::of(layer)),
                });
            }
        });
    }

    /// A name and a button to save under it; returns the name when the
    /// button is pressed.
    fn name_field(
        &mut self,
        ui: &mut egui::Ui,
        id: egui::Id,
        enabled: bool,
        label: &str,
    ) -> Option<String> {
        let id = id.with("preset name");
        let mut name = ui.data_mut(|data| data.get_temp::<String>(id).unwrap_or_default());
        let mut saved = None;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut name)
                    .hint_text("Nom")
                    .desired_width(140.0),
            );
            let ready = enabled && !name.trim().is_empty() && self.folder.is_some();
            if ui.add_enabled(ready, egui::Button::new(label)).clicked() {
                saved = Some(name.trim().to_owned());
                name.clear();
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("Non enregistré : {error}"),
            );
        }
        ui.data_mut(|data| data.insert_temp(id, name));
        saved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_presets_are_listed_by_kind() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PresetStore::new(Some(dir.path().join("Préréglages")));
        assert!(store.effects.is_empty() && store.layers.is_empty());
        store.save(Preset {
            name: "Kaléido".into(),
            content: PresetContent::Effects {
                effects: vec![Effect::new("kaleidoscope")],
            },
        });
        let mut comp = prism_core::Composition::default();
        let layer = comp.layer_mut(comp.layers()[0].id()).unwrap();
        store.save(Preset {
            name: "Couche douce".into(),
            content: PresetContent::Layer(LayerPreset::of(layer)),
        });
        assert_eq!(store.error, None);
        assert_eq!(store.effects.len(), 1);
        assert_eq!(store.layers[0].name, "Couche douce");
        // Read again from disk.
        let again = PresetStore::new(Some(dir.path().join("Préréglages")));
        assert_eq!(again.effects, store.effects);

        // Without a folder: nothing saved, said so.
        let mut nowhere = PresetStore::new(None);
        nowhere.save(store.effects[0].clone());
        assert!(nowhere.error.is_some() && nowhere.effects.is_empty());
    }
}
