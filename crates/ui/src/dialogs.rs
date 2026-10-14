//! Windows and confirmations over the control window.

use egui::RichText;
use prism_core::{FrameRate, Resolution};

use crate::panel::ControlPanel;
use crate::shortcuts::SHORTCUT_HELP;

const RESOLUTIONS: [(u32, u32); 5] = [
    (1280, 720),
    (1920, 1080),
    (2560, 1440),
    (3840, 2160),
    (1080, 1920),
];
const FRAME_RATES: [u32; 5] = [24, 25, 30, 50, 60];

impl ControlPanel {
    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        self.about(ctx);
        self.shortcut_help(ctx);
        self.composition_settings(ctx);
        self.missing_media(ctx);
        self.unsaved_changes(ctx);
        self.message(ctx);
    }

    fn about(&mut self, ctx: &egui::Context) {
        egui::Window::new("À propos de Prism")
            .open(&mut self.dialogs.about)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading(format!("Prism {}", env!("CARGO_PKG_VERSION")));
                ui.label("Logiciel libre de VJing et de projection mapping.");
                ui.add_space(6.0);
                ui.label(RichText::new("Projet indépendant, non affilié à Resolume.").strong()); // clean-room-allow
                ui.label("Développé en « clean room », à partir de descriptions de comportement uniquement.");
                ui.add_space(6.0);
                ui.label("Licence MIT ou Apache 2.0, au choix.");
            });
    }

    fn shortcut_help(&mut self, ctx: &egui::Context) {
        egui::Window::new("Raccourcis clavier")
            .open(&mut self.dialogs.shortcuts)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("shortcut help")
                    .num_columns(2)
                    .spacing([16.0, 4.0])
                    .show(ui, |ui| {
                        for (key, action) in SHORTCUT_HELP {
                            ui.label(RichText::new(key).monospace());
                            ui.label(action);
                            ui.end_row();
                        }
                    });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Les raccourcis sans Cmd marchent aussi dans la fenêtre de sortie.",
                    )
                    .weak(),
                );
            });
    }

    fn composition_settings(&mut self, ctx: &egui::Context) {
        let mut open = self.dialogs.composition;
        egui::Window::new("Réglages de la composition")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let composition = self.document.composition_mut();
                egui::Grid::new("composition settings")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Nom");
                        ui.text_edit_singleline(&mut composition.name);
                        ui.end_row();

                        ui.label("Résolution");
                        let current = composition.resolution();
                        ui.horizontal(|ui| {
                            let mut width = current.width;
                            let mut height = current.height;
                            ui.add(
                                egui::DragValue::new(&mut width)
                                    .range(16..=prism_core::MAX_DIMENSION),
                            );
                            ui.label("×");
                            ui.add(
                                egui::DragValue::new(&mut height)
                                    .range(16..=prism_core::MAX_DIMENSION),
                            );
                            egui::ComboBox::from_id_salt("resolution presets")
                                .selected_text("Préréglages")
                                .show_ui(ui, |ui| {
                                    for (w, h) in RESOLUTIONS {
                                        if ui
                                            .selectable_label(false, format!("{w} × {h}"))
                                            .clicked()
                                        {
                                            (width, height) = (w, h);
                                        }
                                    }
                                });
                            let wanted = Resolution { width, height };
                            if wanted != current {
                                let _ = composition.set_resolution(wanted);
                            }
                        });
                        ui.end_row();

                        ui.label("Images par seconde");
                        let current = composition.frame_rate();
                        ui.horizontal(|ui| {
                            for fps in FRAME_RATES {
                                let rate = FrameRate {
                                    numerator: fps,
                                    denominator: 1,
                                };
                                if ui
                                    .selectable_label(current == rate, fps.to_string())
                                    .clicked()
                                {
                                    let _ = composition.set_frame_rate(rate);
                                }
                            }
                        });
                        ui.end_row();
                    });
            });
        self.dialogs.composition = open;
    }

    fn missing_media(&mut self, ctx: &egui::Context) {
        if self.dialogs.missing.is_empty() {
            return;
        }
        let mut open = true;
        let mut search = false;
        let mut relocate = None;
        let composition = self.document.composition();
        egui::Window::new("Médias manquants")
            .open(&mut open)
            .collapsible(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label(format!(
                    "{} clip(s) pointent vers des fichiers introuvables.",
                    self.dialogs.missing.len()
                ));
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for media in &self.dialogs.missing {
                            let place = format!(
                                "{} / {}",
                                composition
                                    .layer(media.slot.layer)
                                    .map_or("?", |l| l.name.as_str()),
                                composition
                                    .column(media.slot.column)
                                    .map_or("?", |c| c.name.as_str()),
                            );
                            ui.horizontal(|ui| {
                                if ui.button("Localiser…").clicked() {
                                    relocate = Some(media.slot);
                                }
                                ui.label(RichText::new(place).strong());
                                ui.label(
                                    RichText::new(media.path.display().to_string())
                                        .small()
                                        .monospace(),
                                );
                            });
                        }
                    });
                ui.add_space(6.0);
                search = ui
                    .button("Chercher dans un dossier…")
                    .on_hover_text(
                        "Retrouve les fichiers par leur nom dans un dossier et ses sous-dossiers",
                    )
                    .clicked();
            });
        if let Some(slot) = relocate {
            self.relocate_dialog(slot);
            self.dialogs.missing = self.document.missing_media();
        }
        if search && let Some(folder) = rfd::FileDialog::new().pick_folder() {
            let found = self.document.relink_missing(&folder);
            self.dialogs.missing = self.document.missing_media();
            self.dialogs.message = Some(format!(
                "{found} fichier(s) retrouvé(s), {} toujours introuvable(s).",
                self.dialogs.missing.len()
            ));
        }
        if !open {
            self.dialogs.missing.clear();
        }
    }

    fn unsaved_changes(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.dialogs.unsaved else {
            return;
        };
        let mut choice = None;
        let response = egui::Modal::new(egui::Id::new("unsaved changes")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Modifications non enregistrées");
            ui.label(format!(
                "Enregistrer les modifications de « {} » ?",
                self.document.display_name()
            ));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Enregistrer").clicked() {
                    choice = Some(true);
                }
                if ui.button("Ne pas enregistrer").clicked() {
                    choice = Some(false);
                }
                if ui.button("Annuler").clicked() {
                    self.dialogs.unsaved = None;
                }
            });
        });
        if response.should_close() && choice.is_none() {
            self.dialogs.unsaved = None;
        }
        if let Some(save) = choice {
            self.dialogs.unsaved = None;
            if !save || self.save_project() {
                self.resume(pending);
            }
        }
    }

    fn message(&mut self, ctx: &egui::Context) {
        let Some(message) = self.dialogs.message.clone() else {
            return;
        };
        let response = egui::Modal::new(egui::Id::new("message")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.label(message);
            ui.add_space(8.0);
            ui.button("OK").clicked()
        });
        if response.inner || response.should_close() {
            self.dialogs.message = None;
        }
    }
}
