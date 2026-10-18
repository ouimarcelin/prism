//! The clip grid: one row per layer, top layer first, one cell per column.

use egui::text::{LayoutJob, TextWrapping};
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, TextStyle, Vec2};
use prism_core::{ColumnId, ControlTarget, CrossfaderSide, LayerId, Slot, Unit};
use prism_engine::{ClipState, EngineStatus};

use crate::labels;
use crate::panel::ControlPanel;
use crate::theme::{self, Emphasis, color};

const HEADER_WIDTH: f32 = 210.0;
const CELL_SIZE: Vec2 = Vec2::new(116.0, 64.0);

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
                ui.add_space(theme::SPACE_2);
                if ui
                    .add(egui::Button::new("+ Ajouter une couche").frame_when_inactive(false))
                    .clicked()
                {
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
                [HEADER_WIDTH, theme::CONTROL_HEIGHT],
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
                let label = RichText::new(label).color(color::TEXT_SECONDARY);
                let response = ui
                    .add_sized(
                        [CELL_SIZE.x, theme::CONTROL_HEIGHT],
                        egui::Button::new(label)
                            .truncate()
                            .frame_when_inactive(false),
                    )
                    .on_hover_text("Déclencher toute la colonne");
                if response.clicked() {
                    self.trigger_column(column);
                }
                self.learn
                    .cover(ui, response.rect, ControlTarget::Column { column });
                response.context_menu(|ui| self.column_menu(ui, column));
            }
            if ui
                .add_sized(
                    [theme::CONTROL_HEIGHT, theme::CONTROL_HEIGHT],
                    egui::Button::new("+").frame_when_inactive(false),
                )
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
                let name = RichText::new(entry.name.clone()).semibold();
                let name = if selected_layer {
                    name.color(color::BRAND_TEXT)
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
                    ui.label(RichText::new("⚠").color(color::DANGER_TEXT))
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
        let clip = composition.clip(slot);
        let clip_name = clip.map(|clip| clip.name.clone());
        let kind = clip.map(|clip| labels::source_kind(&clip.source));
        let playing = composition
            .layer(slot.layer)
            .is_some_and(|layer| layer.active_column() == Some(slot.column));
        let selected = self.selected == Some(slot);
        let missing = self.is_missing(slot);

        let (rect, response) = ui.allocate_exact_size(CELL_SIZE, Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                true,
                playing,
                clip_name.as_deref().unwrap_or("Case vide"),
            )
        });
        if ui.is_rect_visible(rect) {
            let cell = Cell {
                name: clip_name.as_deref(),
                kind,
                playing,
                selected,
                missing,
                hovered: response.hovered(),
                progress,
            };
            cell.paint(ui, rect);
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

/// What a clip cell shows.
struct Cell<'a> {
    name: Option<&'a str>,
    kind: Option<&'static str>,
    playing: bool,
    selected: bool,
    missing: bool,
    hovered: bool,
    progress: Option<f32>,
}

impl Cell<'_> {
    /// Empty cells are outlines on the canvas; clips are cards, green while
    /// they play, with their kind above their name.
    fn paint(&self, ui: &egui::Ui, rect: Rect) {
        let painter = ui.painter();
        let radius = theme::RADIUS;
        let Some(name) = self.name else {
            let fill = if self.hovered {
                color::BG
            } else {
                Color32::TRANSPARENT
            };
            painter.rect(
                rect,
                radius,
                fill,
                Stroke::new(1.0, color::BORDER_SUBTLE),
                StrokeKind::Inside,
            );
            if self.hovered {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "+",
                    TextStyle::Heading.resolve(ui.style()),
                    color::TEXT_TERTIARY,
                );
            }
            self.paint_selection(ui, rect);
            return;
        };

        let fill = match (self.playing, self.hovered) {
            (true, _) => color::SUCCESS,
            (false, true) => color::BG_TERTIARY,
            (false, false) => color::BG_SECONDARY,
        };
        painter.rect_filled(rect, radius, fill);

        let inner = rect.shrink(theme::SPACE_2);
        let (kind_color, name_color) = match (self.missing, self.playing) {
            (true, _) => (color::DANGER_TEXT, color::DANGER_TEXT),
            (false, true) => (color::TEXT, color::TEXT),
            (false, false) => (color::TEXT_TERTIARY, color::TEXT),
        };
        let kind = if self.missing {
            "⚠ Introuvable"
        } else {
            self.kind.unwrap_or_default()
        };
        let small = TextStyle::Small.resolve(ui.style());
        painter.text(inner.left_top(), Align2::LEFT_TOP, kind, small, kind_color);
        let body = TextStyle::Body.resolve(ui.style());
        let galley = painter.layout_job(one_line(name, body, name_color, inner.width()));
        painter.galley(
            inner.left_bottom() - Vec2::new(0.0, galley.size().y),
            galley,
            name_color,
        );

        if let Some(progress) = self.progress {
            let track = Rect::from_min_max(
                egui::pos2(rect.left(), rect.bottom() - 2.0),
                rect.right_bottom(),
            );
            let mut bar = track;
            bar.set_width(track.width() * progress.clamp(0.0, 1.0));
            painter.rect_filled(bar, 0.0, color::TEXT);
        }
        self.paint_selection(ui, rect);
    }

    fn paint_selection(&self, ui: &egui::Ui, rect: Rect) {
        if self.selected {
            ui.painter().rect_stroke(
                rect,
                theme::RADIUS,
                Stroke::new(2.0, color::BRAND),
                StrokeKind::Outside,
            );
        }
    }
}

/// `text` on one line of at most `width` points, cut with an ellipsis.
fn one_line(text: &str, font: FontId, color: Color32, width: f32) -> LayoutJob {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = TextWrapping::truncate_at_width(width);
    job
}
