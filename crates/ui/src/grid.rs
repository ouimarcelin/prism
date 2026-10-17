//! The clip grid: one row per layer, top layer first, one cell per column.

use egui::{Color32, RichText, Sense, Stroke, Vec2};
use prism_core::{ColumnId, ControlTarget, CrossfaderSide, LayerId, Slot, Unit};
use prism_engine::{ClipState, EngineStatus};

use crate::labels;
use crate::panel::ControlPanel;

const HEADER_WIDTH: f32 = 210.0;
const CELL_SIZE: Vec2 = Vec2::new(116.0, 64.0);
const PLAYING: Color32 = Color32::from_rgb(46, 125, 50);
const MISSING: Color32 = Color32::from_rgb(220, 80, 60);

impl ControlPanel {
    pub(crate) fn grid(&mut self, ui: &mut egui::Ui, status: &EngineStatus) {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.column_headers(ui);
                let layers: Vec<LayerId> = self
                    .document
                    .composition()
                    .layers()
                    .iter()
                    .rev()
                    .map(|layer| layer.id())
                    .collect();
                for layer in layers {
                    self.layer_row(ui, layer, status);
                }
                ui.add_space(6.0);
                if ui.button("+ Ajouter une couche").clicked() {
                    let number = self.document.composition().layers().len() + 1;
                    self.document
                        .composition_mut()
                        .add_layer(format!("Couche {number}"));
                }
            });
    }

    fn column_headers(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_sized(
                [HEADER_WIDTH, 24.0],
                egui::Label::new(RichText::new("Couches / colonnes").weak()),
            );
            let columns: Vec<(ColumnId, String)> = self
                .document
                .composition()
                .columns()
                .iter()
                .map(|column| (column.id(), column.name.clone()))
                .collect();
            for (index, (column, name)) in columns.into_iter().enumerate() {
                let label = if index < 9 {
                    format!("{}  {name}", index + 1)
                } else {
                    name
                };
                let response = ui
                    .add_sized([CELL_SIZE.x, 24.0], egui::Button::new(label).truncate())
                    .on_hover_text("Déclencher toute la colonne");
                if response.clicked() {
                    self.trigger_column(column);
                }
                self.learn
                    .cover(ui, response.rect, ControlTarget::Column { column });
                response.context_menu(|ui| self.column_menu(ui, column));
            }
            if ui
                .add_sized([28.0, 24.0], egui::Button::new("+"))
                .on_hover_text("Ajouter une colonne")
                .clicked()
            {
                let number = self.document.composition().columns().len() + 1;
                self.document
                    .composition_mut()
                    .add_column(format!("Colonne {number}"));
            }
        });
    }

    fn column_menu(&mut self, ui: &mut egui::Ui, column: ColumnId) {
        if let Some(entry) = self.document.composition_mut().column_mut(column) {
            ui.label("Nom");
            ui.text_edit_singleline(&mut entry.name);
        }
        ui.separator();
        let last = self.document.composition().columns().len() <= 1;
        if ui
            .add_enabled(!last, egui::Button::new("Supprimer la colonne"))
            .clicked()
        {
            let _ = self.document.composition_mut().remove_column(column);
            if self.selected.is_some_and(|slot| slot.column == column) {
                self.selected = None;
            }
            ui.close();
        }
    }

    fn layer_row(&mut self, ui: &mut egui::Ui, layer: LayerId, status: &EngineStatus) {
        let layer_status = status.layers.iter().find(|s| s.layer == Some(layer));
        ui.horizontal(|ui| {
            ui.allocate_ui(Vec2::new(HEADER_WIDTH, CELL_SIZE.y), |ui| {
                ui.set_min_size(Vec2::new(HEADER_WIDTH, CELL_SIZE.y));
                self.layer_header(ui, layer, layer_status.map(|s| &s.state));
            });
            let columns: Vec<ColumnId> = self
                .document
                .composition()
                .columns()
                .iter()
                .map(|column| column.id())
                .collect();
            for column in columns {
                let slot = Slot::new(layer, column);
                let progress = layer_status
                    .filter(|s| s.slot == Some(slot) && s.duration > 0.0)
                    .map(|s| (s.position / s.duration) as f32);
                self.clip_cell(ui, slot, progress);
            }
        });
    }

    fn layer_header(&mut self, ui: &mut egui::Ui, layer: LayerId, state: Option<&ClipState>) {
        let selected_layer = self.selected.is_some_and(|slot| slot.layer == layer);
        let removable = self.document.composition().layers().len() > 1;
        let (mut stop, mut remove) = (false, false);
        let Some(entry) = self.document.composition_mut().layer_mut(layer) else {
            return;
        };
        let mut covers = Vec::new();
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let name = RichText::new(entry.name.clone()).strong();
                let name = if selected_layer {
                    name.underline()
                } else {
                    name
                };
                let label = ui.add(egui::Label::new(name).truncate().sense(Sense::click()));
                covers.push((label.rect, ControlTarget::LayerStop { layer }));
                label
                    .on_hover_text("Clic droit : renommer, arrêter, supprimer")
                    .context_menu(|ui| {
                        ui.label("Nom");
                        ui.text_edit_singleline(&mut entry.name);
                        ui.separator();
                        if ui.button("Arrêter la couche").clicked() {
                            stop = true;
                            ui.close();
                        }
                        if ui
                            .add_enabled(removable, egui::Button::new("Supprimer la couche"))
                            .clicked()
                        {
                            remove = true;
                            ui.close();
                        }
                    });
                if let Some(ClipState::Error(message)) = state {
                    ui.label(RichText::new("⚠").color(MISSING))
                        .on_hover_text(message.as_str());
                }
            });
            ui.horizontal(|ui| {
                let bypass = ui
                    .toggle_value(&mut entry.bypass, "B")
                    .on_hover_text("Bypass : couche masquée");
                covers.push((bypass.rect, ControlTarget::LayerBypass { layer }));
                let solo = ui
                    .toggle_value(&mut entry.solo, "S")
                    .on_hover_text("Solo : seules les couches en solo sont visibles");
                covers.push((solo.rect, ControlTarget::LayerSolo { layer }));
                egui::ComboBox::from_id_salt(("side", layer))
                    .width(34.0)
                    .selected_text(labels::side(entry.crossfader_side))
                    .show_ui(ui, |ui| {
                        for side in [None, Some(CrossfaderSide::A), Some(CrossfaderSide::B)] {
                            ui.selectable_value(
                                &mut entry.crossfader_side,
                                side,
                                labels::side(side),
                            );
                        }
                    })
                    .response
                    .on_hover_text("Côté du crossfader");
                egui::ComboBox::from_id_salt(("blend", layer))
                    .width(86.0)
                    .selected_text(labels::blend(entry.blend_mode))
                    .show_ui(ui, |ui| {
                        for mode in prism_core::BlendMode::ALL {
                            ui.selectable_value(&mut entry.blend_mode, mode, labels::blend(mode));
                        }
                    })
                    .response
                    .on_hover_text("Mode de fusion");
            });
            let mut opacity = entry.opacity.get();
            let slider = ui
                .add(egui::Slider::new(&mut opacity, 0.0..=1.0).show_value(false))
                .on_hover_text(format!("Opacité {:.0} %", opacity * 100.0));
            covers.push((slider.rect, ControlTarget::LayerOpacity { layer }));
            entry.opacity = Unit::new(opacity);
            for (rect, target) in covers.drain(..) {
                self.learn.cover(ui, rect, target);
            }
        });
        if stop {
            let _ = self.document.composition_mut().stop_layer(layer);
        }
        if remove {
            let _ = self.document.composition_mut().remove_layer(layer);
            if self.selected.is_some_and(|slot| slot.layer == layer) {
                self.selected = None;
            }
        }
    }

    fn clip_cell(&mut self, ui: &mut egui::Ui, slot: Slot, progress: Option<f32>) {
        let composition = self.document.composition();
        let clip_name = composition.clip(slot).map(|clip| clip.name.clone());
        let playing = composition
            .layer(slot.layer)
            .is_some_and(|layer| layer.active_column() == Some(slot.column));
        let selected = self.selected == Some(slot);
        let missing = self.is_missing(slot);

        let text = match &clip_name {
            Some(name) if missing => RichText::new(name).color(MISSING),
            Some(name) => RichText::new(name),
            None => RichText::new("").weak(),
        };
        let mut button = egui::Button::new(text)
            .truncate()
            .min_size(CELL_SIZE)
            .selected(playing);
        if playing {
            button = button.fill(PLAYING);
        }
        if selected {
            button = button.stroke(Stroke::new(2.0, ui.visuals().selection.stroke.color));
        }
        let response = ui.add_sized(CELL_SIZE, button);
        if let Some(progress) = progress {
            let rect = response.rect.shrink(4.0);
            let bar = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.bottom() - 3.0),
                Vec2::new(rect.width() * progress.clamp(0.0, 1.0), 3.0),
            );
            ui.painter().rect_filled(bar, 1.0, Color32::WHITE);
        }

        if clip_name.is_some() {
            self.learn
                .cover(ui, response.rect, ControlTarget::Clip { slot });
        }
        if response.clicked() {
            self.selected = Some(slot);
            if clip_name.is_some() {
                let _ = self.document.composition_mut().trigger_clip(slot);
            }
        }
        if response.double_clicked() && clip_name.is_none() {
            self.import_dialog(Some(slot));
        }
        let response = match &clip_name {
            Some(name) if missing => {
                response.on_hover_text(format!("{name} : fichier introuvable"))
            }
            Some(name) => response.on_hover_text(name.as_str()),
            None => response.on_hover_text("Double-clic pour importer des médias"),
        };
        response.context_menu(|ui| {
            if ui.button("Importer des médias ici…").clicked() {
                ui.close();
                self.import_dialog(Some(slot));
            }
            if ui.button("Importer une séquence d'images ici…").clicked() {
                ui.close();
                self.import_sequence_dialog(Some(slot));
            }
            if clip_name.is_some() {
                if ui.button("Localiser le fichier…").clicked() {
                    ui.close();
                    self.relocate_dialog(slot);
                }
                if ui.button("Retirer le clip").clicked() {
                    let _ = self.document.composition_mut().set_clip(slot, None);
                    ui.close();
                }
            }
        });
    }
}
