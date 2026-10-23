//! Right panel: program preview, the effects of the composition, then the
//! properties of the group of the selected layer, of the layer and of the clip.

use egui::{RichText, Vec2};
use prism_core::{
    Animation, CUE_POINT_COUNT, Clip, ControlTarget, CrossfaderSide, Direction, EffectOwner,
    GroupId, LoopMode, MAX_SPEED, Slot, Unit,
};
use prism_engine::{Command, LayerStatus};
use prism_fx::{ISF_KIND, ParamInfo};

/// The opacity of a layer, described as a setting for its animation.
const OPACITY: ParamInfo = ParamInfo {
    id: "opacity",
    label: "Opacité",
    min: 0.0,
    max: 1.0,
    default: 1.0,
    choices: &[],
};

use crate::effects::{
    Tools, animation_settings, band_menu, dashboard_section, effect_section, generator_settings,
};
use crate::labels;
use crate::panel::{Action, ControlPanel, FrameInfo};
use crate::slider::Slider;
use crate::theme::{self, Emphasis};

impl ControlPanel {
    /// Settings of the selection: its group, its layer and its clip (their
    /// effects are in [`ControlPanel::effects_panel`]).
    pub(crate) fn settings_panel(&mut self, ui: &mut egui::Ui, frame: &FrameInfo<'_>) {
        egui::ScrollArea::vertical()
            .id_salt("settings panel")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let Some(slot) = self.selected else {
                    ui.label(RichText::new("Sélectionnez une case de la grille.").weak());
                    return;
                };
                let status = frame
                    .status
                    .layers
                    .iter()
                    .find(|s| s.layer == Some(slot.layer));
                let group = self
                    .document
                    .composition()
                    .layer(slot.layer)
                    .and_then(|layer| layer.group());
                if let Some(group) = group {
                    self.group_properties(ui, group);
                    ui.separator();
                }
                self.layer_properties(ui, slot);
                ui.separator();
                self.clip_properties(ui, slot, status);
            });
    }

    /// Chains of effects of the selection, from the clip out to the
    /// composition; an effect of the bank dropped on one joins it.
    pub(crate) fn effects_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("effects panel")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let selected = self.selected;
                let layer = selected.map(|slot| slot.layer);
                let group = layer.and_then(|layer| {
                    self.document
                        .composition()
                        .layer(layer)
                        .and_then(|layer| layer.group())
                });
                let mut owners = Vec::new();
                if let Some(slot) = selected
                    && self.document.composition().clip(slot).is_some()
                {
                    owners.push((EffectOwner::Clip { slot }, "Effets du clip"));
                }
                if let Some(layer) = layer {
                    owners.push((EffectOwner::Layer { layer }, "Effets de la couche"));
                }
                if let Some(group) = group {
                    owners.push((EffectOwner::Group { group }, "Effets du groupe"));
                }
                owners.push((EffectOwner::Composition, "Effets de la composition"));
                for (owner, title) in owners {
                    let dashboard = matches!(owner, EffectOwner::Clip { .. });
                    let response = ui
                        .scope(|ui| {
                            let Some(effects) = effects_in(self.document.composition_mut(), owner)
                            else {
                                return;
                            };
                            effect_section(
                                ui,
                                title,
                                owner,
                                effects,
                                dashboard,
                                Tools {
                                    presets: &mut self.presets,
                                    plugins: &mut self.plugins,
                                    learn: &mut self.learn,
                                },
                            );
                        })
                        .response;
                    if let Some(kind) = crate::bank::effect_dropped(ui, response.rect)
                        && let Some(effects) = self.effects_of(owner)
                        && let Some(info) = prism_fx::find(kind)
                    {
                        effects.push(info.create());
                    }
                    ui.separator();
                }
            });
    }

    /// The chain of effects of `owner`, if it still exists.
    pub(crate) fn effects_of(
        &mut self,
        owner: EffectOwner,
    ) -> Option<&mut Vec<prism_core::Effect>> {
        effects_in(self.document.composition_mut(), owner)
    }

    /// Settings and effects of the group of the selected layer.
    fn group_properties(&mut self, ui: &mut egui::Ui, id: GroupId) {
        let Some(group) = self.document.composition_mut().group_mut(id) else {
            return;
        };
        ui.label(RichText::new("Groupe").heading().semibold());
        egui::Grid::new("group properties")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Nom");
                ui.text_edit_singleline(&mut group.name);
                ui.end_row();

                ui.label("Opacité");
                let mut opacity = group.opacity.get();
                ui.add(Slider::new(&mut opacity, 0.0..=1.0).quarters());
                group.opacity = Unit::new(opacity);
                ui.end_row();

                ui.label("Fusion");
                egui::ComboBox::from_id_salt("inspector group blend")
                    .selected_text(labels::blend(group.blend_mode))
                    .show_ui(ui, |ui| {
                        for mode in prism_core::BlendMode::ALL {
                            ui.selectable_value(&mut group.blend_mode, mode, labels::blend(mode));
                        }
                    });
                ui.end_row();

                ui.label("");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut group.bypass, "Bypass");
                    ui.checkbox(&mut group.solo, "Solo");
                });
                ui.end_row();
            });
    }

    fn layer_properties(&mut self, ui: &mut egui::Ui, slot: Slot) {
        let Some(layer) = self.document.composition_mut().layer_mut(slot.layer) else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new("Couche").heading().semibold());
            self.presets
                .layer_menu(ui, egui::Id::new(("layer presets", slot.layer)), layer);
        });
        egui::Grid::new("layer properties")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Nom");
                ui.text_edit_singleline(&mut layer.name);
                ui.end_row();

                ui.label("Opacité");
                ui.horizontal(|ui| {
                    let mut opacity = layer.opacity.get();
                    let animated = layer.opacity_animation.is_some();
                    let driven = animated || layer.opacity_audio.is_some();
                    let slider =
                        ui.add_enabled(!driven, Slider::new(&mut opacity, 0.0..=1.0).quarters());
                    self.learn.cover(
                        ui,
                        slider.rect,
                        ControlTarget::LayerOpacity { layer: slot.layer },
                    );
                    layer.opacity = Unit::new(opacity);
                    let hint = if animated {
                        "Arrêter l'animation de l'opacité"
                    } else {
                        "Animer l'opacité : une onde entre deux valeurs, au tempo ou en secondes"
                    };
                    if ui
                        .selectable_label(animated, "~")
                        .on_hover_text(hint)
                        .clicked()
                    {
                        layer.opacity_animation = if animated {
                            None
                        } else {
                            Some(Animation::new(0.0, 1.0))
                        };
                    }
                    band_menu(ui, "opacity band", &mut layer.opacity_audio);
                });
                ui.end_row();
                if let Some(animation) = &mut layer.opacity_animation {
                    ui.label("");
                    ui.push_id("opacity animation", |ui| {
                        animation_settings(ui, &OPACITY, animation);
                    });
                    ui.end_row();
                }

                ui.label("Fusion");
                egui::ComboBox::from_id_salt("inspector blend")
                    .selected_text(labels::blend(layer.blend_mode))
                    .show_ui(ui, |ui| {
                        for mode in prism_core::BlendMode::ALL {
                            ui.selectable_value(&mut layer.blend_mode, mode, labels::blend(mode));
                        }
                    });
                ui.end_row();

                ui.label("Crossfader");
                ui.horizontal(|ui| {
                    for side in [None, Some(CrossfaderSide::A), Some(CrossfaderSide::B)] {
                        ui.selectable_value(&mut layer.crossfader_side, side, labels::side(side));
                    }
                });
                ui.end_row();

                ui.label("Transition");
                ui.add(
                    egui::DragValue::new(&mut layer.transition_ms)
                        .range(0..=10_000)
                        .speed(10.0)
                        .suffix(" ms"),
                )
                .on_hover_text("Durée du fondu quand la couche change de clip ; 0 = coupe franche");
                ui.end_row();

                ui.label("");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut layer.bypass, "Bypass");
                    ui.checkbox(&mut layer.solo, "Solo");
                });
                ui.end_row();
            });
    }

    fn clip_properties(&mut self, ui: &mut egui::Ui, slot: Slot, status: Option<&LayerStatus>) {
        ui.label(RichText::new("Clip").heading().semibold());
        let missing = self.is_missing(slot);
        if self.document.composition().clip(slot).is_none() {
            ui.label(RichText::new("Case vide.").weak());
            // Wrapped: the sources would make the inspector wider than its pane.
            ui.horizontal_wrapped(|ui| {
                if ui.button("Importer des médias…").clicked() {
                    self.import_dialog(Some(slot));
                }
                if ui
                    .button("Générateur…")
                    .on_hover_text("Choisir parmi les générateurs, par famille, avec un aperçu")
                    .clicked()
                {
                    self.open_generator_browser(slot);
                }
                egui::ComboBox::from_id_salt("new generator")
                    .selected_text("Shader…")
                    .show_ui(ui, |ui| {
                        let plugin = self.plugins.menu_items(ui, false);
                        ui.separator();
                        let picked = ui
                            .selectable_label(false, "Shader ISF…")
                            .on_hover_text("Une image dessinée par un fichier ISF (.fs)")
                            .clicked();
                        if let Some(path) =
                            plugin.or_else(|| picked.then(crate::effects::pick_isf_file).flatten())
                        {
                            let name = path.file_stem().map_or_else(
                                || "Shader".into(),
                                |stem| stem.to_string_lossy().into_owned(),
                            );
                            let mut clip = Clip::from_generator(name, ISF_KIND);
                            if let Some(generator) = clip.source.generator_mut() {
                                *generator = crate::effects::isf_effect(&path);
                            }
                            let _ = self.document.composition_mut().set_clip(slot, Some(clip));
                        }
                    })
                    .response
                    .on_hover_text("Une image dessinée par le GPU, sans fichier");
                // Live sources on a line of their own.
                ui.end_row();
                egui::ComboBox::from_id_salt("new camera")
                    .selected_text("Caméra…")
                    .show_ui(ui, |ui| {
                        let cameras = prism_engine::cameras();
                        if cameras.is_empty() {
                            ui.label(RichText::new("Aucune caméra").weak());
                        }
                        for (id, name) in cameras {
                            if ui.selectable_label(false, &name).clicked() {
                                let clip = Clip::from_camera(id, name);
                                let _ = self.document.composition_mut().set_clip(slot, Some(clip));
                            }
                        }
                    })
                    .response
                    .on_hover_text("L'image en direct d'une caméra ou d'une carte de capture");
                if cfg!(target_os = "macos") {
                    egui::ComboBox::from_id_salt("new syphon")
                        .selected_text("Syphon…")
                        .show_ui(ui, |ui| {
                            let servers = prism_engine::syphon_servers();
                            if servers.is_empty() {
                                ui.label(RichText::new("Aucune application ne publie").weak());
                            }
                            for (app, name) in servers {
                                let label = if name.is_empty() {
                                    app.clone()
                                } else {
                                    format!("{app} – {name}")
                                };
                                if ui.selectable_label(false, label).clicked() {
                                    let clip = Clip::from_syphon(app, name);
                                    let _ =
                                        self.document.composition_mut().set_clip(slot, Some(clip));
                                }
                            }
                        })
                        .response
                        .on_hover_text("L'image qu'une autre application publie sur Syphon");
                }
                if cfg!(target_os = "windows") {
                    egui::ComboBox::from_id_salt("new spout")
                        .selected_text("Spout…")
                        .show_ui(ui, |ui| {
                            let senders = prism_engine::spout_senders();
                            if senders.is_empty() {
                                ui.label(RichText::new("Aucune application ne publie").weak());
                            }
                            for name in senders {
                                if ui.selectable_label(false, &name).clicked() {
                                    let clip = Clip::from_spout(name);
                                    let _ =
                                        self.document.composition_mut().set_clip(slot, Some(clip));
                                }
                            }
                        })
                        .response
                        .on_hover_text("L'image qu'une autre application publie sur Spout");
                }
                egui::ComboBox::from_id_salt("new ndi")
                    .selected_text("NDI®…")
                    .show_ui(ui, |ui| match prism_engine::ndi_status() {
                        Err(reason) => {
                            ui.label(RichText::new(reason).weak());
                        }
                        Ok(_) => {
                            let sources = prism_engine::ndi_sources();
                            if sources.is_empty() {
                                ui.label(
                                    RichText::new("Recherche des sources du réseau…").weak(),
                                );
                            }
                            for name in sources {
                                if ui.selectable_label(false, &name).clicked() {
                                    let clip = Clip::from_ndi(name);
                                    let _ =
                                        self.document.composition_mut().set_clip(slot, Some(clip));
                                }
                            }
                        }
                    })
                    .response
                    .on_hover_text("L'image d'une source NDI® du réseau local (NDI® est une marque déposée de Vizrt NDI AB)");
            });
            return;
        }
        if let Some(clip) = self.document.composition_mut().clip_mut(slot)
            && clip.source.is_live()
        {
            let device = match &clip.source {
                prism_core::MediaSource::Camera { name, .. } => format!("Caméra : {name}"),
                prism_core::MediaSource::Syphon { app, name } => format!("Syphon : {app} – {name}"),
                prism_core::MediaSource::Ndi { name } => format!("NDI® : {name}"),
                prism_core::MediaSource::Spout { name } => format!("Spout : {name}"),
                _ => String::new(),
            };
            egui::Grid::new("camera clip")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Nom");
                    ui.text_edit_singleline(&mut clip.name);
                    ui.end_row();
                    ui.label("Source");
                    ui.label(device);
                    ui.end_row();
                });
            dashboard_section(ui, slot, &mut clip.dashboard, &mut self.learn);
            return;
        }
        let mut browse = false;
        if let Some(clip) = self.document.composition_mut().clip_mut(slot)
            && clip.source.generator().is_some()
        {
            egui::Grid::new("generated clip")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Nom");
                    ui.text_edit_singleline(&mut clip.name);
                    ui.end_row();
                });
            if let Some(generator) = clip.source.generator_mut() {
                browse = generator_settings(ui, generator, &mut self.learn, slot);
            }
            dashboard_section(ui, slot, &mut clip.dashboard, &mut self.learn);
            if browse {
                self.open_generator_browser(slot);
            }
            return;
        }
        let playing = status.filter(|status| status.slot == Some(slot));
        self.playhead(ui, slot, playing);

        let mut relocate = false;
        let timecode_now = self.timecode_seconds;
        let timecode_rate = self.timecode_rate();
        let Some(clip) = self.document.composition_mut().clip_mut(slot) else {
            return;
        };
        egui::Grid::new("clip properties")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Nom");
                ui.text_edit_singleline(&mut clip.name);
                ui.end_row();

                ui.label("Fichier");
                ui.vertical(|ui| {
                    let path = clip
                        .source
                        .path()
                        .map_or_else(String::new, |path| path.display().to_string());
                    ui.add(egui::Label::new(RichText::new(&path).small().monospace()).truncate())
                        .on_hover_text(path);
                    if missing {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Introuvable")
                                    .color(theme::palette(ui).danger_text),
                            );
                            relocate = ui.button("Localiser…").clicked();
                        });
                    }
                });
                ui.end_row();

                if let prism_core::MediaSource::ImageSequence { frame_rate, .. } = &mut clip.source
                {
                    ui.label("Images / s");
                    ui.add(
                        egui::DragValue::new(frame_rate)
                            .range(1.0..=120.0)
                            .speed(0.1),
                    )
                    .on_hover_text("Fréquence de la séquence d'images");
                    ui.end_row();
                }

                let playback = &mut clip.playback;
                ui.label("Sens");
                ui.horizontal(|ui| {
                    for direction in [
                        Direction::Forward,
                        Direction::Backward,
                        Direction::Random,
                        Direction::Paused,
                    ] {
                        ui.selectable_value(
                            &mut playback.direction,
                            direction,
                            labels::direction(direction),
                        );
                    }
                });
                ui.end_row();

                ui.label("Vitesse");
                ui.add(
                    Slider::new(&mut playback.speed, 0.0..=MAX_SPEED)
                        .logarithmic(true)
                        .suffix(" ×")
                        .snaps([1.0]),
                );
                ui.end_row();

                ui.label("Volume");
                let mut volume = playback.volume.get();
                ui.add(Slider::new(&mut volume, 0.0..=1.0).quarters())
                    .on_hover_text("Volume du son du clip ; l'opacité de la couche le règle aussi");
                playback.volume = Unit::new(volume);
                ui.end_row();

                ui.label("Timecode");
                ui.horizontal(|ui| {
                    let mut chased = playback.timecode_start.is_some();
                    if ui
                        .checkbox(&mut chased, "Calé")
                        .on_hover_text("La position de lecture suit le timecode (fenêtre Composition > Timecode, « Suivre le timecode ») : le début du média joue à l'heure de départ")
                        .changed()
                    {
                        playback.timecode_start = chased.then(|| timecode_now.unwrap_or(0.0));
                    }
                    if let Some(start) = &mut playback.timecode_start {
                        let id = ui.id().with("timecode start");
                        let mut text = ui
                            .data_mut(|data| data.get_temp::<String>(id))
                            .unwrap_or_else(|| {
                                prism_clock::Timecode::from_seconds(*start, timecode_rate).to_string()
                            });
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut text)
                                .desired_width(100.0)
                                .font(egui::TextStyle::Monospace),
                        );
                        if response.changed() {
                            if let Some(timecode) =
                                prism_clock::Timecode::parse(&text, timecode_rate)
                            {
                                *start = timecode.seconds();
                            }
                            ui.data_mut(|data| data.insert_temp(id, text));
                        } else if !response.has_focus() {
                            ui.data_mut(|data| data.remove::<String>(id));
                        }
                        if let Some(now) = timecode_now
                            && ui
                                .small_button("Maintenant")
                                .on_hover_text("Le début du média joue au timecode reçu à cet instant")
                                .clicked()
                        {
                            *start = now;
                        }
                    }
                });
                ui.end_row();

                ui.label("Fin");
                ui.horizontal(|ui| {
                    for mode in [LoopMode::Loop, LoopMode::PingPong, LoopMode::Once] {
                        ui.selectable_value(&mut playback.loop_mode, mode, labels::loop_mode(mode));
                    }
                });
                ui.end_row();

                ui.label("Entrée");
                ui.add(
                    egui::DragValue::new(&mut playback.in_point)
                        .range(0.0..=f64::MAX)
                        .speed(0.01)
                        .suffix(" s"),
                );
                ui.end_row();

                ui.label("Sortie");
                ui.horizontal(|ui| {
                    let mut to_end = playback.out_point.is_none();
                    if ui.checkbox(&mut to_end, "Fin du média").changed() {
                        playback.out_point = if to_end {
                            None
                        } else {
                            Some(playing.map_or(1.0, |s| s.duration))
                        };
                    }
                    if let Some(out_point) = &mut playback.out_point {
                        ui.add(
                            egui::DragValue::new(out_point)
                                .range(0.0..=f64::MAX)
                                .speed(0.01)
                                .suffix(" s"),
                        );
                    }
                });
                ui.end_row();
            });
        dashboard_section(ui, slot, &mut clip.dashboard, &mut self.learn);
        if relocate {
            self.relocate_dialog(slot);
        }

        ui.add_space(8.0);
        self.cue_points(ui, slot, playing);
    }

    /// Position of the clip when it plays: a slider to scratch.
    fn playhead(&mut self, ui: &mut egui::Ui, slot: Slot, playing: Option<&LayerStatus>) {
        let Some(status) = playing else {
            ui.label(RichText::new("À l'arrêt : cliquez la case pour le lancer.").weak());
            return;
        };
        ui.horizontal(|ui| {
            ui.label(format!(
                "{} / {}",
                labels::time(status.position),
                labels::time(status.duration)
            ));
            if let Some(frame) = status.frame {
                ui.label(
                    RichText::new(format!("image {}/{}", frame + 1, status.frame_count)).weak(),
                );
            }
        });
        let mut position = status.position;
        let response = ui.add(
            Slider::new(&mut position, 0.0..=status.duration.max(0.001))
                .show_value(false)
                .text("scratch"),
        );
        if response.dragged() || response.changed() {
            self.actions.push(Action::Engine(Command::Seek {
                layer: slot.layer,
                position,
                hold: response.dragged(),
            }));
        }
        if response.drag_stopped() {
            self.actions
                .push(Action::Engine(Command::Release { layer: slot.layer }));
        }
    }

    fn cue_points(&mut self, ui: &mut egui::Ui, slot: Slot, playing: Option<&LayerStatus>) {
        ui.label(RichText::new("Points de repère").semibold());
        let position = playing.map(|status| status.position);
        let Some(clip) = self.document.composition_mut().clip_mut(slot) else {
            return;
        };
        let mut jump = None;
        egui::Grid::new("cue points")
            .num_columns(4)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for index in 0..CUE_POINT_COUNT {
                    let cue = &mut clip.playback.cue_points[index];
                    ui.label(format!("{}", index + 1));
                    ui.label(cue.map_or_else(|| "–".to_owned(), labels::time));
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(position.is_some(), egui::Button::new("Placer"))
                            .on_hover_text("Place le repère à la position actuelle")
                            .clicked()
                        {
                            *cue = position;
                        }
                        if ui
                            .add_enabled(
                                cue.is_some() && position.is_some(),
                                egui::Button::new("Aller"),
                            )
                            .clicked()
                        {
                            jump = Some(index);
                        }
                        if ui
                            .add_enabled(cue.is_some(), egui::Button::new("×"))
                            .clicked()
                        {
                            *cue = None;
                        }
                    });
                    ui.end_row();
                }
            });
        if let Some(index) = jump {
            self.actions.push(Action::Engine(Command::JumpToCue {
                layer: slot.layer,
                index,
            }));
        }
    }
}

