//! Editing a chain of effects: one block per effect, its settings, and a
//! menu to add one.

use std::fmt::Debug;
use std::path::{Path, PathBuf};

use egui::RichText;
use prism_core::{
    Animation, Band, ControlTarget, DASHBOARD_SIZE, Effect, EffectOwner, MIN_PERIOD, Period, Shape,
    Slot, Unit,
};
use prism_fx::{CATALOG, EffectInfo, ISF_KIND, ParamInfo, find, find_generator, shader_of};

use crate::learn::Learn;
use crate::presets::PresetStore;
use crate::slider::Slider;
use crate::theme::Emphasis;

/// What effect editors share with the rest of the window: presets, and the
/// learn mode.
pub(crate) struct Tools<'a> {
    pub(crate) presets: &'a mut PresetStore,
    pub(crate) plugins: &'a mut crate::plugins::PluginShelf,
    pub(crate) learn: &'a mut Learn,
}

/// A change asked for in the chain editor.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ChainEdit {
    /// Adds an effect of this kind at the end.
    Add(&'static str),
    /// Adds an effect read from this ISF file at the end.
    AddFile(PathBuf),
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
        ChainEdit::AddFile(ref path) => effects.push(isf_effect(path)),
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

/// An effect read from the ISF file at `path`.
pub(crate) fn isf_effect(path: &Path) -> Effect {
    let mut effect = Effect::new(ISF_KIND);
    effect.file = Some(path.to_path_buf());
    effect
}

/// Asks for an ISF file.
pub(crate) fn pick_isf_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Choisir un shader ISF")
        .add_filter("Shader ISF", &["fs", "isf", "frag"])
        .pick_file()
}

/// What an effect is: its description, or why it is not rendered.
fn describe(effect: &Effect) -> Result<&'static EffectInfo, String> {
    if effect.kind == ISF_KIND {
        return shader_of(effect)
            .map(|shader| shader.info)
            .map_err(|error| match &effect.file {
                Some(path) => format!("{} : {error}", path.display()),
                None => error.to_string(),
            });
    }
    find(&effect.kind)
        .or_else(|| find_generator(&effect.kind))
        .ok_or_else(|| {
            format!(
                "« {} » vient d'une autre version de Prism : il est gardé dans le projet mais pas rendu.",
                effect.kind
            )
        })
}

/// Hover text of an effect read from a file: its path and description.
fn file_hover(effect: &Effect) -> Option<String> {
    let path = effect.file.as_ref()?;
    let description = shader_of(effect).ok().and_then(|shader| shader.description);
    Some(match description {
        Some(description) => format!("{description}\n{}", path.display()),
        None => path.display().to_string(),
    })
}

/// A collapsible section editing `effects`, titled `title`. `id` tells the
/// chains of different layers and clips apart, `owner` names it for
/// mappings. With `dashboard`, settings can follow a slider of the clip's
/// dashboard.
pub(crate) fn effect_section(
    ui: &mut egui::Ui,
    title: &str,
    owner: EffectOwner,
    effects: &mut Vec<Effect>,
    dashboard: bool,
    tools: Tools<'_>,
) {
    let id = owner;
    let active = effects.iter().filter(|effect| !effect.bypass).count();
    let header = if effects.is_empty() {
        title.to_owned()
    } else {
        format!("{title} ({active}/{})", effects.len())
    };
    egui::CollapsingHeader::new(RichText::new(header).semibold())
        .id_salt(("effects", &id))
        .default_open(!effects.is_empty())
        .show(ui, |ui| effect_chain(ui, owner, effects, dashboard, tools));
}

