//! Choosing a generator: its families, a search, a moving preview of
//! every generator and a large view of the one under the pointer.

use egui::{Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use prism_core::{Clip, Slot};
use prism_engine::{Command, preview_rect};
use prism_fx::{EffectInfo, GENERATOR_CATEGORIES, GENERATORS, generator_category};

use crate::panel::{Action, ControlPanel};
use crate::theme::{self, Emphasis};

/// Size of a preview in the grid.
const TILE: Vec2 = Vec2::new(160.0, 90.0);
/// Size of the large view.
const VIEW: Vec2 = Vec2::new(384.0, 216.0);

/// The generator browser while it is open.
pub(crate) struct GeneratorBrowser {
    /// The cell the chosen generator goes to.
    slot: Slot,
    /// Family shown; `None` for all of them.
    family: Option<usize>,
    search: String,
    /// The generator clicked once, shown in the large view.
    selected: Option<&'static EffectInfo>,
    /// The generator under the pointer, last frame.
    hovered: Option<&'static EffectInfo>,
}

impl GeneratorBrowser {
    /// A browser for the cell `slot`, on the family of its generator if it
    /// has one.
    pub(crate) fn new(slot: Slot, current: Option<&str>) -> Self {
        let selected = current.and_then(prism_fx::find_generator);
        let family = current.and_then(generator_category).and_then(|name| {
            GENERATOR_CATEGORIES
                .iter()
                .position(|(family, _)| *family == name)
        });
        Self {
            slot,
            family,
            search: String::new(),
            selected,
            hovered: None,
        }
    }
}

/// Generators of `family` (all for `None`) whose name holds `search`.
fn shown(family: Option<usize>, search: &str) -> Vec<(usize, &'static EffectInfo)> {
    let search = search.trim().to_lowercase();
    GENERATORS
        .iter()
        .enumerate()
        .filter(|(_, info)| {
            family.is_none_or(|family| {
                GENERATOR_CATEGORIES
                    .get(family)
                    .is_some_and(|(_, kinds)| kinds.contains(&info.kind))
            })
        })
        .filter(|(_, info)| search.is_empty() || info.label.to_lowercase().contains(&search))
        .collect()
}

/// Draws the preview of the generator at `index` of [`GENERATORS`] in
/// `rect`, or its name on a plain card without previews.
fn paint_preview(ui: &egui::Ui, previews: Option<egui::TextureId>, index: usize, rect: Rect) {
    let palette = theme::palette(ui);
    match previews {
        Some(texture) => {
            let [left, top, right, bottom] = preview_rect(index);
            ui.painter().image(
                texture,
                rect,
                Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom)),
                egui::Color32::WHITE,
            );
        }
        None => {
            ui.painter().rect_filled(rect, theme::RADIUS, palette.card);
        }
    }
}

impl ControlPanel {
    /// Opens the generator browser for the cell `slot`.
    pub(crate) fn open_generator_browser(&mut self, slot: Slot) {
        let current = self
            .document
            .composition()
            .clip(slot)
            .and_then(|clip| clip.source.generator())
            .map(|generator| generator.kind.clone());
        self.browser = Some(GeneratorBrowser::new(slot, current.as_deref()));
    }

    /// The generator browser, when open; asks the engine for the previews
    /// of the generators and effects, which the bank always shows.
    pub(crate) fn generator_browser(
        &mut self,
        ctx: &egui::Context,
        previews: Option<egui::TextureId>,
    ) {
        // The bank always shows them.
        let wanted = true;
        if wanted != self.previews_asked {
            self.previews_asked = wanted;
            self.actions
                .push(Action::Engine(Command::ShowGeneratorPreviews(wanted)));
        }
        let Some(browser) = &mut self.browser else {
            return;
        };
        let mut open = true;
        let mut picked = None;
        egui::Window::new("Générateurs")
            .id(egui::Id::new("generator browser"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([960.0, 660.0])
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                picked = browser_contents(ui, browser, previews);
            });
        if let Some(info) = picked {
            let slot = browser.slot;
            self.use_generator(slot, info);
            open = false;
        }
        if !open {
            self.browser = None;
        }
    }

