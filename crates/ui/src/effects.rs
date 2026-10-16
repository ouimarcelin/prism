//! Editing a chain of effects: one block per effect, its settings, and a
//! menu to add one.

use std::fmt::Debug;
use std::hash::Hash;

use egui::RichText;
use prism_core::{Animation, Effect, MIN_PERIOD, Period, Shape};
use prism_fx::{CATALOG, EffectInfo, ParamInfo, find, find_generator};

/// A change asked for in the chain editor.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ChainEdit {
    /// Adds an effect of this kind at the end.
    Add(&'static str),
    /// Moves the effect at this index one place earlier.
    MoveUp(usize),
    /// Moves the effect at this index one place later.
    MoveDown(usize),
    /// Removes the effect at this index.
    Remove(usize),
}

/// Applies `edit` to `effects`. Edits out of range do nothing.
pub(crate) fn apply_edit(effects: &mut Vec<Effect>, edit: &ChainEdit) {
    match *edit {
        ChainEdit::Add(kind) => {
            if let Some(info) = find(kind) {
                effects.push(info.create());
            }
        }
        ChainEdit::MoveUp(index) if index > 0 && index < effects.len() => {
            effects.swap(index - 1, index);
        }
        ChainEdit::MoveDown(index) if index + 1 < effects.len() => {
            effects.swap(index, index + 1);
        }
        ChainEdit::Remove(index) if index < effects.len() => {
            effects.remove(index);
        }
        _ => {}
    }
}

/// A collapsible section editing `effects`, titled `title`. `id` tells the
/// chains of different layers and clips apart.
pub(crate) fn effect_section(
    ui: &mut egui::Ui,
    title: &str,
    id: impl Hash + Debug,
    effects: &mut Vec<Effect>,
) {
    let active = effects.iter().filter(|effect| !effect.bypass).count();
    let header = if effects.is_empty() {
        title.to_owned()
    } else {
        format!("{title} ({active}/{})", effects.len())
    };
    egui::CollapsingHeader::new(RichText::new(header).strong())
        .id_salt(("effects", &id))
        .default_open(!effects.is_empty())
        .show(ui, |ui| effect_chain(ui, &id, effects));
}

/// Every effect of the chain, then the menu to add one.
fn effect_chain(ui: &mut egui::Ui, id: impl Hash + Debug, effects: &mut Vec<Effect>) {
    let mut edits = Vec::new();
    let count = effects.len();
    for (index, effect) in effects.iter_mut().enumerate() {
        ui.push_id(index, |ui| {
            let info = find(&effect.kind);
            ui.horizontal(|ui| {
                let name = info.map_or("Effet inconnu", |info| info.label);
                let mut on = !effect.bypass;
                ui.checkbox(&mut on, RichText::new(name).strong())
                    .on_hover_text("Décocher pour passer l'effet (bypass)");
                effect.bypass = !on;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Retirer l'effet").clicked() {
                        edits.push(ChainEdit::Remove(index));
                    }
                    if ui
                        .add_enabled(index + 1 < count, egui::Button::new("⏷").small())
                        .on_hover_text("Plus tard dans la chaîne")
                        .clicked()
                    {
                        edits.push(ChainEdit::MoveDown(index));
                    }
                    if ui
                        .add_enabled(index > 0, egui::Button::new("⏶").small())
                        .on_hover_text("Plus tôt dans la chaîne")
                        .clicked()
                    {
                        edits.push(ChainEdit::MoveUp(index));
                    }
                });
            });
            match info {
                Some(info) => settings(ui, info, effect),
                None => {
                    ui.label(
                        RichText::new(format!(
                            "« {} » vient d'une autre version de Prism : il est gardé dans le projet mais pas rendu.",
                            effect.kind
                        ))
                        .weak(),
                    );
                }
            }
            ui.add_space(4.0);
        });
    }

    egui::ComboBox::from_id_salt(("add effect", &id))
        .selected_text("Ajouter un effet…")
        .show_ui(ui, |ui| {
            for info in CATALOG {
                if ui.selectable_label(false, info.label).clicked() {
                    edits.push(ChainEdit::Add(info.kind));
                }
            }
        });
    for edit in &edits {
        apply_edit(effects, edit);
    }
}

/// One line per setting of an effect.
/// Settings of the generator of a generated clip, animations included.
pub(crate) fn generator_settings(ui: &mut egui::Ui, generator: &mut Effect) {
    match find_generator(&generator.kind) {
        Some(info) => {
            ui.label(RichText::new(info.label).strong());
            ui.push_id("generator", |ui| settings(ui, info, generator));
        }
        None => {
            ui.label(
                RichText::new(format!(
                    "Générateur « {} » d'une autre version de Prism : gardé dans le projet mais pas dessiné.",
                    generator.kind
                ))
                .weak(),
            );
        }
    }
}

