//! Stable identifiers for layers and columns.
//!
//! Identifiers never change when layers or columns are reordered, so the
//! rest of the application can keep references across edits.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Stable identifier of a layer within a composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerId(pub(crate) u32);

/// Stable identifier of a column within a composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ColumnId(pub(crate) u32);

/// Stable identifier of a group of layers within a composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GroupId(pub(crate) u32);

/// A cell of the clip grid: one layer, one column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Slot {
    /// Row of the grid.
    pub layer: LayerId,
    /// Column of the grid.
    pub column: ColumnId,
}

impl Slot {
    /// Creates a slot from a layer and a column.
    pub fn new(layer: LayerId, column: ColumnId) -> Self {
        Self { layer, column }
    }
}

impl fmt::Display for LayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "layer #{}", self.0)
    }
}

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "group #{}", self.0)
    }
}

impl fmt::Display for ColumnId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "column #{}", self.0)
    }
}
