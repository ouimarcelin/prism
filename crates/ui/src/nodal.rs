//! The node editor: the patch of the selected clip, its generators,
//! effects and mixes wired together, drawn by `egui-snarl`.
//!
//! The patch in the composition is what counts; the editor's graph is
//! rebuilt from it whenever they differ (another clip, an undo), and every
//! change made in the editor goes to the patch at once.

use std::collections::HashMap;

use egui::{Pos2, RichText};
use egui_snarl::ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer};
use egui_snarl::{InPin, NodeId as SnarlId, OutPin, Snarl};
use prism_core::{BlendMode, Clip, Effect, NodeId, NodeKind, Patch, Slot, Unit};
use prism_fx::{
    CATALOG, EFFECT_CATEGORIES, GENERATOR_CATEGORIES, GENERATORS, find, find_generator,
};

use crate::panel::{ControlPanel, TopView};
use crate::slider::Slider;

/// The editor of one patch.
pub(crate) struct NodalView {
    slot: Slot,
    snarl: Snarl<NodeId>,
    /// Nodes and wires of the patch when the graph was last in step.
    signature: Signature,
}

type Signature = (Vec<NodeId>, Vec<(NodeId, NodeId, u8)>);

fn signature(patch: &Patch) -> Signature {
    let mut nodes: Vec<NodeId> = patch.nodes.iter().map(|node| node.id).collect();
    nodes.sort_unstable();
    let mut links: Vec<_> = patch
        .links
        .iter()
        .map(|link| (link.from, link.to, link.input))
        .collect();
    links.sort_unstable();
    (nodes, links)
}

/// The editor's graph for `patch`.
fn build(slot: Slot, patch: &Patch) -> NodalView {
    let mut snarl = Snarl::new();
    let mut ids = HashMap::new();
    for node in &patch.nodes {
        let id = snarl.insert_node(Pos2::new(node.position[0], node.position[1]), node.id);
        ids.insert(node.id, id);
    }
    for link in &patch.links {
        if let (Some(&from), Some(&to)) = (ids.get(&link.from), ids.get(&link.to)) {
            snarl.connect(
                egui_snarl::OutPinId {
                    node: from,
                    output: 0,
                },
                egui_snarl::InPinId {
                    node: to,
                    input: usize::from(link.input),
                },
            );
        }
    }
    NodalView {
        slot,
        snarl,
        signature: signature(patch),
    }
}

/// Name of a node.
fn title(kind: &NodeKind) -> String {
    match kind {
        NodeKind::Generator { generator } => find_generator(&generator.kind)
            .map_or("Générateur", |info| info.label)
            .to_owned(),
        NodeKind::Effect { effect } => find(&effect.kind)
            .map_or("Effet", |info| info.label)
            .to_owned(),
        NodeKind::Mix { .. } => "Mélange".to_owned(),
        NodeKind::Output => "Sortie du patch".to_owned(),
    }
}

/// What the editor works on: the patch, changed in place.
struct PatchViewer<'a> {
    patch: &'a mut Patch,
}

impl PatchViewer<'_> {
    fn kind(&self, node: NodeId) -> Option<&NodeKind> {
        self.patch.node(node).map(|node| &node.kind)
    }
}

