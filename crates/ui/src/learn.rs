//! MIDI learn: while active, every control that a controller can drive is
//! covered by a frame; clicking it picks the control, and the next message
//! of a controller is mapped to it.

use std::collections::HashMap;

use egui::{Color32, Rect, Sense, Stroke};
use prism_control::describe_source;
use prism_core::{ControlTarget, Mapping};

use crate::theme::{self, color};

/// State of the learn mode.
#[derive(Debug, Default)]
pub(crate) struct Learn {
    /// Whether the mode is on.
    pub(crate) active: bool,
    /// The control picked, waiting for a message.
    pub(crate) selected: Option<ControlTarget>,
    /// What each mapped control listens to, as written in the interface.
    mapped: HashMap<ControlTarget, String>,
}

/// Overlays: the accent on mapped controls, grey on free ones, the
/// warning colour on the control waiting for a message.
/// (Premultiplied: [`color::BRAND`], [`color::BG_TERTIARY`] and
/// [`color::WARNING`] at 43 %.)
const MAPPED: Color32 = Color32::from_rgba_premultiplied(5, 60, 100, 110);
const FREE: Color32 = Color32::from_rgba_premultiplied(29, 29, 29, 110);
const PICKED: Color32 = Color32::from_rgba_premultiplied(105, 83, 12, 110);

impl Learn {
    /// Reads which controls are mapped.
    pub(crate) fn refresh(&mut self, mappings: &[Mapping]) {
        self.mapped.clear();
        for mapping in mappings {
            self.mapped
                .entry(mapping.target.clone())
                .and_modify(|text| {
                    text.push_str(" ; ");
                    text.push_str(&describe_source(&mapping.source));
                })
                .or_insert_with(|| describe_source(&mapping.source));
        }
    }

    /// In learn mode, covers the control drawn in `rect`: a click picks
    /// `target` instead of using the control.
    pub(crate) fn cover(&mut self, ui: &egui::Ui, rect: Rect, target: ControlTarget) {
        if !self.active {
            return;
        }
        let response = ui.interact(rect, ui.id().with(("learn", &target)), Sense::click());
        let picked = self.selected.as_ref() == Some(&target);
        let mapped = self.mapped.get(&target);
        let fill = match (picked, mapped) {
            (true, _) => PICKED,
            (false, Some(_)) => MAPPED,
            (false, None) => FREE,
        };
        ui.painter().rect_filled(rect, theme::RADIUS, fill);
        if picked {
            ui.painter().rect_stroke(
                rect,
                theme::RADIUS,
                Stroke::new(2.0, color::WARNING),
                egui::StrokeKind::Inside,
            );
        }
        let hint = match mapped {
            Some(source) => format!("{source}\nCliquer puis bouger une commande MIDI pour changer"),
            None => "Cliquer puis bouger une commande MIDI".to_owned(),
        };
        if response.on_hover_text(hint).clicked() {
            self.selected = Some(target);
        }
    }
}
