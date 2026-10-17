//! Right panel: program preview, the effects of the composition, then the
//! properties of the selected layer and clip.

use egui::{RichText, Vec2};
use prism_core::{
    Animation, CUE_POINT_COUNT, Clip, ControlTarget, CrossfaderSide, Direction, EffectOwner,
    LoopMode, MAX_SPEED, Slot, Unit,
};
use prism_engine::{Command, LayerStatus};
use prism_fx::{GENERATORS, ISF_KIND, ParamInfo};

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

impl ControlPanel {
    pub(crate) fn inspector(&mut self, ui: &mut egui::Ui, frame: &FrameInfo<'_>) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                preview(ui, frame);
                effect_section(
                    ui,
                    "Effets de la composition",
                    EffectOwner::Composition,
                    &mut self.document.composition_mut().effects,
                    false,
                    Tools {
                        presets: &mut self.presets,
                        learn: &mut self.learn,
                    },
                );
                ui.separator();
                let Some(slot) = self.selected else {
                    ui.label(RichText::new("Sélectionnez une case de la grille.").weak());
                    return;
                };
                let status = frame
                    .status
                    .layers
                    .iter()
                    .find(|s| s.layer == Some(slot.layer));
                self.layer_properties(ui, slot);
                ui.separator();
                self.clip_properties(ui, slot, status);
            });
    }

    fn layer_properties(&mut self, ui: &mut egui::Ui, slot: Slot) {
        let Some(layer) = self.document.composition_mut().layer_mut(slot.layer) else {
            return;
        };
        ui.horizontal(|ui| {
            ui.heading("Couche");
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
                        ui.add_enabled(!driven, egui::Slider::new(&mut opacity, 0.0..=1.0));
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
        effect_section(
            ui,
            "Effets de la couche",
            EffectOwner::Layer { layer: slot.layer },
            &mut layer.effects,
            true,
            Tools {
                presets: &mut self.presets,
                learn: &mut self.learn,
            },
        );
    }

    fn clip_properties(&mut self, ui: &mut egui::Ui, slot: Slot, status: Option<&LayerStatus>) {
        ui.heading("Clip");
        let missing = self.is_missing(slot);
        if self.document.composition().clip(slot).is_none() {
            ui.label(RichText::new("Case vide.").weak());
            ui.horizontal(|ui| {
                if ui.button("Importer des médias…").clicked() {
                    self.import_dialog(Some(slot));
                }
                egui::ComboBox::from_id_salt("new generator")
                    .selected_text("Générateur…")
                    .show_ui(ui, |ui| {
                        for info in GENERATORS {
                            if ui.selectable_label(false, info.label).clicked() {
                                let mut clip = Clip::from_generator(info.label, info.kind);
                                if let Some(generator) = clip.source.generator_mut() {
                                    *generator = info.create();
                                }
                                let _ = self.document.composition_mut().set_clip(slot, Some(clip));
                            }
                        }
                        ui.separator();
                        if ui
                            .selectable_label(false, "Shader ISF…")
                            .on_hover_text("Une image dessinée par un fichier ISF (.fs)")
                            .clicked()
                            && let Some(path) = crate::effects::pick_isf_file()
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
            });
            return;
        }
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
                generator_settings(ui, generator, &mut self.learn, slot);
            }
            dashboard_section(ui, slot, &mut clip.dashboard, &mut self.learn);
            effect_section(
                ui,
                "Effets du clip",
                EffectOwner::Clip { slot },
                &mut clip.effects,
                true,
                Tools {
                    presets: &mut self.presets,
                    learn: &mut self.learn,
                },
            );
            return;
        }
        let playing = status.filter(|status| status.slot == Some(slot));
        self.playhead(ui, slot, playing);

        let mut relocate = false;
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
                                    .color(egui::Color32::from_rgb(220, 80, 60)),
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
                    egui::Slider::new(&mut playback.speed, 0.0..=MAX_SPEED)
                        .logarithmic(true)
                        .suffix(" ×"),
                );
                ui.end_row();

                ui.label("Volume");
                let mut volume = playback.volume.get();
                ui.add(egui::Slider::new(&mut volume, 0.0..=1.0))
                    .on_hover_text("Volume du son du clip ; l'opacité de la couche le règle aussi");
                playback.volume = Unit::new(volume);
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
        effect_section(
            ui,
            "Effets du clip",
            EffectOwner::Clip { slot },
            &mut clip.effects,
            true,
            Tools {
                presets: &mut self.presets,
                learn: &mut self.learn,
            },
        );
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
            egui::Slider::new(&mut position, 0.0..=status.duration.max(0.001))
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
        ui.label(RichText::new("Points de repère").strong());
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

/// The program image, as wide as the panel.
fn preview(ui: &mut egui::Ui, frame: &FrameInfo<'_>) {
    ui.label(RichText::new("Programme").strong());
    let Some((texture, resolution)) = frame.preview else {
        ui.label(RichText::new("Aperçu indisponible").weak());
        return;
    };
    let width = ui.available_width();
    let height = width * resolution.height as f32 / resolution.width.max(1) as f32;
    ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(
        texture,
        Vec2::new(width, height),
    )));
    ui.label(
        RichText::new(format!(
            "{} × {}{}",
            resolution.width,
            resolution.height,
            if frame.output_screen.is_some() {
                " · sortie ouverte"
            } else {
                ""
            }
        ))
        .weak(),
    );
}
