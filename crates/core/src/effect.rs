//! Effects: image treatments chained on a clip, a layer or the whole
//! composition.
//!
//! The model only names an effect and holds its settings; what each kind
//! does and which parameters it has is up to the renderer.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One effect of a chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    /// Kind of effect, such as `blur`. A kind this build does not know is
    /// kept in the project and skipped when rendering.
    pub kind: String,
    /// Parameter values by name. A missing parameter takes its default.
    #[serde(default)]
    pub params: BTreeMap<String, f32>,
    /// When set, the effect is skipped.
    #[serde(default)]
    pub bypass: bool,
}

impl Effect {
    /// An effect of `kind` with every parameter at its default.
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            params: BTreeMap::new(),
            bypass: false,
        }
    }

    /// Value of a parameter, if it was set.
    pub fn param(&self, name: &str) -> Option<f32> {
        self.params.get(name).copied()
    }

    /// Sets a parameter. NaN is ignored.
    pub fn set_param(&mut self, name: impl Into<String>, value: f32) {
        if !value.is_nan() {
            self.params.insert(name.into(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_are_saved_by_name() {
        let mut effect = Effect::new("blur");
        assert_eq!(effect.param("radius"), None);
        effect.set_param("radius", 4.0);
        effect.set_param("radius", f32::NAN);
        assert_eq!(effect.param("radius"), Some(4.0));

        let json = serde_json::to_string(&effect).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"blur","params":{"radius":4.0},"bypass":false}"#
        );
        let read: Effect = serde_json::from_str(r#"{"kind":"future"}"#).unwrap();
        assert_eq!(read, Effect::new("future"));
    }
}
