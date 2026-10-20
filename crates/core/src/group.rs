//! Groups: neighbouring layers mixed together into an image of their own,
//! which goes through the group's effects and is then mixed with the other
//! layers like one layer.

use serde::{Deserialize, Serialize};

use crate::{BlendMode, Effect, GroupId, Unit};

/// Settings of a group of layers. Which layers belong to it is held by the
/// layers themselves ([`crate::Layer::group`]); they always sit next to one
/// another in the stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub(crate) id: GroupId,
    /// Name shown in the interface.
    pub name: String,
    /// Opacity of the mixed group.
    pub opacity: Unit,
    /// Blend mode of the mixed group against the layers below it.
    #[serde(default)]
    pub blend_mode: BlendMode,
    /// When set, no layer of the group is rendered.
    #[serde(default)]
    pub bypass: bool,
    /// When any layer or group is solo, only solo layers and the layers of
    /// solo groups are rendered.
    #[serde(default)]
    pub solo: bool,
    /// Effects applied to the mixed group, first to last.
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// Whether the grid hides the layers of the group.
    #[serde(default)]
    pub collapsed: bool,
}

impl Group {
    pub(crate) fn new(id: GroupId, name: String) -> Self {
        Self {
            id,
            name,
            opacity: Unit::ONE,
            blend_mode: BlendMode::default(),
            bypass: false,
            solo: false,
            effects: Vec::new(),
            collapsed: false,
        }
    }

    /// Stable identifier of the group.
    pub fn id(&self) -> GroupId {
        self.id
    }
}
