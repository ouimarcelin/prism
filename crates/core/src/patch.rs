//! A patch: an image made by a graph of nodes, generators, effects and
//! mixes wired together, ending at one output.
//!
//! The model only holds the graph; the engine draws it, node by node, in
//! the order [`Patch::order`] gives.

use serde::{Deserialize, Serialize};

use crate::effect::Effect;
use crate::layer::BlendMode;
use crate::value::Unit;

/// Identifier of a node, unique within its patch.
pub type NodeId = u32;

/// What a node does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeKind {
    /// Draws an image of its own: a generator, its settings and
    /// animations. No input.
    Generator {
        /// The generator.
        generator: Effect,
    },
    /// Changes the image wired to its input.
    Effect {
        /// The effect.
        effect: Effect,
    },
    /// Lays its second input over its first.
    Mix {
        /// How the second input is laid over the first.
        #[serde(default)]
        blend: BlendMode,
        /// Opacity of the second input.
        amount: Unit,
    },
    /// The image of the patch: what is wired to its input.
    Output,
}

impl NodeKind {
    /// Number of inputs.
    pub fn inputs(&self) -> usize {
        match self {
            Self::Generator { .. } => 0,
            Self::Effect { .. } | Self::Output => 1,
            Self::Mix { .. } => 2,
        }
    }

    /// Whether the node gives an image to wire on.
    pub fn has_output(&self) -> bool {
        !matches!(self, Self::Output)
    }
}

/// A node and where it sits in the editor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Identifier, unique within the patch.
    pub id: NodeId,
    /// What it does.
    #[serde(flatten)]
    pub kind: NodeKind,
    /// Place in the editor, in points.
    #[serde(default)]
    pub position: [f32; 2],
}

/// A wire: the image of node `from` into input `input` of node `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Link {
    /// Node giving the image.
    pub from: NodeId,
    /// Node taking it.
    pub to: NodeId,
    /// Input of `to` it goes to.
    pub input: u8,
}

/// The graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    /// The nodes, one of them the output.
    pub nodes: Vec<Node>,
    /// The wires, at most one per input.
    #[serde(default)]
    pub links: Vec<Link>,
    /// Identifier for the next node.
    #[serde(default)]
    pub next_id: NodeId,
}

impl Default for Patch {
    /// A generator wired to the output.
    fn default() -> Self {
        let mut patch = Self {
            nodes: Vec::new(),
            links: Vec::new(),
            next_id: 0,
        };
        let output = patch.add(NodeKind::Output, [520.0, 120.0]);
        let source = patch.add(
            NodeKind::Generator {
                generator: Effect::new("plasma"),
            },
            [80.0, 100.0],
        );
        patch.connect(source, output, 0);
        patch
    }
}