    /// Puts generator `info` in cell `slot`: a new clip in an empty cell,
    /// the generator of the clip changed otherwise.
    pub(crate) fn use_generator(&mut self, slot: Slot, info: &'static EffectInfo) {
        let composition = self.document.composition_mut();
        match composition.clip_mut(slot) {
            Some(clip) => {
                let Some(generator) = clip.source.generator_mut() else {
                    return;
                };
                let before = prism_fx::find_generator(&generator.kind).map(|old| old.label);
                crate::effects::switch_generator(generator, info);
                // A clip still named after its generator follows the new one.
                if before.is_some_and(|before| clip.name == before) {
                    clip.name = info.label.to_owned();
                }
            }
            None => {
                let mut clip = Clip::from_generator(info.label, info.kind);
                if let Some(generator) = clip.source.generator_mut() {
                    *generator = info.create();
                }
                let _ = composition.set_clip(slot, Some(clip));
            }
        }
    }
}

/// The inside of the browser. Returns the generator chosen, by a double
/// click or the button.
fn browser_contents(
    ui: &mut egui::Ui,
    browser: &mut GeneratorBrowser,
    previews: Option<egui::TextureId>,
) -> Option<&'static EffectInfo> {
    let palette = theme::palette(ui);
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut browser.search)
                .hint_text("Rechercher un générateur…")
                .desired_width(260.0),
        );
        if previews.is_none() {
            ui.label(RichText::new("Aperçus en préparation…").weak());
        }
    });
    ui.separator();
    ui.horizontal_top(|ui| {
        // Families.
        ui.vertical(|ui| {
            ui.set_width(170.0);
            let all = format!("Tous ({})", GENERATORS.len());
            if ui.selectable_label(browser.family.is_none(), all).clicked() {
                browser.family = None;
            }
            for (index, (name, kinds)) in GENERATOR_CATEGORIES.iter().enumerate() {
                let label = format!("{name} ({})", kinds.len());
                if ui
                    .selectable_label(browser.family == Some(index), label)
                    .clicked()
                {
                    browser.family = Some(index);
                }
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            // Large view: the generator under the pointer, else the one
            // clicked.
            let shown_info = browser.hovered.or(browser.selected);
            ui.horizontal_top(|ui| {
                let (view, _) = ui.allocate_exact_size(VIEW, Sense::hover());
                match shown_info
                    .and_then(|info| GENERATORS.iter().position(|known| known.kind == info.kind))
                {
                    Some(index) => paint_preview(ui, previews, index, view),
                    None => {
                        ui.painter().rect_filled(view, theme::RADIUS, palette.card);
                    }
                }
                ui.vertical(|ui| {
                    match shown_info {
                        Some(info) => {
                            ui.label(RichText::new(info.label).heading().semibold());
                            if let Some(family) = generator_category(info.kind) {
                                ui.label(RichText::new(family).weak());
                            }
                            let own = info.common.unwrap_or(info.params.len());
                            ui.label(format!("{own} réglages propres"));
                            if info.common.is_some() {
                                ui.label(RichText::new("+ espace, temps et couleur").weak());
                            }
                            if info.memory {
                                ui.label(
                                    RichText::new("Garde son image d'un instant à l'autre").weak(),
                                );
                            }
                        }
                        None => {
                            ui.label(
                                RichText::new("Survolez un générateur pour le voir en grand.")
                                    .weak(),
                            );
                        }
                    }
                    ui.add_space(8.0);
                    if ui
                        .add_enabled(browser.selected.is_some(), egui::Button::new("Choisir"))
                        .on_hover_text("Ou double-cliquez sur un aperçu")
                        .clicked()
                    {
                        picked = browser.selected;
                    }
                });
            });
            ui.separator();
            browser.hovered = None;
            let list = shown(browser.family, &browser.search);
            if list.is_empty() {
                ui.label(RichText::new("Aucun générateur ne correspond.").weak());
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
                        for (index, info) in list {
                            let (rect, response) =
                                ui.allocate_exact_size(TILE + Vec2::new(0.0, 22.0), Sense::click());
                            let image = Rect::from_min_size(rect.min, TILE);
                            paint_preview(ui, previews, index, image);
                            ui.painter().text(
                                egui::pos2(rect.left() + 2.0, image.bottom() + 4.0),
                                egui::Align2::LEFT_TOP,
                                info.label,
                                egui::FontId::proportional(12.0),
                                palette.text,
                            );
                            let selected = browser.selected.is_some_and(|s| s.kind == info.kind);
                            if selected || response.hovered() {
                                ui.painter().rect_stroke(
                                    image,
                                    theme::RADIUS,
                                    Stroke::new(if selected { 2.0 } else { 1.0 }, palette.brand),
                                    StrokeKind::Outside,
                                );
                            }
                            if response.hovered() {
                                browser.hovered = Some(info);
                            }
                            if response.clicked() {
                                browser.selected = Some(info);
                            }
                            if response.double_clicked() {
                                picked = Some(info);
                            }
                        }
                    });
                });
        });
    });
    picked
}