fn settings(ui: &mut egui::Ui, info: &EffectInfo, effect: &mut Effect) {
    egui::Grid::new("settings")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for param in info.params {
                let animated = effect.animations.contains_key(param.id);
                ui.label(param.label);
                ui.horizontal(|ui| {
                    // While animated, the set value waits for the animation
                    // to stop.
                    ui.add_enabled_ui(!animated, |ui| setting(ui, param, effect));
                    animation_toggle(ui, param, effect);
                });
                ui.end_row();
                if let Some(animation) = effect.animations.get_mut(param.id) {
                    ui.label("");
                    ui.push_id(param.id, |ui| animation_settings(ui, param, animation));
                    ui.end_row();
                }
            }
        });
}

/// Name of an animation shape in the interface.
fn shape_label(shape: Shape) -> &'static str {
    match shape {
        Shape::Sine => "Sinus",
        Shape::Triangle => "Triangle",
        Shape::Ramp => "Rampe",
        Shape::Square => "Carré",
        Shape::Random => "Aléatoire",
    }
}

/// Starts or stops the animation of a setting.
fn animation_toggle(ui: &mut egui::Ui, param: &ParamInfo, effect: &mut Effect) {
    let animated = effect.animations.contains_key(param.id);
    let hint = if animated {
        "Arrêter l'animation de ce réglage"
    } else {
        "Animer ce réglage : une onde entre deux valeurs, au tempo ou en secondes"
    };
    if ui
        .selectable_label(animated, "~")
        .on_hover_text(hint)
        .clicked()
    {
        if animated {
            effect.animations.remove(param.id);
        } else {
            effect
                .animations
                .insert(param.id.to_owned(), Animation::new(param.min, param.max));
        }
    }
}

/// Shape, period and range of an animation.
fn animation_settings(ui: &mut egui::Ui, param: &ParamInfo, animation: &mut Animation) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("shape")
                .selected_text(shape_label(animation.shape))
                .show_ui(ui, |ui| {
                    for shape in Shape::ALL {
                        ui.selectable_value(&mut animation.shape, shape, shape_label(shape));
                    }
                });
            let (mut length, on_beats) = match animation.period {
                Period::Beats(length) => (length, true),
                Period::Seconds(length) => (length, false),
            };
            ui.add(
                egui::DragValue::new(&mut length)
                    .range(MIN_PERIOD..=64.0)
                    .speed(0.05)
                    .suffix(if on_beats { " temps" } else { " s" }),
            )
            .on_hover_text("Durée d'un cycle");
            let beats = ui
                .selectable_label(on_beats, "au tempo")
                .on_hover_text("Compter le cycle en temps, au tempo de la composition")
                .clicked()
                != on_beats;
            animation.period = if beats {
                Period::Beats(length)
            } else {
                Period::Seconds(length)
            };
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = 70.0;
            ui.label("de");
            ui.add(egui::Slider::new(
                &mut animation.from,
                param.min..=param.max,
            ));
            ui.label("à");
            ui.add(egui::Slider::new(&mut animation.to, param.min..=param.max));
        });
    });
}

fn setting(ui: &mut egui::Ui, param: &ParamInfo, effect: &mut Effect) {
    let mut value = param.clamp(effect.param(param.id).unwrap_or(param.default));
    let response = if param.choices.is_empty() {
        ui.add(egui::Slider::new(&mut value, param.min..=param.max))
    } else {
        let selected = param.choices.get(value as usize).copied().unwrap_or("");
        egui::ComboBox::from_id_salt(param.id)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (index, choice) in param.choices.iter().enumerate() {
                    ui.selectable_value(&mut value, index as f32, *choice);
                }
            })
            .response
    };
    let mut reset = false;
    response
        .on_hover_text("Clic droit : valeur par défaut")
        .context_menu(|ui| {
            if ui.button("Valeur par défaut").clicked() {
                reset = true;
                ui.close();
            }
        });
    if reset {
        value = param.default;
    }
    // Untouched settings stay out of the project file.
    let stored = effect.param(param.id);
    if stored != Some(value) && (stored.is_some() || value != param.default) {
        effect.set_param(param.id, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(effects: &[Effect]) -> Vec<&str> {
        effects.iter().map(|effect| effect.kind.as_str()).collect()
    }

    #[test]
    fn edits_add_move_and_remove() {
        let mut effects = Vec::new();
        for kind in ["blur", "invert", "mirror"] {
            apply_edit(&mut effects, &ChainEdit::Add(kind));
        }
        apply_edit(&mut effects, &ChainEdit::Add("unknown"));
        assert_eq!(kinds(&effects), ["blur", "invert", "mirror"]);

        apply_edit(&mut effects, &ChainEdit::MoveUp(2));
        assert_eq!(kinds(&effects), ["blur", "mirror", "invert"]);
        apply_edit(&mut effects, &ChainEdit::MoveDown(0));
        assert_eq!(kinds(&effects), ["mirror", "blur", "invert"]);
        // At the ends, nothing moves.
        apply_edit(&mut effects, &ChainEdit::MoveUp(0));
        apply_edit(&mut effects, &ChainEdit::MoveDown(2));
        assert_eq!(kinds(&effects), ["mirror", "blur", "invert"]);

        apply_edit(&mut effects, &ChainEdit::Remove(1));
        apply_edit(&mut effects, &ChainEdit::Remove(7));
        assert_eq!(kinds(&effects), ["mirror", "invert"]);
    }
}
