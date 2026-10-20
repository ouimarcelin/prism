//! The clip grid: one row per layer, top layer first, one cell per column;
//! a header row above the layers of each group.

use egui::text::{LayoutJob, TextWrapping};
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, TextStyle, Vec2};
use prism_core::{ColumnId, ControlTarget, CrossfaderSide, GroupId, LayerId, Slot, Unit};
use prism_engine::{ClipState, EngineStatus};

use crate::labels;
use crate::panel::ControlPanel;
use crate::slider::Slider;
use crate::theme::{self, Emphasis};

const HEADER_WIDTH: f32 = 210.0;
const CELL_SIZE: Vec2 = Vec2::new(116.0, 64.0);
/// Width of the bar marking the layers of a group, in points.
const GROUP_BAR: f32 = 3.0;
/// Indent of the headers of the layers of a group, in points.
const GROUP_INDENT: f32 = 14.0;

impl ControlPanel {
    pub(crate) fn grid(&mut self, ui: &mut egui::Ui, status: &EngineStatus) {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.column_headers(ui);
                let layers: Vec<(LayerId, Option<GroupId>)> = self
                    .document
                    .composition()
                    .layers()
                    .iter()
                    .rev()
                    .map(|layer| (layer.id(), layer.group()))
                    .collect();
                let mut current = None;
                for (layer, group) in layers {
                    if group != current {
                        current = group;
                        if let Some(group) = group {
                            self.group_row(ui, group);
                        }
                    }
                    let collapsed = group
                        .and_then(|group| self.document.composition().group(group))
                        .is_some_and(|group| group.collapsed);
                    if !collapsed {
                        self.layer_row(ui, layer, status);
                    }
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
                let label = RichText::new(label).color(theme::palette(ui).text_secondary);
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

    /// Header of a group: folding, name, bypass, solo, blend mode and
    /// opacity of the mixed group. A click selects its top layer, which
    /// shows the group in the inspector.
    fn group_row(&mut self, ui: &mut egui::Ui, group: GroupId) {
        let top = self
            .document
            .composition()
            .group_layers(group)
            .last()
            .copied();
        let column = self.selected.map(|slot| slot.column).or_else(|| {
            self.document
                .composition()
                .columns()
                .first()
                .map(|c| c.id())
        });
        let selected_group = self
            .selected
            .and_then(|slot| self.document.composition().layer(slot.layer))
            .is_some_and(|layer| layer.group() == Some(group));
        let mut dissolve = false;
        let mut select = false;
        let Some(entry) = self.document.composition_mut().group_mut(group) else {
            return;
        };
        let palette = theme::palette(ui);
        ui.horizontal(|ui| {
            ui.set_width(HEADER_WIDTH);
            let arrow = if entry.collapsed { "▶" } else { "▼" };
            if ui
                .add(egui::Button::new(arrow).frame_when_inactive(false))
                .on_hover_text(if entry.collapsed {
                    "Montrer les couches du groupe"
                } else {
                    "Replier le groupe"
                })
                .clicked()
            {
                entry.collapsed = !entry.collapsed;
            }
            let name = RichText::new(entry.name.clone()).semibold();
            let name = if selected_group {
                name.color(palette.brand_text)
            } else {
                name
            };
            let label = ui.add(egui::Label::new(name).truncate().sense(Sense::click()));
            select = label.clicked();
            label
                .on_hover_text("Groupe : ses couches sont mélangées ensemble, puis passent par ses effets. Clic droit : renommer, dissoudre")
                .context_menu(|ui| {
                    ui.label("Nom");
                    ui.text_edit_singleline(&mut entry.name);
                    ui.separator();
                    if ui.button("Dissoudre le groupe").clicked() {
                        dissolve = true;
                        ui.close();
                    }
                });
            ui.toggle_value(&mut entry.bypass, "B")
                .on_hover_text("Bypass : groupe masqué");
            ui.toggle_value(&mut entry.solo, "S")
                .on_hover_text("Solo : seuls les groupes et couches en solo sont visibles");
            let mut opacity = entry.opacity.get();
            ui.spacing_mut().slider_width = ui.available_width().max(24.0);
            ui.add(
                Slider::new(&mut opacity, 0.0..=1.0)
                    .show_value(false)
                    .quarters(),
            )
            .on_hover_text(format!("Opacité du groupe {:.0} %", opacity * 100.0));
            entry.opacity = Unit::new(opacity);
        });
        if select && let (Some(layer), Some(column)) = (top, column) {
            self.selected = Some(Slot::new(layer, column));
        }
        if dissolve {
            let _ = self.document.composition_mut().dissolve_group(group);
        }
    }

    fn layer_row(&mut self, ui: &mut egui::Ui, layer: LayerId, status: &EngineStatus) {
        let layer_status = status.layers.iter().find(|s| s.layer == Some(layer));
        let grouped = self
            .document
            .composition()
            .layer(layer)
            .is_some_and(|layer| layer.group().is_some());
        let indent = if grouped { GROUP_INDENT } else { 0.0 };
        let row = ui.horizontal(|ui| {
            let mut width = HEADER_WIDTH;
            if grouped {
                ui.add_space(indent);
                width -= indent + ui.spacing().item_spacing.x;
            }
            ui.allocate_ui(Vec2::new(width, CELL_SIZE.y), |ui| {
                ui.set_min_size(Vec2::new(width, CELL_SIZE.y));
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
        if grouped {
            // A bar along the layers of a group.
            let rect = row.response.rect;
            let bar = Rect::from_min_max(
                rect.left_top() + Vec2::new(4.0, 2.0),
                rect.left_bottom() + Vec2::new(4.0 + GROUP_BAR, -2.0),
            );
            ui.painter()
                .rect_filled(bar, 1.0, theme::palette(ui).brand.gamma_multiply(0.6));
        }
    }

    fn layer_header(&mut self, ui: &mut egui::Ui, layer: LayerId, state: Option<&ClipState>) {
        let selected_layer = self.selected.is_some_and(|slot| slot.layer == layer);
        let composition = self.document.composition();
        let removable = composition.layers().len() > 1;
        let below_top = composition
            .layer_index(layer)
            .is_some_and(|index| index + 1 < composition.layers().len());
        let (mut stop, mut remove, mut group, mut ungroup) = (false, false, false, false);
        let Some(entry) = self.document.composition_mut().layer_mut(layer) else {
            return;
        };
        let mut covers = Vec::new();
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let name = RichText::new(entry.name.clone()).semibold();
                let name = if selected_layer {
                    name.color(theme::palette(ui).brand_text)
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
                        ui.separator();
                        if ui
                            .add_enabled(
                                below_top,
                                egui::Button::new("Grouper avec la couche du dessus"),
                            )
                            .clicked()
                        {
                            group = true;
                            ui.close();
                        }
                        if entry.group().is_some() && ui.button("Retirer du groupe").clicked() {
                            ungroup = true;
                            ui.close();
                        }
                        ui.separator();
                        if ui
                            .add_enabled(removable, egui::Button::new("Supprimer la couche"))
                            .clicked()
                        {
                            remove = true;
                            ui.close();
                        }
                    });
                if let Some(ClipState::Error(message)) = state {
                    ui.label(RichText::new("⚠").color(theme::palette(ui).danger_text))
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
                .add(
                    Slider::new(&mut opacity, 0.0..=1.0)
                        .show_value(false)
                        .quarters(),
                )
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
        if group {
            let _ = self.document.composition_mut().group_with_above(layer);
        }
        if ungroup {
            let _ = self.document.composition_mut().remove_from_group(layer);
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
        let palette = theme::palette(ui);
        let painter = ui.painter();
        let radius = theme::RADIUS;
        let Some(name) = self.name else {
            let fill = if self.hovered {
                palette.bg
            } else {
                Color32::TRANSPARENT
            };
            painter.rect(
                rect,
                radius,
                fill,
                Stroke::new(1.0, palette.border_subtle),
                StrokeKind::Inside,
            );
            if self.hovered {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "+",
                    TextStyle::Heading.resolve(ui.style()),
                    palette.text_tertiary,
                );
            }
            self.paint_selection(ui, rect);
            return;
        };

        let fill = match (self.playing, self.hovered) {
            (true, _) => palette.success,
            (false, true) => palette.card_hover,
            (false, false) => palette.card,
        };
        painter.rect_filled(rect, radius, fill);

        let inner = rect.shrink(theme::SPACE_2);
        let (kind_color, name_color) = match (self.missing, self.playing) {
            (true, _) => (palette.danger_text, palette.danger_text),
            (false, true) => (Color32::WHITE, Color32::WHITE),
            (false, false) => (palette.text_tertiary, palette.text),
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
            painter.rect_filled(bar, 0.0, Color32::WHITE);
        }
        self.paint_selection(ui, rect);
    }

    fn paint_selection(&self, ui: &egui::Ui, rect: Rect) {
        let palette = theme::palette(ui);
        if self.selected {
            ui.painter().rect_stroke(
                rect,
                theme::RADIUS,
                Stroke::new(2.0, palette.brand),
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