/// Every effect of the chain, then the menu to add one.
fn effect_chain(
    ui: &mut egui::Ui,
    owner: EffectOwner,
    effects: &mut Vec<Effect>,
    dashboard: bool,
    tools: Tools<'_>,
) {
    let id = owner;
    let mut edits = Vec::new();
    let count = effects.len();
    for (index, effect) in effects.iter_mut().enumerate() {
        ui.push_id(index, |ui| {
            let info = describe(effect);
            ui.horizontal(|ui| {
                let name = match (&info, &effect.file) {
                    (Ok(info), _) => info.label.to_owned(),
                    (Err(_), Some(path)) => path.file_stem().map_or_else(
                        || "Shader".into(),
                        |stem| stem.to_string_lossy().into_owned(),
                    ),
                    (Err(_), None) => "Effet inconnu".to_owned(),
                };
                let mut on = !effect.bypass;
                let hover = file_hover(effect).map_or_else(String::new, |text| format!("{text}\n"));
                let checkbox = ui
                    .checkbox(&mut on, RichText::new(name).semibold())
                    .on_hover_text(format!("{hover}Décocher pour passer l'effet (bypass)"));
                effect.bypass = !on;
                tools.learn.cover(
                    ui,
                    checkbox.rect,
                    ControlTarget::EffectBypass { owner, index },
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("×")
                        .on_hover_text("Retirer l'effet")
                        .clicked()
                    {
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
                Ok(info) => settings(ui, info, effect, dashboard, tools.learn, (owner, index)),
                Err(message) => {
                    ui.label(RichText::new(message).weak());
                }
            }
            ui.add_space(4.0);
        });
    }

    let mut add_menu = |ui: &mut egui::Ui| {
        egui::ComboBox::from_id_salt(("add effect", &id))
            .selected_text("Ajouter un effet…")
            .show_ui(ui, |ui| {
                for info in CATALOG {
                    if ui.selectable_label(false, info.label).clicked() {
                        edits.push(ChainEdit::Add(info.kind));
                    }
                }
                ui.separator();
                if let Some(path) = tools.plugins.menu_items(ui, true) {
                    edits.push(ChainEdit::AddFile(path));
                }
                ui.separator();
                if ui
                    .selectable_label(false, "Shader ISF…")
                    .on_hover_text("Un effet lu depuis un fichier ISF (.fs)")
                    .clicked()
                    && let Some(path) = pick_isf_file()
                {
                    edits.push(ChainEdit::AddFile(path));
                }
            })
    };
    ui.horizontal(|ui| {
        add_menu(ui);
        tools
            .presets
            .effects_menu(ui, egui::Id::new(("presets", &id)), effects);
    });
    for edit in &edits {
        apply_edit(effects, edit);
    }
}

/// One line per setting of an effect.
/// Settings of the generator of a generated clip, animations included.
pub(crate) fn generator_settings(
    ui: &mut egui::Ui,
    generator: &mut Effect,
    learn: &mut Learn,
    slot: Slot,
) {
    match describe(generator) {
        Ok(info) => {
            let title = ui.label(RichText::new(info.label).semibold());
            if let Some(hover) = file_hover(generator) {
                title.on_hover_text(hover);
            }
            if info.text.is_some() {
                ui.add(
                    egui::TextEdit::multiline(&mut generator.text)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .hint_text("Texte à afficher"),
                );
            }
            let owner = (EffectOwner::Generator { slot }, 0);
            ui.push_id("generator", |ui| {
                settings(ui, info, generator, true, learn, owner)
            });
        }
        Err(message) => {
            ui.label(RichText::new(message).weak());
        }
    }
}

/// `place` is the chain and index of the effect, for mappings.
fn settings(
    ui: &mut egui::Ui,
    info: &EffectInfo,
    effect: &mut Effect,
    dashboard: bool,
    learn: &mut Learn,
    place: (EffectOwner, usize),
) {
    egui::Grid::new("settings")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for param in info.params {
                let driven = effect.animations.contains_key(param.id)
                    || effect.audio.contains_key(param.id)
                    || (dashboard && effect.links.contains_key(param.id));
                ui.label(param.label);
                ui.horizontal(|ui| {
                    // While animated or following a slider, the set value
                    // waits.
                    let rect = ui
                        .add_enabled_ui(!driven, |ui| setting(ui, param, effect))
                        .inner;
                    learn.cover(
                        ui,
                        rect,
                        ControlTarget::EffectParam {
                            owner: place.0,
                            index: place.1,
                            param: param.id.to_owned(),
                        },
                    );
                    animation_toggle(ui, param, effect);
                    ui.push_id(param.id, |ui| link_menu(ui, param, effect, dashboard));
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

/// Links a setting to a slider of the dashboard (with `dashboard`) or to a
/// band of the sound, or unlinks it.
fn link_menu(ui: &mut egui::Ui, param: &ParamInfo, effect: &mut Effect, dashboard: bool) {
    let before = match (effect.audio.get(param.id), effect.links.get(param.id)) {
        (Some(band), _) => Link::Band(*band),
        (None, Some(index)) if dashboard => Link::Slider(*index),
        _ => Link::None,
    };
    let mut link = before;
    let text = match link {
        Link::None => "—".to_owned(),
        Link::Slider(index) => format!("C{}", index + 1),
        Link::Band(band) => band_short_label(band).to_owned(),
    };
    let hover = if dashboard {
        "Suivre un curseur du tableau de bord du clip ou une bande du son (menu Audio), sur toute la plage du réglage"
    } else {
        "Suivre une bande du son (menu Audio), sur toute la plage du réglage"
    };
    egui::ComboBox::from_id_salt("link")
        .width(48.0)
        .selected_text(text)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut link, Link::None, "Aucun lien");
            if dashboard {
                ui.separator();
                for index in 0..DASHBOARD_SIZE {
                    ui.selectable_value(
                        &mut link,
                        Link::Slider(index),
                        format!("Curseur {}", index + 1),
                    );
                }
            }
            ui.separator();
            for band in Band::ALL {
                ui.selectable_value(
                    &mut link,
                    Link::Band(band),
                    format!("Son : {}", band_label(band)),
                );
            }
        })
        .response
        .on_hover_text(hover);
    if link != before {
        effect.audio.remove(param.id);
        if dashboard {
            effect.links.remove(param.id);
        }
        match link {
            Link::None => {}
            Link::Slider(index) => {
                effect.links.insert(param.id.to_owned(), index);
            }
            Link::Band(band) => {
                effect.audio.insert(param.id.to_owned(), band);
            }
        }
    }
}

/// What a setting follows besides its own value.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Link {
    None,
    Slider(usize),
    Band(Band),
}

/// A menu to make a value follow a band of the sound, or not.
pub(crate) fn band_menu(ui: &mut egui::Ui, id: &str, band: &mut Option<Band>) {
    egui::ComboBox::from_id_salt(id)
        .width(48.0)
        .selected_text(band.map_or("—", band_short_label))
        .show_ui(ui, |ui| {
            ui.selectable_value(band, None, "Aucun lien");
            ui.separator();
            for choice in Band::ALL {
                ui.selectable_value(band, Some(choice), format!("Son : {}", band_label(choice)));
            }
        })
        .response
        .on_hover_text("Suivre une bande du son (menu Audio), de 0 à 1");
}

/// Name of a band of the sound.
pub(crate) fn band_label(band: Band) -> &'static str {
    match band {
        Band::Level => "niveau",
        Band::Bass => "graves",
        Band::Mid => "médiums",
        Band::High => "aigus",
    }
}

fn band_short_label(band: Band) -> &'static str {
    match band {
        Band::Level => "Niv.",
        Band::Bass => "Grav.",
        Band::Mid => "Méd.",
        Band::High => "Aigus",
    }
}

/// A collapsible section with the sliders of a clip's dashboard.
pub(crate) fn dashboard_section(
    ui: &mut egui::Ui,
    slot: Slot,
    dashboard: &mut [Unit; DASHBOARD_SIZE],
    learn: &mut Learn,
) {
    egui::CollapsingHeader::new(RichText::new("Tableau de bord").semibold())
        .id_salt(("dashboard", slot))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Huit curseurs que les réglages d'effet peuvent suivre (menu « — » à côté d'un réglage).")
                    .weak()
                    .small(),
            );
            egui::Grid::new("dashboard")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    for (index, slider) in dashboard.iter_mut().enumerate() {
                        ui.label(format!("Curseur {}", index + 1));
                        let mut value = slider.get();
                        let response = ui.add(Slider::new(&mut value, 0.0..=1.0).quarters());
                        *slider = Unit::new(value);
                        learn.cover(
                            ui,
                            response.rect,
                            ControlTarget::Dashboard {
                                slot,
                                slider: index,
                            },
                        );
                        ui.end_row();
                    }
                });
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
pub(crate) fn animation_settings(ui: &mut egui::Ui, param: &ParamInfo, animation: &mut Animation) {
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
            if ui
                .selectable_label(animation.once, "une fois")
                .on_hover_text(
                    "Enveloppe : un seul cycle à chaque lancement du clip, \
                     puis la dernière valeur est gardée",
                )
                .clicked()
            {
                animation.once = !animation.once;
            }
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = 70.0;
            ui.label("de");
            ui.add(
                Slider::new(&mut animation.from, param.min..=param.max)
                    .snaps([f64::from(param.default)]),
            );
            ui.label("à");
            ui.add(
                Slider::new(&mut animation.to, param.min..=param.max)
                    .snaps([f64::from(param.default)]),
            );
        });
    });
}

/// Returns where the control is drawn.
fn setting(ui: &mut egui::Ui, param: &ParamInfo, effect: &mut Effect) -> egui::Rect {
    let mut value = param.clamp(effect.param(param.id).unwrap_or(param.default));
    let response = if param.choices.is_empty() {
        ui.add(Slider::new(&mut value, param.min..=param.max).snaps([f64::from(param.default)]))
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
    let rect = response.rect;
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
    rect
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

        apply_edit(&mut effects, &ChainEdit::AddFile("/shaders/glow.fs".into()));
        assert_eq!(kinds(&effects), ["mirror", "invert", ISF_KIND]);
        assert_eq!(
            effects[2].file.as_deref(),
            Some(Path::new("/shaders/glow.fs"))
        );
        // A missing file: said so, with its path.
        let message = describe(&effects[2]).unwrap_err();
        assert!(message.starts_with("/shaders/glow.fs : "), "{message}");
    }
}