impl SnarlViewer<NodeId> for PatchViewer<'_> {
    fn title(&mut self, node: &NodeId) -> String {
        self.kind(*node).map_or_else(String::new, title)
    }

    fn inputs(&mut self, node: &NodeId) -> usize {
        self.kind(*node).map_or(0, NodeKind::inputs)
    }

    fn outputs(&mut self, node: &NodeId) -> usize {
        usize::from(self.kind(*node).is_some_and(NodeKind::has_output))
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<NodeId>,
    ) -> impl SnarlPin + 'static {
        let node = snarl[pin.id.node];
        let label = match (self.kind(node), pin.id.input) {
            (Some(NodeKind::Mix { .. }), 0) => "Fond",
            (Some(NodeKind::Mix { .. }), _) => "Dessus",
            _ => "Image",
        };
        ui.label(label);
        PinInfo::circle().with_fill(egui::Color32::from_rgb(13, 153, 255))
    }

    fn show_output(
        &mut self,
        _pin: &OutPin,
        ui: &mut egui::Ui,
        _snarl: &mut Snarl<NodeId>,
    ) -> impl SnarlPin + 'static {
        ui.label("Image");
        PinInfo::circle().with_fill(egui::Color32::from_rgb(13, 153, 255))
    }

    fn has_body(&mut self, node: &NodeId) -> bool {
        !matches!(self.kind(*node), Some(NodeKind::Output) | None)
    }

    fn show_body(
        &mut self,
        node: SnarlId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<NodeId>,
    ) {
        let id = snarl[node];
        let Some(node) = self.patch.node_mut(id) else {
            return;
        };
        ui.push_id(id, |ui| {
            ui.set_max_width(230.0);
            match &mut node.kind {
                NodeKind::Generator { generator: effect } | NodeKind::Effect { effect } => {
                    effect_body(ui, effect);
                }
                NodeKind::Mix { blend, amount } => {
                    egui::ComboBox::from_id_salt("blend")
                        .selected_text(crate::labels::blend(*blend))
                        .show_ui(ui, |ui| {
                            for mode in BlendMode::ALL {
                                ui.selectable_value(blend, mode, crate::labels::blend(mode));
                            }
                        });
                    let mut value = amount.get();
                    ui.add(Slider::new(&mut value, 0.0..=1.0).quarters());
                    *amount = Unit::new(value);
                }
                NodeKind::Output => {}
            }
        });
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<NodeId>) {
        let (source, target) = (snarl[from.id.node], snarl[to.id.node]);
        let Ok(input) = u8::try_from(to.id.input) else {
            return;
        };
        if self.patch.connect(source, target, input) {
            for &remote in &to.remotes {
                snarl.disconnect(remote, to.id);
            }
            snarl.connect(from.id, to.id);
        }
    }

    fn disconnect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<NodeId>) {
        if let Ok(input) = u8::try_from(to.id.input) {
            self.patch.disconnect(snarl[to.id.node], input);
        }
        snarl.disconnect(from.id, to.id);
    }

    fn has_graph_menu(&mut self, _pos: Pos2, _snarl: &mut Snarl<NodeId>) -> bool {
        true
    }

    fn show_graph_menu(&mut self, pos: Pos2, ui: &mut egui::Ui, snarl: &mut Snarl<NodeId>) {
        ui.label(RichText::new("Ajouter").weak());
        let mut added = None;
        ui.menu_button("Générateur", |ui| {
            for (family, kinds) in GENERATOR_CATEGORIES {
                ui.menu_button(*family, |ui| {
                    for kind in *kinds {
                        if let Some(info) = GENERATORS.iter().find(|info| info.kind == *kind)
                            && ui.button(info.label).clicked()
                        {
                            added = Some(NodeKind::Generator {
                                generator: info.create(),
                            });
                        }
                    }
                });
            }
        });
        ui.menu_button("Effet", |ui| {
            for (family, kinds) in EFFECT_CATEGORIES {
                ui.menu_button(*family, |ui| {
                    for kind in *kinds {
                        if let Some(info) = CATALOG.iter().find(|info| info.kind == *kind)
                            && ui.button(info.label).clicked()
                        {
                            added = Some(NodeKind::Effect {
                                effect: info.create(),
                            });
                        }
                    }
                });
            }
        });
        if ui.button("Mélange").clicked() {
            added = Some(NodeKind::Mix {
                blend: BlendMode::Normal,
                amount: Unit::new(1.0),
            });
        }
        if let Some(kind) = added {
            let id = self.patch.add(kind, [pos.x, pos.y]);
            snarl.insert_node(pos, id);
            ui.close();
        }
    }

    fn has_node_menu(&mut self, node: &NodeId) -> bool {
        !matches!(self.kind(*node), Some(NodeKind::Output) | None)
    }

    fn show_node_menu(
        &mut self,
        node: SnarlId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<NodeId>,
    ) {
        if ui.button("Supprimer le nœud").clicked() {
            self.patch.remove(snarl[node]);
            snarl.remove_node(node);
            ui.close();
        }
    }
}

