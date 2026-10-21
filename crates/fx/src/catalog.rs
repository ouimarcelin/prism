//! The effects this build knows: names and parameters.

use prism_core::Effect;

use crate::FrameTime;
use crate::text::TEXT_KIND;

/// Parameters an effect can have at most.
pub const MAX_PARAMS: usize = 32;

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
    pub(crate) const fn range(
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

    pub(crate) const fn choice(
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

    /// The same setting with another default value.
    pub(crate) const fn with_default(mut self, default: f32) -> Self {
        self.default = default;
        self
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

/// The settings of a generator of the procedural bank: its own, then the
/// shared ones of [`COMMON_PARAMS`].
macro_rules! bank_params {
    ($($param:expr),* $(,)?) => {
        &[
            $($param,)*
            COMMON_PARAMS[0], COMMON_PARAMS[1], COMMON_PARAMS[2], COMMON_PARAMS[3],
            COMMON_PARAMS[4], COMMON_PARAMS[5], COMMON_PARAMS[6], COMMON_PARAMS[7],
            COMMON_PARAMS[8], COMMON_PARAMS[9], COMMON_PARAMS[10], COMMON_PARAMS[11],
            COMMON_PARAMS[12], COMMON_PARAMS[13], COMMON_PARAMS[14], COMMON_PARAMS[15],
        ]
    };
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
    /// For generators that draw a text: the text of a new one.
    pub text: Option<&'static str>,
    pub(crate) source: &'static str,
    pub(crate) passes: Passes,
    /// For generators of the procedural bank: index of the first of the
    /// shared settings ([`COMMON_PARAMS`]), after the generator's own.
    pub common: Option<usize>,
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

    /// Values of the parameters of `effect` at `time`: like
    /// [`EffectInfo::values`], with animated parameters at their current
    /// value (envelopes from the launch of the clip), parameters following
    /// a band of the sound at its level in their range, and parameters
    /// linked to a slider of the dashboard at the slider's place in their
    /// range.
    pub fn values_at(&self, effect: &Effect, time: FrameTime) -> [f32; MAX_PARAMS] {
        let mut values = self.values(effect);
        for (value, param) in values.iter_mut().zip(self.params) {
            if let Some(animation) = effect.animations.get(param.id) {
                *value = param.clamp(animation.value_at(time.moment()));
            } else if let Some(band) = effect.audio.get(param.id) {
                let level = time.audio[band.index()];
                *value = param.clamp(param.min + (param.max - param.min) * level);
            } else if let Some(slider) = effect
                .links
                .get(param.id)
                .and_then(|index| time.dashboard.get(*index))
            {
                *value = param.clamp(param.min + (param.max - param.min) * slider);
            }
        }
        values
    }

    /// Number of full-screen passes the effect runs.
    pub fn passes(&self) -> usize {
        self.passes.len()
    }

    /// A new effect of this kind, every parameter at its default.
    pub fn create(&self) -> Effect {
        let mut effect = Effect::new(self.kind);
        effect.text = self.text.unwrap_or_default().into();
        effect
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
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "invert",
        label: "Inversion",
        params: &[ParamInfo::range("amount", "Quantité", 0.0, 1.0, 1.0)],
        source: include_str!("shaders/invert.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
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
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "mirror",
        label: "Miroir",
        params: &[ParamInfo::choice("mode", "Sens", MIRROR_CHOICES)],
        source: include_str!("shaders/mirror.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
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
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "blur",
        label: "Flou",
        params: &[ParamInfo::range("radius", "Rayon", 0.0, 64.0, 8.0)],
        source: include_str!("shaders/blur.wgsl"),
        passes: &["fs_horizontal", "fs_vertical"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "pixelate",
        label: "Pixellisation",
        params: &[ParamInfo::range("size", "Taille", 1.0, 128.0, 16.0)],
        source: include_str!("shaders/pixelate.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "posterize",
        label: "Postérisation",
        params: &[ParamInfo::range("levels", "Niveaux", 2.0, 32.0, 6.0)],
        source: include_str!("shaders/posterize.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "threshold",
        label: "Seuil",
        params: &[
            ParamInfo::range("level", "Niveau", 0.0, 1.0, 0.5),
            ParamInfo::range("softness", "Douceur", 0.0, 0.5, 0.02),
        ],
        source: include_str!("shaders/threshold.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "edges",
        label: "Contours",
        params: &[
            ParamInfo::range("strength", "Force", 0.0, 4.0, 1.0),
            ParamInfo::range("mix", "Mélange", 0.0, 1.0, 1.0),
        ],
        source: include_str!("shaders/edges.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "rgb_split",
        label: "Décalage RVB",
        params: &[
            ParamInfo::range("distance", "Distance", 0.0, 64.0, 8.0),
            ParamInfo::range("angle", "Angle", -180.0, 180.0, 0.0),
        ],
        source: include_str!("shaders/rgb_split.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "glitch",
        label: "Glitch",
        params: &[
            ParamInfo::range("amount", "Quantité", 0.0, 1.0, 0.5),
            ParamInfo::range("bands", "Bandes", 1.0, 64.0, 16.0),
            ParamInfo::range("rate", "Changements par temps", 0.25, 16.0, 2.0),
        ],
        source: include_str!("shaders/glitch.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "wave",
        label: "Vague",
        params: &[
            ParamInfo::range("amplitude", "Amplitude", 0.0, 0.2, 0.03),
            ParamInfo::range("waves", "Ondulations", 0.5, 20.0, 4.0),
            ParamInfo::range("speed", "Vitesse (par temps)", -4.0, 4.0, 0.5),
            ParamInfo::choice("direction", "Sens", WAVE_CHOICES),
        ],
        source: include_str!("shaders/wave.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "swirl",
        label: "Tourbillon",
        params: &[
            ParamInfo::range("angle", "Angle", -720.0, 720.0, 180.0),
            ParamInfo::range("radius", "Rayon", 0.05, 1.0, 0.5),
        ],
        source: include_str!("shaders/swirl.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "tile",
        label: "Mosaïque",
        params: &[
            ParamInfo::range("columns", "Colonnes", 1.0, 8.0, 2.0),
            ParamInfo::range("rows", "Lignes", 1.0, 8.0, 2.0),
            ParamInfo::choice("mirror", "Reflet", TILE_CHOICES),
        ],
        source: include_str!("shaders/tile.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "vignette",
        label: "Vignette",
        params: &[
            ParamInfo::range("amount", "Quantité", 0.0, 1.0, 0.5),
            ParamInfo::range("size", "Taille", 0.0, 1.0, 0.5),
        ],
        source: include_str!("shaders/vignette.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "strobe",
        label: "Stroboscope",
        params: &[
            ParamInfo::range("rate", "Éclairs par temps", 0.25, 16.0, 2.0),
            ParamInfo::range("duty", "Part d'image", 0.0, 1.0, 0.5),
            ParamInfo::choice("between", "Entre les éclairs", STROBE_CHOICES),
        ],
        source: include_str!("shaders/strobe.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "colorize",
        label: "Colorisation",
        params: &[
            ParamInfo::range("hue", "Teinte", 0.0, 360.0, 200.0),
            ParamInfo::range("saturation", "Saturation", 0.0, 1.0, 0.8),
            ParamInfo::range("amount", "Quantité", 0.0, 1.0, 1.0),
        ],
        source: include_str!("shaders/colorize.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
];

const WAVE_CHOICES: &[&str] = &["Horizontale", "Verticale"];

const TILE_CHOICES: &[&str] = &["Non", "Une tuile sur deux"];

const STROBE_CHOICES: &[&str] = &["Noir", "Blanc", "Transparent"];

/// Every generator, in display order: effects that draw an image of their
/// own instead of changing one.
pub const GENERATORS: &[EffectInfo] = &[
    EffectInfo {
        kind: "solid",
        label: "Couleur unie",
        params: &[
            ParamInfo::range("hue", "Teinte", 0.0, 360.0, 0.0),
            ParamInfo::range("saturation", "Saturation", 0.0, 1.0, 0.0),
            ParamInfo::range("brightness", "Luminosité", 0.0, 1.0, 1.0),
            ParamInfo::range("opacity", "Opacité", 0.0, 1.0, 1.0),
        ],
        source: include_str!("shaders/solid.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "gradient",
        label: "Dégradé",
        params: &[
            ParamInfo::range("hue_from", "Teinte de départ", 0.0, 360.0, 200.0),
            ParamInfo::range("hue_to", "Teinte d'arrivée", 0.0, 360.0, 320.0),
            ParamInfo::range("angle", "Angle", -180.0, 180.0, 0.0),
            ParamInfo::range("saturation", "Saturation", 0.0, 1.0, 0.8),
            ParamInfo::range("speed", "Défilement (par temps)", -2.0, 2.0, 0.0),
        ],
        source: include_str!("shaders/gradient.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "lines",
        label: "Lignes",
        params: &[
            ParamInfo::range("count", "Nombre", 1.0, 64.0, 8.0),
            ParamInfo::range("width", "Épaisseur", 0.0, 1.0, 0.3),
            ParamInfo::range("angle", "Angle", -180.0, 180.0, 0.0),
            ParamInfo::range("speed", "Défilement (par temps)", -4.0, 4.0, 0.25),
        ],
        source: include_str!("shaders/lines.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "rings",
        label: "Cercles",
        params: &[
            ParamInfo::range("count", "Nombre", 1.0, 32.0, 6.0),
            ParamInfo::range("width", "Épaisseur", 0.0, 1.0, 0.3),
            ParamInfo::range("speed", "Expansion (par temps)", -4.0, 4.0, 0.5),
        ],
        source: include_str!("shaders/rings.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "checker",
        label: "Damier",
        params: &[
            ParamInfo::range("count", "Cases", 1.0, 64.0, 8.0),
            ParamInfo::range("rate", "Inversions par temps", 0.0, 8.0, 1.0),
        ],
        source: include_str!("shaders/checker.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: "noise",
        label: "Bruit",
        params: &[
            ParamInfo::range("scale", "Échelle", 1.0, 64.0, 8.0),
            ParamInfo::range("speed", "Changements par temps", 0.0, 8.0, 0.5),
            ParamInfo::choice("color", "Couleur", NOISE_CHOICES),
        ],
        source: include_str!("shaders/noise.wgsl"),
        passes: &["fs_main"],
        common: None,
        text: None,
    },
    EffectInfo {
        kind: TEXT_KIND,
        label: "Texte",
        params: &[
            ParamInfo::range("size", "Taille", 0.02, 1.0, 0.15),
            ParamInfo::range("x", "Position X", -1.0, 1.0, 0.0),
            ParamInfo::range("y", "Position Y", -1.0, 1.0, 0.0),
            ParamInfo::range("hue", "Teinte", 0.0, 360.0, 0.0),
            ParamInfo::range("saturation", "Saturation", 0.0, 1.0, 0.0),
            ParamInfo::range("brightness", "Luminosité", 0.0, 1.0, 1.0),
            ParamInfo::choice("font", "Police", FONT_CHOICES),
            ParamInfo::choice("align", "Alignement", ALIGN_CHOICES),
        ],
        text: Some("Texte"),
        source: include_str!("shaders/text.wgsl"),
        passes: &["fs_main"],
        common: None,
    },
    EffectInfo {
        kind: "fractal_noise",
        label: "Bruit fractal",
        params: bank_params!(
            ParamInfo::choice("base", "Bruit de base", BASE_CHOICES).with_default(2.0),
            ParamInfo::choice("mode", "Mode", FRACTAL_CHOICES),
            ParamInfo::range("frequency", "Fréquence", 0.25, 32.0, 3.0),
            ParamInfo::range("octaves", "Octaves", 1.0, 10.0, 5.0),
            ParamInfo::range("lacunarity", "Lacunarité", 1.2, 4.0, 2.0),
            ParamInfo::range("gain", "Gain", 0.1, 0.95, 0.5),
            ParamInfo::range("evolution", "Évolution", 0.0, 4.0, 0.3),
            ParamInfo::range("drift", "Dérive", 0.0, 4.0, 0.0),
            ParamInfo::range("drift_angle", "Sens de la dérive", -180.0, 180.0, 0.0),
            ParamInfo::range("sharpness", "Finesse des crêtes", 0.5, 4.0, 2.0),
        ),
        source: include_str!("shaders/bank/fractal_noise.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(10),
    },
    EffectInfo {
        kind: "cells",
        label: "Cellules",
        params: bank_params!(
            ParamInfo::choice("mode", "Mode", CELL_CHOICES),
            ParamInfo::choice("metric", "Distance", METRIC_CHOICES),
            ParamInfo::range("density", "Densité", 1.0, 40.0, 6.0),
            ParamInfo::range("jitter", "Aléa", 0.0, 1.0, 1.0),
            ParamInfo::range("motion", "Mouvement", 0.0, 8.0, 1.0),
            ParamInfo::range("edge", "Épaisseur des bords", 0.0, 0.5, 0.08),
        ),
        source: include_str!("shaders/bank/cells.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
    },
    EffectInfo {
        kind: "domain_warp",
        label: "Distorsion",
        params: bank_params!(
            ParamInfo::choice("base", "Bruit de base", BASE_CHOICES).with_default(2.0),
            ParamInfo::range("frequency", "Fréquence", 0.25, 16.0, 2.0),
            ParamInfo::range("octaves", "Octaves", 1.0, 8.0, 4.0),
            ParamInfo::range("warp_levels", "Niveaux de distorsion", 1.0, 3.0, 2.0),
            ParamInfo::range("strength", "Force", 0.0, 6.0, 1.5),
            ParamInfo::range("warp_scale", "Échelle de la distorsion", 0.25, 4.0, 1.0),
            ParamInfo::range("evolution", "Évolution", 0.0, 4.0, 0.2),
        ),
        source: include_str!("shaders/bank/domain_warp.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(7),
    },
    EffectInfo {
        kind: "plane_wave",
        label: "Onde plane",
        params: bank_params!(
            ParamInfo::choice("shape", "Forme", WAVE_SHAPES),
            ParamInfo::range("direction", "Direction", -180.0, 180.0, 0.0),
            ParamInfo::range("wavelength", "Longueur d'onde", 0.01, 2.0, 0.2),
            ParamInfo::range("duty", "Rapport cyclique", 0.05, 0.95, 0.5),
            ParamInfo::range("softness", "Douceur des bords", 0.0, 1.0, 0.1),
        ),
        source: include_str!("shaders/bank/plane_wave.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
    },
    EffectInfo {
        kind: "ripples",
        label: "Ondes concentriques",
        params: bank_params!(
            ParamInfo::choice("shape", "Forme", WAVE_SHAPES),
            ParamInfo::range("center_x", "Centre X", -1.0, 1.0, 0.0),
            ParamInfo::range("center_y", "Centre Y", -1.0, 1.0, 0.0),
            ParamInfo::range("wavelength", "Longueur d'onde", 0.01, 2.0, 0.1),
            ParamInfo::range("damping", "Amortissement", 0.0, 10.0, 1.0),
            ParamInfo::range("duty", "Rapport cyclique", 0.05, 0.95, 0.5),
        ),
        source: include_str!("shaders/bank/ripples.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
    },
    EffectInfo {
        kind: "interference",
        label: "Interférences",
        params: bank_params!(
            ParamInfo::range("sources", "Sources", 2.0, 8.0, 3.0),
            ParamInfo::choice("layout", "Disposition", LAYOUT_CHOICES),
            ParamInfo::range("spread", "Écart", 0.0, 1.0, 0.3),
            ParamInfo::range("motion", "Mouvement", 0.0, 2.0, 0.3),
            ParamInfo::range("wavelength", "Longueur d'onde", 0.01, 1.0, 0.06),
            ParamInfo::range("phase", "Déphasage", 0.0, 1.0, 0.0),
            ParamInfo::choice("render", "Rendu", INTERFERENCE_CHOICES),
        ),
        source: include_str!("shaders/bank/interference.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(7),
    },
    EffectInfo {
        kind: "plasma",
        label: "Plasma",
        params: bank_params!(
            ParamInfo::range("terms", "Termes", 1.0, 8.0, 4.0),
            ParamInfo::range("frequency", "Fréquence", 0.5, 30.0, 6.0),
            ParamInfo::range("complexity", "Complexité", 0.0, 3.0, 1.0),
        ),
        source: include_str!("shaders/bank/plasma.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(3),
    },
    EffectInfo {
        kind: "moire",
        label: "Moiré",
        params: bank_params!(
            ParamInfo::choice("pattern", "Trame", MOIRE_CHOICES),
            ParamInfo::range("pitch", "Pas", 0.004, 0.2, 0.035),
            ParamInfo::range("angle_gap", "Écart d'angle", -45.0, 45.0, 5.0),
            ParamInfo::range("pitch_gap", "Écart de pas", -0.5, 0.5, 0.0),
            ParamInfo::range("shift", "Décalage", -0.5, 0.5, 0.0),
            ParamInfo::range("motion", "Rotation", -20.0, 20.0, 1.0),
            ParamInfo::range("sharpness", "Netteté", 0.0, 1.0, 0.0),
        ),
        source: include_str!("shaders/bank/moire.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(7),
    },
    EffectInfo {
        kind: "spiral",
        label: "Spirale",
        params: bank_params!(
            ParamInfo::range("arms", "Bras", 1.0, 16.0, 3.0),
            ParamInfo::range("twist", "Enroulement", -40.0, 40.0, 8.0),
            ParamInfo::choice("curve", "Type", SPIRAL_CHOICES),
            ParamInfo::choice("shape", "Forme", WAVE_SHAPES),
            ParamInfo::range("duty", "Rapport cyclique", 0.05, 0.95, 0.5),
        ),
        source: include_str!("shaders/bank/spiral.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
    },
    EffectInfo {
        kind: "line_waves",
        label: "Vagues de lignes",
        params: bank_params!(
            ParamInfo::range("lines", "Lignes", 2.0, 100.0, 24.0),
            ParamInfo::range("amplitude", "Amplitude", 0.0, 0.5, 0.06),
            ParamInfo::range("frequency", "Fréquence", 0.25, 20.0, 2.0),
            ParamInfo::range("thickness", "Épaisseur", 0.02, 0.95, 0.2),
            ParamInfo::range("perspective", "Perspective", 0.0, 1.0, 0.0),
            ParamInfo::range("noise", "Bruit", 0.0, 1.0, 0.0),
        ),
        source: include_str!("shaders/bank/line_waves.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
    },
];

const BASE_CHOICES: &[&str] = &["Valeur", "Perlin", "Simplex"];
const FRACTAL_CHOICES: &[&str] = &["fBm", "Turbulence", "Crêtes", "Gonflé"];
const CELL_CHOICES: &[&str] = &[
    "Plus proche (F1)",
    "Bords",
    "Pleines",
    "Deuxième (F2)",
    "Produit",
];
const METRIC_CHOICES: &[&str] = &["Euclidienne", "Manhattan", "Tchebychev"];
const PALETTE_CHOICES: &[&str] = &[
    "Gris",
    "Arc-en-ciel",
    "Feu",
    "Glace",
    "Océan",
    "Néon",
    "Coucher de soleil",
];
const TIME_CHOICES: &[&str] = &["Temps du tempo", "Secondes"];
const WAVE_SHAPES: &[&str] = &["Sinus", "Triangle", "Carré", "Dents de scie"];
const LAYOUT_CHOICES: &[&str] = &["Cercle", "Ligne", "Au hasard"];
const INTERFERENCE_CHOICES: &[&str] = &["Somme", "Intensité"];
const MOIRE_CHOICES: &[&str] = &["Lignes", "Cercles", "Grille"];
const SPIRAL_CHOICES: &[&str] = &["Archimède", "Logarithmique"];
const NO_YES: &[&str] = &["Non", "Oui"];

/// Settings shared by the generators of the procedural bank, after their
/// own: space, time and colour. Their order matches generator.wgsl.
pub const COMMON_PARAMS: [ParamInfo; 16] = [
    ParamInfo::range("zoom", "Zoom", 0.05, 20.0, 1.0),
    ParamInfo::range("offset_x", "Position X", -8.0, 8.0, 0.0),
    ParamInfo::range("offset_y", "Position Y", -8.0, 8.0, 0.0),
    ParamInfo::range("angle", "Rotation", -180.0, 180.0, 0.0),
    ParamInfo::range("stretch", "Étirement", -3.0, 3.0, 0.0),
    ParamInfo::range("speed", "Vitesse", 0.0, 8.0, 1.0),
    ParamInfo::choice("time_unit", "Temps", TIME_CHOICES),
    ParamInfo::range("seed", "Graine", 0.0, 100.0, 0.0),
    ParamInfo::choice("palette", "Palette", PALETTE_CHOICES),
    ParamInfo::range("hue", "Teinte", -180.0, 180.0, 0.0),
    ParamInfo::range("saturation", "Saturation", 0.0, 2.0, 1.0),
    ParamInfo::range("contrast", "Contraste", 0.0, 4.0, 1.0),
    ParamInfo::range("brightness", "Luminosité", 0.0, 2.0, 1.0),
    ParamInfo::choice("invert", "Inverser", NO_YES),
    ParamInfo::range("levels", "Niveaux", 0.0, 16.0, 0.0),
    ParamInfo::range("alpha", "Noir transparent", 0.0, 1.0, 0.0),
];

const NOISE_CHOICES: &[&str] = &["Gris", "Couleur"];
const FONT_CHOICES: &[&str] = &["Ubuntu Light", "Hack"];
const ALIGN_CHOICES: &[&str] = &["Centre", "Gauche", "Droite"];

/// The generator of a kind, if this build knows it.
pub fn find_generator(kind: &str) -> Option<&'static EffectInfo> {
    GENERATORS.iter().find(|info| info.kind == kind)
}

/// Description of `effect`: from the catalog, or from its file for an
/// effect read from a file. `None` for a kind this build does not know or a
/// file that cannot be used.
pub fn info_for(effect: &Effect) -> Option<&'static EffectInfo> {
    if effect.kind == crate::isf::ISF_KIND {
        crate::isf::shader_of(effect).ok().map(|shader| shader.info)
    } else {
        find_any(&effect.kind)
    }
}

/// The effect or generator of a kind.
pub(crate) fn find_any(kind: &str) -> Option<&'static EffectInfo> {
    find(kind).or_else(|| find_generator(kind))
}

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
        for info in GENERATORS {
            assert!(info.params.len() <= MAX_PARAMS, "{}", info.kind);
            if let Some(common) = info.common {
                assert_eq!(&info.params[common..], &COMMON_PARAMS, "{}", info.kind);
            }
            assert_eq!(find_generator(info.kind), Some(info));
            assert_eq!(find(info.kind), None, "{} is also an effect", info.kind);
        }
    }

    #[test]
    fn values_follow_the_effect_within_range() {
        let info = find("color").unwrap();
        let mut effect = info.create();
        effect.set_param("contrast", 9.0);
        effect.set_param("hue", -45.0);
        assert_eq!(&info.values(&effect)[..4], &[0.0, 3.0, 1.0, -45.0]);

        // An animated setting: its value follows the beat, within range.
        let invert = find("invert").unwrap();
        let mut effect = invert.create();
        effect.set_param("amount", 0.3);
        let mut animation = prism_core::Animation::new(-1.0, 2.0);
        animation.shape = prism_core::Shape::Square;
        animation.period = prism_core::Period::Beats(1.0);
        effect.animations.insert("amount".into(), animation);
        let at = |beat| {
            let time = FrameTime {
                beat,
                ..FrameTime::default()
            };
            invert.values_at(&effect, time)[0]
        };
        assert_eq!(at(0.25), 1.0);
        assert_eq!(at(0.75), 0.0);
        assert_eq!(invert.values(&effect)[0], 0.3);

        // An envelope starts at the launch of the clip.
        let mut envelope = prism_core::Animation::new(0.0, 1.0);
        envelope.shape = prism_core::Shape::Ramp;
        envelope.once = true;
        effect.animations.insert("amount".into(), envelope);
        let time = FrameTime {
            beat: 7.0,
            launch_beat: 5.0,
            ..FrameTime::default()
        };
        assert_eq!(invert.values_at(&effect, time)[0], 0.5);

        // A slider of the dashboard runs the whole range; an animation goes
        // first.
        let blur = find("blur").unwrap();
        let mut effect = blur.create();
        effect.links.insert("radius".into(), 3);
        let mut time = FrameTime::default();
        time.dashboard[3] = 0.25;
        assert_eq!(blur.values_at(&effect, time)[0], 16.0);
        effect
            .animations
            .insert("radius".into(), prism_core::Animation::new(5.0, 5.0));
        assert_eq!(blur.values_at(&effect, time)[0], 5.0);

        // A band of the sound goes before a slider, after an animation.
        effect.animations.clear();
        effect.audio.insert("radius".into(), prism_core::Band::Bass);
        time.audio[prism_core::Band::Bass.index()] = 0.5;
        assert_eq!(blur.values_at(&effect, time)[0], 32.0);

        let mirror = find("mirror").unwrap();
        let mut effect = mirror.create();
        effect.set_param("mode", 2.4);
        assert_eq!(mirror.values(&effect)[0], 2.0);
    }
}
