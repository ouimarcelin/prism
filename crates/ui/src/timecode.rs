//! Timecode: cues launched when the MIDI timecode received passes their
//! time, and the window listing them.

use egui::RichText;
use prism_clock::{Timecode, TimecodeRate};
use prism_core::{Cue, CueTarget};

use crate::panel::{Action, ControlPanel};

impl ControlPanel {
    /// Follows the timecode received (`None` while none runs): the cues it
    /// passes launch their columns or clips.
    pub fn follow_timecode(&mut self, timecode: Option<Timecode>) -> Vec<Action> {
        let previous = self.timecode_seconds;
        self.timecode_seconds = timecode.map(|timecode| timecode.seconds());
        if let Some(timecode) = timecode {
            self.timecode_seen = Some(timecode);
        }
        let (Some(from), Some(to)) = (previous, self.timecode_seconds) else {
            return Vec::new();
        };
        let timeline = &self.document.composition().timeline;
        if !timeline.enabled {
            return Vec::new();
        }
        let passed = timeline.passed(from, to);
        if passed.is_empty() {
            return Vec::new();
        }
        let before = self.document.composition().clone();
        let composition = self.document.composition_mut();
        for cue in passed {
            match cue.target {
                CueTarget::Column { column } => {
                    let _ = composition.trigger_column(column);
                }
                CueTarget::Clip { slot } => {
                    let _ = composition.trigger_clip(slot);
                }
                CueTarget::StopAll => composition.stop_all(),
            }
        }
        self.document.commit_live(&before);
        std::mem::take(&mut self.actions)
    }

    /// Rate of the timecode received last, or 25 frames per second.
    fn timecode_rate(&self) -> TimecodeRate {
        self.timecode_seen
            .map_or(TimecodeRate::Fps25, |timecode| timecode.rate)
    }

    /// The cues, with the timecode received.
    pub(crate) fn timecode_window(&mut self, ctx: &egui::Context) {
        let mut open = self.dialogs.timecode;
        let rate = self.timecode_rate();
        let now = self.timecode_seconds;
        let columns: Vec<_> = self
            .document
            .composition()
            .columns()
            .iter()
            .map(|column| (column.id(), column.name.clone()))
            .collect();
        let selected = self.selected;
        egui::Window::new("Timecode")
            .open(&mut open)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                let timeline = &mut self.document.composition_mut().timeline;
                ui.checkbox(&mut timeline.enabled, "Suivre le timecode MIDI (MTC)")
                    .on_hover_text("Les repères se déclenchent quand le timecode reçu passe leur heure ; un saut ne déclenche rien");
                let reading = match now {
                    Some(seconds) => RichText::new(Timecode::from_seconds(seconds, rate).to_string())
                        .monospace()
                        .size(20.0),
                    None => RichText::new("Aucun timecode reçu").weak(),
                };
                ui.label(reading);
                ui.separator();
                let mut removed = None;
                let mut order: Vec<usize> = (0..timeline.cues.len()).collect();
                order.sort_by(|a, b| timeline.cues[*a].at.total_cmp(&timeline.cues[*b].at));
                egui::Grid::new("cues").num_columns(3).show(ui, |ui| {
                    for index in order {
                        let cue = &mut timeline.cues[index];
                        let id = ui.id().with(("cue", index));
                        let mut text = ui
                            .data_mut(|data| data.get_temp::<String>(id))
                            .unwrap_or_else(|| Timecode::from_seconds(cue.at, rate).to_string());
                        let response =
                            ui.add(egui::TextEdit::singleline(&mut text).desired_width(110.0).font(egui::TextStyle::Monospace));
                        if response.changed() {
                            if let Some(timecode) = Timecode::parse(&text, rate) {
                                cue.at = timecode.seconds();
                            }
                            ui.data_mut(|data| data.insert_temp(id, text));
                        } else if !response.has_focus() {
                            ui.data_mut(|data| data.remove::<String>(id));
                        }
                        let label = match cue.target {
                            CueTarget::Column { column } => columns
                                .iter()
                                .find(|(id, _)| *id == column)
                                .map_or_else(|| "colonne retirée".to_owned(), |(_, name)| name.clone()),
                            CueTarget::Clip { .. } => "Clip".to_owned(),
                            CueTarget::StopAll => "Tout arrêter".to_owned(),
                        };
                        egui::ComboBox::from_id_salt(("cue target", index))
                            .selected_text(label)
                            .show_ui(ui, |ui| {
                                for (column, name) in &columns {
                                    ui.selectable_value(
                                        &mut cue.target,
                                        CueTarget::Column { column: *column },
                                        name,
                                    );
                                }
                                if let Some(slot) = selected {
                                    ui.selectable_value(
                                        &mut cue.target,
                                        CueTarget::Clip { slot },
                                        "Clip sélectionné",
                                    );
                                }
                                ui.selectable_value(&mut cue.target, CueTarget::StopAll, "Tout arrêter");
                            });
                        if ui.small_button("×").on_hover_text("Retirer").clicked() {
                            removed = Some(index);
                        }
                        ui.end_row();
                    }
                });
                if let Some(index) = removed {
                    timeline.cues.remove(index);
                }
                if ui
                    .button("+ Repère")
                    .on_hover_text("Au timecode reçu, ou à 00:00:00:00 ; lance la première colonne, à changer")
                    .clicked()
                {
                    let target = columns
                        .first()
                        .map_or(CueTarget::StopAll, |(column, _)| CueTarget::Column { column: *column });
                    timeline.cues.push(Cue {
                        at: now.unwrap_or(0.0),
                        target,
                    });
                }
                ui.label(
                    RichText::new(format!(
                        "Heures en hh:mm:ss:ff à {} images/s ; le timecode vient des entrées MIDI écoutées (menu Contrôle).",
                        match rate {
                            TimecodeRate::Fps24 => "24",
                            TimecodeRate::Fps25 => "25",
                            TimecodeRate::Fps2997Drop => "29,97",
                            TimecodeRate::Fps30 => "30",
                        }
                    ))
                    .weak(),
                );
            });
        self.dialogs.timecode = open;
    }
}
