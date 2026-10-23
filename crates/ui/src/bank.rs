//! The bank of effects and generators: families, a search, a moving
//! preview of each; dragged onto the grid or onto a chain of effects, or
//! double-clicked to go to the selection.

use egui::{Rect, Sense, Stroke, StrokeKind, Vec2};
use prism_core::EffectOwner;
use prism_engine::{effect_preview_rect, preview_rect};
use prism_fx::{CATALOG, EFFECT_CATEGORIES, EffectInfo, GENERATOR_CATEGORIES, GENERATORS};

use crate::panel::ControlPanel;
use crate::theme;

/// Size of a preview in the bank.
const TILE: Vec2 = Vec2::new(112.0, 63.0);

/// An effect or a generator of the bank, while dragged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BankItem {
    pub(crate) info: &'static EffectInfo,
    pub(crate) generator: bool,
}

/// What the bank shows.
#[derive(Default)]
pub(crate) struct BankState {
    generators: bool,
    family: Option<usize>,
    search: String,
}

/// The kind of effect of the bank dropped on `rect` this frame, if any;
/// outlines `rect` while one is held over it.
pub(crate) fn effect_dropped(ui: &egui::Ui, rect: Rect) -> Option<&'static str> {
    dropped(ui, rect, false)
}

/// The kind of generator of the bank dropped on `rect` this frame, if any.
pub(crate) fn generator_dropped(ui: &egui::Ui, rect: Rect) -> Option<&'static str> {
    dropped(ui, rect, true)
}

fn dropped(ui: &egui::Ui, rect: Rect, generator: bool) -> Option<&'static str> {
    let item = egui::DragAndDrop::payload::<BankItem>(ui.ctx())?;
    if item.generator != generator {
        return None;
    }
    let pointer = ui.ctx().pointer_interact_pos()?;
    if !rect.contains(pointer) || !ui.clip_rect().contains(pointer) {
        return None;
    }
    let palette = theme::palette(ui);
    ui.painter().rect(
        rect,
        theme::RADIUS,
        palette.brand.gamma_multiply(0.12),
        Stroke::new(1.5, palette.brand),
        StrokeKind::Inside,
    );
    if ui.input(|input| input.pointer.any_released()) {
        egui::DragAndDrop::clear_payload(ui.ctx());
        return Some(item.info.kind);
    }
    None
}

/// Draws the preview of `item` in `rect`, or a plain card without previews.
fn paint_preview(ui: &egui::Ui, previews: Option<egui::TextureId>, item: BankItem, rect: Rect) {
    let index = if item.generator {
        GENERATORS
            .iter()
            .position(|known| known.kind == item.info.kind)
    } else {
        CATALOG
            .iter()
            .position(|known| known.kind == item.info.kind)
    };
    match (previews, index) {
        (Some(texture), Some(index)) => {
            let [left, top, right, bottom] = if item.generator {
                preview_rect(index)
            } else {
                effect_preview_rect(index)
            };
            ui.painter().image(
                texture,
                rect,
                Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom)),
                egui::Color32::WHITE,
            );
        }
        _ => {
            ui.painter()
                .rect_filled(rect, theme::RADIUS, theme::palette(ui).card);
        }
    }
}