/// The settings of a generator or effect node: its own (not the shared
/// space, time and colour of the bank), as compact sliders and lists.
fn effect_body(ui: &mut egui::Ui, effect: &mut Effect) {
    let Ok(info) = crate::effects::describe(effect) else {
        ui.label(RichText::new("Inconnu").weak());
        return;
    };
    let own = info.common.unwrap_or(info.params.len());
    egui::Grid::new("settings")
        .num_columns(2)
        .spacing([6.0, 3.0])
        .show(ui, |ui| {
            for param in &info.params[..own] {
                ui.label(RichText::new(param.label).small());
                let mut value = param.clamp(effect.param(param.id).unwrap_or(param.default));
                if param.choices.is_empty() {
                    ui.add(Slider::new(&mut value, param.min..=param.max));
                } else {
                    let index = value as usize;
                    egui::ComboBox::from_id_salt(param.id)
                        .selected_text(param.choices.get(index).copied().unwrap_or("?"))
                        .show_ui(ui, |ui| {
                            for (choice, label) in param.choices.iter().enumerate() {
                                if ui.selectable_label(choice == index, *label).clicked() {
                                    value = choice as f32;
                                }
                            }
                        });
                }
                if (value - param.clamp(effect.param(param.id).unwrap_or(param.default))).abs()
                    > f32::EPSILON
                {
                    effect.set_param(param.id, value);
                }
                ui.end_row();
            }
        });
}

impl ControlPanel {
    /// The node editor of the selected cell.
    pub(crate) fn nodal_panel(&mut self, ui: &mut egui::Ui) {
        let Some(slot) = self.selected else {
            ui.label(RichText::new("Sélectionnez une case pour éditer son patch.").weak());
            return;
        };
        let composition = self.document.composition_mut();
        let Some(clip) = composition.clip_mut(slot) else {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Case vide.").weak());
                if ui.button("Créer un patch ici").clicked() {
                    let _ = composition
                        .set_clip(slot, Some(Clip::from_patch("Patch", Patch::default())));
                }
            });
            return;
        };
        let name = clip.name.clone();
        let Some(patch) = clip.source.patch_mut() else {
            ui.label(
                RichText::new(format!(
                    "« {name} » n'est pas un patch : choisissez une case vide pour en créer un."
                ))
                .weak(),
            );
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new("Patch").heading());
            ui.label(RichText::new(&name).weak());
            ui.label(RichText::new("Clic droit sur le fond : ajouter un nœud · tirer d'une sortie à une entrée : câbler · clic droit sur un nœud : supprimer").weak().small());
        });
        let stale = self
            .nodal
            .as_ref()
            .is_none_or(|view| view.slot != slot || view.signature != signature(patch));
        if stale {
            self.nodal = Some(build(slot, patch));
        }
        let Some(view) = &mut self.nodal else {
            return;
        };
        let style = SnarlStyle::new();
        let mut viewer = PatchViewer { patch };
        view.snarl.show(&mut viewer, &style, ("patch", slot), ui);
        // Where the nodes were dragged, kept in the patch.
        for (_, info) in view.snarl.nodes_ids_data() {
            if let Some(node) = viewer.patch.node_mut(info.value) {
                node.position = [info.pos.x, info.pos.y];
            }
        }
        view.signature = signature(viewer.patch);
    }

    /// Creates a patch in the empty cell `slot` and shows its editor.
    pub(crate) fn new_patch(&mut self, slot: Slot) {
        let _ = self
            .document
            .composition_mut()
            .set_clip(slot, Some(Clip::from_patch("Patch", Patch::default())));
        self.top_view = TopView::Nodal;
    }
}
