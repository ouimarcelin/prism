//! The effects this build knows: names and parameters.

use prism_core::Effect;

/// Parameters an effect can have at most.
pub const MAX_PARAMS: usize = 8;

/// One setting of an effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamInfo {
    /// Name in project files.
    pub id: &'static str,
    /// Name shown in the interface.
    pub label: &'static str,
    /// Lowest value.
    pub min: f32,
    /// Highest value.
    pub max: f32,
    /// Value of a new effect.
    pub default: f32,
    /// For a choice between options, their names: the value is the index
    /// of the option. Empty for a continuous value.
    pub choices: &'static [&'static str],
}

impl ParamInfo {
    const fn range(
        id: &'static str,
        label: &'static str,
        min: f32,
        max: f32,
        default: f32,
    ) -> Self {
        Self {
            id,
            label,
            min,
            max,
            default,
            choices: &[],
        }
    }

    const fn choice(
        id: &'static str,
        label: &'static str,
        choices: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            label,
            min: 0.0,
            max: (choices.len() - 1) as f32,
            default: 0.0,
            choices,
        }
    }

    /// `value` brought within the range, rounded to an option for a choice.
    /// NaN gives the default.
    pub fn clamp(&self, value: f32) -> f32 {
        if value.is_nan() {
            return self.default;
        }
        let value = value.clamp(self.min, self.max);
        if self.choices.is_empty() {
            value
        } else {
            value.round()
        }
    }
}

/// Shader passes of an effect, by entry point, run one after the other.
pub(crate) type Passes = &'static [&'static str];

/// One kind of effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectInfo {
    /// Kind in project files, see [`Effect::kind`].
    pub kind: &'static str,
    /// Name shown in the interface.
    pub label: &'static str,
    /// Settings, in display order.
    pub params: &'static [ParamInfo],
    pub(crate) source: &'static str,
    pub(crate) passes: Passes,
}

impl EffectInfo {
    /// Values of the parameters of `effect`, in the order of
    /// [`EffectInfo::params`]: set values within their range, defaults for
    /// the others.
    pub fn values(&self, effect: &Effect) -> [f32; MAX_PARAMS] {
        let mut values = [0.0; MAX_PARAMS];
        for (value, param) in values.iter_mut().zip(self.params) {
            *value = param.clamp(effect.param(param.id).unwrap_or(param.default));
        }
        values
    }

    /// A new effect of this kind, every parameter at its default.
    pub fn create(&self) -> Effect {
        Effect::new(self.kind)
    }
}

const MIRROR_CHOICES: &[&str] = &[
    "Gauche sur droite",
    "Droite sur gauche",
    "Haut sur bas",
    "Bas sur haut",
];

/// Every effect, in display order.
pub const CATALOG: &[EffectInfo] = &[
    EffectInfo {
        kind: "color",
        label: "Couleur",
        params: &[
            ParamInfo::range("brightness", "Luminosité", -1.0, 1.0, 0.0),
            ParamInfo::range("contrast", "Contraste", 0.0, 3.0, 1.0),
            ParamInfo::range("saturation", "Saturation", 0.0, 3.0, 1.0),
            ParamInfo::range("hue", "Teinte", -180.0, 180.0, 0.0),
        ],
        source: include_str!("shaders/color.wgsl"),
        passes: &["fs_main"],
    },
    EffectInfo {
        kind: "invert",
        label: "Inversion",
        params: &[ParamInfo::range("amount", "Quantité", 0.0, 1.0, 1.0)],
        source: include_str!("shaders/invert.wgsl"),
        passes: &["fs_main"],
    },
    EffectInfo {
        kind: "transform",
        label: "Transformation",
        params: &[
            ParamInfo::range("x", "Position X", -1.0, 1.0, 0.0),
            ParamInfo::range("y", "Position Y", -1.0, 1.0, 0.0),
            ParamInfo::range("scale", "Échelle", 0.05, 4.0, 1.0),
            ParamInfo::range("rotation", "Rotation", -180.0, 180.0, 0.0),
        ],
        source: include_str!("shaders/transform.wgsl"),
        passes: &["fs_main"],
    },
    EffectInfo {
        kind: "mirror",
        label: "Miroir",
        params: &[ParamInfo::choice("mode", "Sens", MIRROR_CHOICES)],
        source: include_str!("shaders/mirror.wgsl"),
        passes: &["fs_main"],
    },
    EffectInfo {
        kind: "kaleidoscope",
        label: "Kaléidoscope",
        params: &[
            ParamInfo::range("segments", "Segments", 2.0, 16.0, 6.0),
            ParamInfo::range("rotation", "Rotation", -180.0, 180.0, 0.0),
            ParamInfo::range("zoom", "Zoom", 0.25, 4.0, 1.0),
        ],
        source: include_str!("shaders/kaleidoscope.wgsl"),
        passes: &["fs_main"],
    },
    EffectInfo {
        kind: "blur",
        label: "Flou",
        params: &[ParamInfo::range("radius", "Rayon", 0.0, 64.0, 8.0)],
        source: include_str!("shaders/blur.wgsl"),
        passes: &["fs_horizontal", "fs_vertical"],
    },
];

/// The effect of a kind, if this build knows it.
pub fn find(kind: &str) -> Option<&'static EffectInfo> {
    CATALOG.iter().find(|info| info.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_unique_and_parameters_fit() {
        for (index, info) in CATALOG.iter().enumerate() {
            assert!(info.params.len() <= MAX_PARAMS, "{}", info.kind);
            assert!(!info.passes.is_empty(), "{}", info.kind);
            assert_eq!(find(info.kind), Some(info));
            assert!(
                CATALOG[..index].iter().all(|other| other.kind != info.kind),
                "{}",
                info.kind
            );
            for param in info.params {
                assert!(param.min <= param.default && param.default <= param.max);
            }
        }
        assert_eq!(find("unknown"), None);
    }

    #[test]
    fn values_follow_the_effect_within_range() {
        let info = find("color").unwrap();
        let mut effect = info.create();
        effect.set_param("contrast", 9.0);
        effect.set_param("hue", -45.0);
        assert_eq!(&info.values(&effect)[..4], &[0.0, 3.0, 1.0, -45.0]);

        let mirror = find("mirror").unwrap();
        let mut effect = mirror.create();
        effect.set_param("mode", 2.4);
        assert_eq!(mirror.values(&effect)[0], 2.0);
    }
}