impl ControlPanel {
    /// The bank: effects or generators, by family.
    pub(crate) fn bank_panel(&mut self, ui: &mut egui::Ui, previews: Option<egui::TextureId>) {
        let palette = theme::palette(ui);
        let mut picked = None;
        let state = &mut self.bank;
        ui.horizontal(|ui| {
            if ui.selectable_label(!state.generators, "Effets").clicked() && state.generators {
                state.generators = false;
                state.family = None;
            }
            if ui
                .selectable_label(state.generators, "Générateurs")
                .clicked()
                && !state.generators
            {
                state.generators = true;
                state.family = None;
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut state.search)
                .hint_text("Rechercher…")
                .desired_width(f32::INFINITY),
        );
        let families = if state.generators {
            GENERATOR_CATEGORIES
        } else {
            EFFECT_CATEGORIES
        };
        let family_name = state
            .family
            .and_then(|family| families.get(family))
            .map_or("Toutes les familles", |(name, _)| *name);
        egui::ComboBox::from_id_salt(("bank family", state.generators))
            .selected_text(family_name)
            .width(ui.available_width() - 8.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.family, None, "Toutes les familles");
                for (index, (name, kinds)) in families.iter().enumerate() {
                    ui.selectable_value(
                        &mut state.family,
                        Some(index),
                        format!("{name} ({})", kinds.len()),
                    );
                }
            });
        let list: &'static [EffectInfo] = if state.generators {
            GENERATORS
        } else {
            CATALOG
        };
        let search = state.search.trim().to_lowercase();
        let shown: Vec<BankItem> = list
            .iter()
            .filter(|info| {
                state.family.is_none_or(|family| {
                    families
                        .get(family)
                        .is_some_and(|(_, kinds)| kinds.contains(&info.kind))
                })
            })
            .filter(|info| search.is_empty() || info.label.to_lowercase().contains(&search))
            .map(|info| BankItem {
                info,
                generator: state.generators,
            })
            .collect();
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("bank")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                    for item in shown {
                        let (rect, response) =
                            ui.allocate_exact_size(TILE + Vec2::new(0.0, 18.0), Sense::click_and_drag());
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, item.info.label)
                        });
                        let image = Rect::from_min_size(rect.min, TILE);
                        paint_preview(ui, previews, item, image);
                        ui.painter().text(
                            egui::pos2(rect.left() + 2.0, image.bottom() + 3.0),
                            egui::Align2::LEFT_TOP,
                            item.info.label,
                            egui::FontId::proportional(11.5),
                            palette.text,
                        );
                        if response.hovered() {
                            ui.painter().rect_stroke(
                                image,
                                theme::RADIUS,
                                Stroke::new(1.0, palette.brand),
                                StrokeKind::Outside,
                            );
                        }
                        let response = response.on_hover_text(if item.generator {
                            "Glisser sur une case, ou double-cliquer pour la case sélectionnée"
                        } else {
                            "Glisser sur une case, une couche ou une chaîne d'effets, ou double-cliquer pour la sélection"
                        });
                        if response.dragged() {
                            egui::DragAndDrop::set_payload(ui.ctx(), item);
                            drag_label(ui, item);
                        }
                        if response.double_clicked() {
                            picked = Some(item);
                        }
                    }
                });
            });
        if let Some(item) = picked {
            self.bank_to_selection(item);
        }
    }

    /// Puts `item` on the selection: a generator in the selected cell, an
    /// effect on its clip, or on its layer when the cell is empty.
    fn bank_to_selection(&mut self, item: BankItem) {
        let Some(slot) = self.selected else {
            return;
        };
        if item.generator {
            self.use_generator(slot, item.info);
            return;
        }
        let owner = if self.document.composition().clip(slot).is_some() {
            EffectOwner::Clip { slot }
        } else {
            EffectOwner::Layer { layer: slot.layer }
        };
        if let Some(effects) = self.effects_of(owner) {
            effects.push(item.info.create());
        }
    }
}

/// The name of what is dragged, next to the pointer.
fn drag_label(ui: &egui::Ui, item: BankItem) {
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let palette = theme::palette(ui);
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("bank drag label"),
    ));
    let text = painter.layout_no_wrap(
        item.info.label.to_owned(),
        egui::FontId::proportional(13.0),
        palette.text,
    );
    let rect = Rect::from_min_size(
        pointer + Vec2::new(14.0, 6.0),
        text.size() + Vec2::new(12.0, 6.0),
    );
    painter.rect_filled(rect, theme::RADIUS, palette.menu);
    painter.galley(rect.min + Vec2::new(6.0, 3.0), text, palette.text);
}