/// The program image, as large as its pane allows without distortion,
/// centred, with its size and outputs under it.
pub(crate) fn output_monitor(ui: &mut egui::Ui, frame: &FrameInfo<'_>) {
    ui.label(RichText::new("Programme").semibold());
    let Some((texture, resolution)) = frame.preview else {
        ui.label(RichText::new("Aperçu indisponible").weak());
        return;
    };
    let caption = format!(
        "{} × {}{}",
        resolution.width,
        resolution.height,
        match frame.output_screens.len() {
            0 => String::new(),
            1 => " · sortie ouverte".to_owned(),
            count => format!(" · {count} sorties ouvertes"),
        }
    );
    let caption_height = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
    fit_image(ui, texture, resolution, caption_height + 4.0);
    ui.label(RichText::new(caption).weak());
}

/// The chain of effects of `owner` in `composition`, if it still exists.
pub(crate) fn effects_in(
    composition: &mut prism_core::Composition,
    owner: EffectOwner,
) -> Option<&mut Vec<prism_core::Effect>> {
    match owner {
        EffectOwner::Composition => Some(&mut composition.effects),
        EffectOwner::Layer { layer } => {
            composition.layer_mut(layer).map(|layer| &mut layer.effects)
        }
        EffectOwner::Group { group } => {
            composition.group_mut(group).map(|group| &mut group.effects)
        }
        EffectOwner::Clip { slot } => composition.clip_mut(slot).map(|clip| &mut clip.effects),
        EffectOwner::Generator { .. } => None,
    }
}

/// `texture`, of `resolution`, as large as the room left in `ui` allows
/// (less `below`, kept for what follows) without distortion, centred.
pub(crate) fn fit_image(
    ui: &mut egui::Ui,
    texture: egui::TextureId,
    resolution: prism_core::Resolution,
    below: f32,
) {
    let room = Vec2::new(
        ui.available_width(),
        (ui.available_height() - below).max(16.0),
    );
    let aspect = resolution.width.max(1) as f32 / resolution.height.max(1) as f32;
    let size = if room.x / room.y > aspect {
        Vec2::new(room.y * aspect, room.y)
    } else {
        Vec2::new(room.x, room.x / aspect)
    };
    let (area, _) = ui.allocate_exact_size(Vec2::new(room.x, size.y), egui::Sense::hover());
    let image = egui::Rect::from_center_size(area.center(), size);
    egui::Image::from_texture(egui::load::SizedTexture::new(texture, size)).paint_at(ui, image);
}