impl Patch {
    /// Adds a node; returns its identifier.
    pub fn add(&mut self, kind: NodeKind, position: [f32; 2]) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(Node { id, kind, position });
        id
    }

    /// The node `id`.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// The node `id`, mutable.
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }

    /// Removes node `id` and its wires. The output stays.
    pub fn remove(&mut self, id: NodeId) {
        if self
            .node(id)
            .is_some_and(|node| node.kind == NodeKind::Output)
        {
            return;
        }
        self.nodes.retain(|node| node.id != id);
        self.links.retain(|link| link.from != id && link.to != id);
    }

    /// Wires `from` into input `input` of `to`, replacing what was wired
    /// there. Refused (returns `false`) when an end is missing, the input
    /// does not exist, `from` gives no image, or the wire would make a loop.
    pub fn connect(&mut self, from: NodeId, to: NodeId, input: u8) -> bool {
        let (Some(source), Some(target)) = (self.node(from), self.node(to)) else {
            return false;
        };
        if !source.kind.has_output() || usize::from(input) >= target.kind.inputs() || from == to {
            return false;
        }
        if self.reaches(to, from) {
            return false;
        }
        self.links
            .retain(|link| !(link.to == to && link.input == input));
        self.links.push(Link { from, to, input });
        true
    }

    /// Removes the wire into input `input` of `to`.
    pub fn disconnect(&mut self, to: NodeId, input: u8) {
        self.links
            .retain(|link| !(link.to == to && link.input == input));
    }

    /// What is wired into input `input` of `to`.
    pub fn source_of(&self, to: NodeId, input: u8) -> Option<NodeId> {
        self.links
            .iter()
            .find(|link| link.to == to && link.input == input)
            .map(|link| link.from)
    }

    /// Whether the image of `from` flows, wire after wire, into `to`.
    fn reaches(&self, from: NodeId, to: NodeId) -> bool {
        let mut stack = vec![from];
        let mut seen = Vec::new();
        while let Some(node) = stack.pop() {
            if node == to {
                return true;
            }
            if seen.contains(&node) {
                continue;
            }
            seen.push(node);
            stack.extend(
                self.links
                    .iter()
                    .filter(|link| link.from == node)
                    .map(|link| link.to),
            );
        }
        false
    }

    /// The output node.
    pub fn output(&self) -> Option<NodeId> {
        self.nodes
            .iter()
            .find(|node| node.kind == NodeKind::Output)
            .map(|node| node.id)
    }

    /// The nodes the output needs, each after those wired into it: the
    /// order to draw them in. Nodes not leading to the output are left out.
    pub fn order(&self) -> Vec<NodeId> {
        let mut order = Vec::new();
        if let Some(output) = self.output() {
            self.visit(output, &mut order, 0);
        }
        order
    }

    fn visit(&self, node: NodeId, order: &mut Vec<NodeId>, depth: usize) {
        if order.contains(&node) || depth > self.nodes.len() {
            return;
        }
        let inputs = self.node(node).map_or(0, |node| node.kind.inputs());
        for input in 0..inputs {
            if let Some(source) = self.source_of(node, input as u8) {
                self.visit(source, order, depth + 1);
            }
        }
        order.push(node);
    }

    /// Every effect of the patch (generators included), mutable: for
    /// presets, missing files and the like.
    pub fn effects_mut(&mut self) -> impl Iterator<Item = &mut Effect> {
        self.nodes
            .iter_mut()
            .filter_map(|node| match &mut node.kind {
                NodeKind::Generator { generator } => Some(generator),
                NodeKind::Effect { effect } => Some(effect),
                _ => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wires_refuse_loops_and_order_follows_them() {
        let mut patch = Patch::default();
        let output = patch.output().unwrap();
        let plasma = patch.source_of(output, 0).unwrap();
        let blur = patch.add(
            NodeKind::Effect {
                effect: Effect::new("blur"),
            },
            [0.0, 0.0],
        );
        let lines = patch.add(
            NodeKind::Generator {
                generator: Effect::new("lines"),
            },
            [0.0, 0.0],
        );
        let mix = patch.add(
            NodeKind::Mix {
                blend: BlendMode::Add,
                amount: Unit::new(0.5),
            },
            [0.0, 0.0],
        );
        assert!(patch.connect(plasma, blur, 0));
        assert!(patch.connect(blur, mix, 0));
        assert!(patch.connect(lines, mix, 1));
        assert!(patch.connect(mix, output, 0));
        // A generator has no input, the output no output, no loops.
        assert!(!patch.connect(blur, lines, 0));
        assert!(!patch.connect(output, blur, 0));
        assert!(!patch.connect(mix, blur, 0));
        // Each node after what feeds it.
        let order = patch.order();
        assert_eq!(order.last(), Some(&output));
        let at = |id| order.iter().position(|&node| node == id).unwrap();
        assert!(at(plasma) < at(blur) && at(blur) < at(mix) && at(lines) < at(mix));
        // The output stays; a removed node takes its wires with it.
        patch.remove(output);
        assert!(patch.output().is_some());
        patch.remove(blur);
        assert_eq!(patch.source_of(mix, 0), None);
        // Saved and read back.
        let json = serde_json::to_string(&patch).unwrap();
        assert_eq!(serde_json::from_str::<Patch>(&json).unwrap(), patch);
    }
}
