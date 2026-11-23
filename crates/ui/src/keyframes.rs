//! The timeline: settings of the selected clip and of its layer animated by
//! keys, one track each, keys dragged along the cycle and up and down.

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use prism_core::{Animation, Composition, Ease, Effect, Keyframe, Period, Shape, Slot};
use prism_engine::EngineStatus;

use crate::panel::ControlPanel;
use crate::theme::{self, Emphasis};

/// Height of a track.
const TRACK_HEIGHT: f32 = 54.0;
/// Width of the names of the tracks.
const NAMES_WIDTH: f32 = 220.0;
/// Half the size of a key.
const KEY_RADIUS: f32 = 5.0;

/// Where an animated setting is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TrackRef {
    /// A setting of the generator of the clip.
    Generator(&'static str),
    /// A setting of an effect of the clip.
    ClipEffect(usize, &'static str),
    /// The opacity of the layer.
    LayerOpacity,
    /// A setting of an effect of the layer.
    LayerEffect(usize, &'static str),
}

/// A setting that can get a track: where it is, its name and its range.
struct Setting {
    track: TrackRef,
    name: String,
    range: (f32, f32),
    value: f32,
}

/// The settings of `effect` that can get a track, named after `owner`.
fn add_effect(
    out: &mut Vec<Setting>,
    effect: &Effect,
    owner: &str,
    make: &dyn Fn(&'static str) -> TrackRef,
) {
    let Ok(info) = crate::effects::describe(effect) else {
        return;
    };
    for param in info.params {
        if !param.choices.is_empty() {
            continue;
        }
        out.push(Setting {
            track: make(param.id),
            name: format!("{owner} · {}", param.label),
            range: (param.min, param.max),
            value: param.clamp(effect.param(param.id).unwrap_or(param.default)),
        });
    }
}

/// Every setting of the clip in `slot` and of its layer, animated or not.
fn settings(composition: &Composition, slot: Slot) -> Vec<Setting> {
    let mut out = Vec::new();
    if let Some(clip) = composition.clip(slot) {
        if let Some(generator) = clip.source.generator() {
            add_effect(&mut out, generator, "Générateur", &TrackRef::Generator);
        }
        for (index, effect) in clip.effects.iter().enumerate() {
            let owner = crate::effects::describe(effect).map_or("Effet", |info| info.label);
            add_effect(&mut out, effect, owner, &move |id| {
                TrackRef::ClipEffect(index, id)
            });
        }
    }
    if let Some(layer) = composition.layer(slot.layer) {
        out.push(Setting {
            track: TrackRef::LayerOpacity,
            name: "Couche · Opacité".to_owned(),
            range: (0.0, 1.0),
            value: layer.opacity.get(),
        });
        for (index, effect) in layer.effects.iter().enumerate() {
            let owner = crate::effects::describe(effect).map_or("Effet", |info| info.label);
            add_effect(&mut out, effect, &format!("Couche · {owner}"), &move |id| {
                TrackRef::LayerEffect(index, id)
            });
        }
    }
    out
}

/// The animation of `track` in `composition`, mutable: `None` when the
/// setting is gone.
fn with_animation<R>(
    composition: &mut Composition,
    slot: Slot,
    track: TrackRef,
    edit: impl FnOnce(&mut Option<Animation>) -> R,
) -> Option<R> {
    match track {
        TrackRef::LayerOpacity => {
            let layer = composition.layer_mut(slot.layer)?;
            Some(edit(&mut layer.opacity_animation))
        }
        TrackRef::Generator(id) => {
            let generator = composition.clip_mut(slot)?.source.generator_mut()?;
            Some(edit_effect(generator, id, edit))
        }
        TrackRef::ClipEffect(index, id) => {
            let effect = composition.clip_mut(slot)?.effects.get_mut(index)?;
            Some(edit_effect(effect, id, edit))
        }
        TrackRef::LayerEffect(index, id) => {
            let effect = composition.layer_mut(slot.layer)?.effects.get_mut(index)?;
            Some(edit_effect(effect, id, edit))
        }
    }
}

/// Runs `edit` on the animation of setting `id` of `effect`, the map of
/// animations updated after.
fn edit_effect<R>(
    effect: &mut Effect,
    id: &str,
    edit: impl FnOnce(&mut Option<Animation>) -> R,
) -> R {
    let mut animation = effect.animations.get(id).copied();
    let result = edit(&mut animation);
    match animation {
        Some(animation) => {
            effect.animations.insert(id.to_owned(), animation);
        }
        None => {
            effect.animations.remove(id);
        }
    }
    result
}

/// A new track: two keys at the setting's current value, over a bar.
fn new_track(value: f32) -> Animation {
    let mut animation = Animation {
        shape: Shape::Keyframes,
        period: Period::Beats(4.0),
        ..Animation::new(value, value)
    };
    for time in [0.0, 1.0] {
        animation.keys.insert(Keyframe {
            time,
            value,
            ease: Ease::Linear,
        });
    }
    animation
}

/// Name of an easing.
fn ease_label(ease: Ease) -> &'static str {
    match ease {
        Ease::Linear => "Linéaire",
        Ease::Smooth => "Douce",
        Ease::In => "Entrée lente",
        Ease::Out => "Sortie lente",
        Ease::Hold => "Palier",
    }
}

/// Where the cycle of `animation` stands now, from 0 to 1, when it loops.
fn playhead(animation: &Animation, beat: f64, seconds: f64) -> Option<f32> {
    if animation.once {
        return None;
    }
    let (position, length) = match animation.period {
        Period::Beats(length) => (beat, length),
        Period::Seconds(length) => (seconds, length),
    };
    let cycles =
        position / f64::from(length.max(prism_core::MIN_PERIOD)) + f64::from(animation.phase);
    Some((cycles - cycles.floor()) as f32)
}

impl ControlPanel {
    /// The timeline of the selected clip: its tracks, a menu to add one.
    pub(crate) fn keyframes_panel(&mut self, ui: &mut egui::Ui, status: &EngineStatus) {
        let palette = theme::palette(ui);
        let Some(slot) = self.selected else {
            ui.label(
                RichText::new("Sélectionnez une case pour animer ses réglages par points clés.")
                    .weak(),
            );
            return;
        };
        let all = settings(self.document.composition(), slot);
        let mut tracks = Vec::new();
        let mut free = Vec::new();
        for setting in all {
            let keyed = with_animation(
                self.document.composition_mut(),
                slot,
                setting.track,
                |animation| animation.is_some_and(|animation| animation.shape == Shape::Keyframes),
            )
            .unwrap_or(false);
            if keyed {
                tracks.push(setting);
            } else {
                free.push(setting);
            }
        }
        let name = self
            .document
            .composition()
            .clip(slot)
            .map_or_else(|| "case vide".to_owned(), |clip| clip.name.clone());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Timeline").heading());
            ui.label(RichText::new(name).weak());
            ui.add_space(12.0);
            ui.menu_button("+ Piste", |ui| {
                    egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    if free.is_empty() {
                        ui.label(RichText::new("Aucun réglage à animer").weak());
                    }
                    for setting in &free {
                        if ui.button(&setting.name).clicked() {
                            let value = setting.value;
                            with_animation(self.document.composition_mut(), slot, setting.track, |animation| {
                                *animation = Some(new_track(value));
                            });
                            ui.close();
                        }
                    }
                    });
                });
            ui.label(
                RichText::new("Clic sur une piste : ajouter un point · glisser : déplacer · clic droit : courbe ou supprimer")
                    .weak()
                    .small(),
            );
        });
        ui.separator();
        if tracks.is_empty() {
            ui.label(
                RichText::new("Aucune piste : « + Piste » anime un réglage par points clés.")
                    .weak(),
            );
            return;
        }
        let seconds = ui.input(|input| input.time);
        egui::ScrollArea::vertical()
            .id_salt("tracks")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for setting in tracks {
                    ui.push_id(format!("{:?}", setting.track), |ui| {
                        self.track_row(ui, slot, &setting, status.beat, seconds, palette.brand);
                    });
                }
            });
    }

    /// One track: its name and cycle on the left, its keys on the right.
    #[allow(clippy::too_many_lines)]
    fn track_row(
        &mut self,
        ui: &mut egui::Ui,
        slot: Slot,
        setting: &Setting,
        beat: f64,
        seconds: f64,
        accent: Color32,
    ) {
        let palette = theme::palette(ui);
        let Some(Some(mut animation)) = with_animation(
            self.document.composition_mut(),
            slot,
            setting.track,
            |animation| *animation,
        ) else {
            return;
        };
        let before = animation;
        let mut remove = false;
        let (row, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), TRACK_HEIGHT),
            Sense::hover(),
        );
        let names = Rect::from_min_size(row.min, Vec2::new(NAMES_WIDTH, TRACK_HEIGHT));
        let lane = Rect::from_min_max(
            egui::pos2(names.right() + 8.0, row.top() + 4.0),
            row.max - Vec2::new(4.0, 4.0),
        );

        // Name, cycle length, removal.
        let mut names_ui = ui.new_child(egui::UiBuilder::new().max_rect(names.shrink(2.0)));
        names_ui.label(RichText::new(&setting.name).semibold());
        names_ui.horizontal(|ui| {
            let (mut length, on_beats) = match animation.period {
                Period::Beats(length) => (length, true),
                Period::Seconds(length) => (length, false),
            };
            ui.add(
                egui::DragValue::new(&mut length)
                    .range(prism_core::MIN_PERIOD..=512.0)
                    .speed(0.05)
                    .suffix(if on_beats { " temps" } else { " s" }),
            )
            .on_hover_text("Durée de la piste");
            let beats = ui
                .selectable_label(on_beats, "tempo")
                .on_hover_text("Compter en temps, au tempo")
                .clicked()
                != on_beats;
            animation.period = if beats {
                Period::Beats(length)
            } else {
                Period::Seconds(length)
            };
            if ui
                .selectable_label(animation.once, "1×")
                .on_hover_text("Une seule fois à chaque lancement du clip, puis la dernière valeur")
                .clicked()
            {
                animation.once = !animation.once;
            }
            if ui
                .small_button("×")
                .on_hover_text("Retirer la piste")
                .clicked()
            {
                remove = true;
            }
        });

        // The lane: grid, curve, keys.
        let painter = ui.painter_at(lane.expand(KEY_RADIUS + 2.0));
        painter.rect_filled(lane, theme::RADIUS, palette.card);
        let (low, high) = setting.range;
        let span = (high - low).max(1e-6);
        let to_screen = |time: f32, value: f32| {
            Pos2::new(
                lane.left() + time * lane.width(),
                lane.bottom() - ((value - low) / span).clamp(0.0, 1.0) * lane.height(),
            )
        };
        let from_screen = |pos: Pos2| {
            let time = ((pos.x - lane.left()) / lane.width()).clamp(0.0, 1.0);
            let value = low + (lane.bottom() - pos.y) / lane.height() * span;
            (time, value.clamp(low, high))
        };
        let quarters = 16;
        for step in 1..quarters {
            let x = lane.left() + lane.width() * step as f32 / quarters as f32;
            let strong = step % 4 == 0;
            painter.vline(
                x,
                lane.y_range(),
                Stroke::new(
                    1.0,
                    palette
                        .border
                        .gamma_multiply(if strong { 1.0 } else { 0.4 }),
                ),
            );
        }
        let curve: Vec<Pos2> = (0..=160)
            .map(|step| {
                let time = step as f32 / 160.0;
                to_screen(time, animation.keys.value_at(time).unwrap_or(low))
            })
            .collect();
        painter.add(egui::Shape::line(curve, Stroke::new(1.5, accent)));
        if let Some(t) = playhead(&animation, beat, seconds) {
            painter.vline(
                lane.left() + t * lane.width(),
                lane.y_range(),
                Stroke::new(1.0, palette.warning),
            );
        }

        // Keys.
        let lane_response = ui.interact(lane, ui.id().with("lane"), Sense::click());
        let mut over_key = false;
        let keys: Vec<Keyframe> = animation.keys.as_slice().to_vec();
        let mut moved: Option<(usize, f32, f32)> = None;
        let mut deleted = None;
        let mut eased = None;
        for (index, key) in keys.iter().enumerate() {
            let center = to_screen(key.time, key.value);
            let rect = Rect::from_center_size(center, Vec2::splat(KEY_RADIUS * 2.0 + 6.0));
            let response = ui.interact(rect, ui.id().with(("key", index)), Sense::click_and_drag());
            over_key |= response.hovered() || response.dragged();
            let active = response.hovered() || response.dragged();
            let diamond = vec![
                center + Vec2::new(0.0, -KEY_RADIUS),
                center + Vec2::new(KEY_RADIUS, 0.0),
                center + Vec2::new(0.0, KEY_RADIUS),
                center + Vec2::new(-KEY_RADIUS, 0.0),
            ];
            painter.add(egui::Shape::convex_polygon(
                diamond,
                if active { Color32::WHITE } else { accent },
                Stroke::new(1.0, palette.text),
            ));
            if response.dragged()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let (time, value) = from_screen(pointer);
                moved = Some((index, time, value));
            }
            let response = response.on_hover_text(format!(
                "{:.0} % du cycle · {:.3} · {}",
                key.time * 100.0,
                key.value,
                ease_label(key.ease)
            ));
            response.context_menu(|ui| {
                ui.label(RichText::new("Vers le point suivant").weak());
                for ease in Ease::ALL {
                    if ui
                        .selectable_label(key.ease == ease, ease_label(ease))
                        .clicked()
                    {
                        eased = Some((index, ease));
                        ui.close();
                    }
                }
                ui.separator();
                let mut value = key.value;
                ui.add(
                    egui::DragValue::new(&mut value)
                        .range(low..=high)
                        .speed(span / 200.0)
                        .prefix("Valeur "),
                );
                if value != key.value {
                    moved = Some((index, key.time, value));
                }
                if ui.button("Supprimer le point").clicked() {
                    deleted = Some(index);
                    ui.close();
                }
            });
        }
        if let Some((index, time, value)) = moved {
            animation.keys.set(index, time, value);
        }
        if let Some((index, ease)) = eased
            && let Some(key) = animation.keys.as_slice().get(index).copied()
        {
            animation.keys.remove(index);
            animation.keys.insert(Keyframe { ease, ..key });
        }
        if let Some(index) = deleted
            && animation.keys.len() > 1
        {
            animation.keys.remove(index);
        }
        if lane_response.clicked()
            && !over_key
            && let Some(pointer) = lane_response.interact_pointer_pos()
        {
            let (time, _) = from_screen(pointer);
            // On the curve: the shape does not change until the key moves.
            let value = animation.keys.value_at(time).unwrap_or(setting.value);
            animation.keys.insert(Keyframe {
                time,
                value,
                ease: Ease::Linear,
            });
        }
        ui.add_space(2.0);
        if remove {
            with_animation(
                self.document.composition_mut(),
                slot,
                setting.track,
                |slot_animation| {
                    *slot_animation = None;
                },
            );
        } else if animation != before {
            with_animation(
                self.document.composition_mut(),
                slot,
                setting.track,
                |slot_animation| {
                    *slot_animation = Some(animation);
                },
            );
        }
    }
}
