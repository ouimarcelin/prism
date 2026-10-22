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
    /// For generators that keep an image from one frame to the next: their
    /// first pass (`fs_step`) moves that image on, as many times a frame as
    /// their first setting says, at the size their second setting picks
    /// ([`MEMORY_RESOLUTIONS`]); their second pass shows it.
    pub memory: bool,
}

/// Size of the image a generator with memory keeps, as a divisor of the
/// clip's size, by the value of its second setting.
pub const MEMORY_RESOLUTIONS: [u32; 3] = [1, 2, 4];

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
        memory: false,
        text: None,
    },
    EffectInfo {
        kind: "invert",
        label: "Inversion",
        params: &[ParamInfo::range("amount", "Quantité", 0.0, 1.0, 1.0)],
        source: include_str!("shaders/invert.wgsl"),
        passes: &["fs_main"],
        common: None,
        memory: false,
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
        memory: false,
        text: None,
    },
    EffectInfo {
        kind: "mirror",
        label: "Miroir",
        params: &[ParamInfo::choice("mode", "Sens", MIRROR_CHOICES)],
        source: include_str!("shaders/mirror.wgsl"),
        passes: &["fs_main"],
        common: None,
        memory: false,
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
        memory: false,
        text: None,
    },
    EffectInfo {
        kind: "blur",
        label: "Flou",
        params: &[ParamInfo::range("radius", "Rayon", 0.0, 64.0, 8.0)],
        source: include_str!("shaders/blur.wgsl"),
        passes: &["fs_horizontal", "fs_vertical"],
        common: None,
        memory: false,
        text: None,
    },
    EffectInfo {
        kind: "pixelate",
        label: "Pixellisation",
        params: &[ParamInfo::range("size", "Taille", 1.0, 128.0, 16.0)],
        source: include_str!("shaders/pixelate.wgsl"),
        passes: &["fs_main"],
        common: None,
        memory: false,
        text: None,
    },
    EffectInfo {
        kind: "posterize",
        label: "Postérisation",
        params: &[ParamInfo::range("levels", "Niveaux", 2.0, 32.0, 6.0)],
        source: include_str!("shaders/posterize.wgsl"),
        passes: &["fs_main"],
        common: None,
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
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
        memory: false,
    },
    EffectInfo {
        kind: "curl_noise",
        label: "Bruit tourbillonnant",
        params: bank_params!(
            ParamInfo::range("scale", "Échelle", 0.5, 16.0, 3.0),
            ParamInfo::range("strength", "Force", 0.0, 4.0, 1.0),
            ParamInfo::range("octaves", "Octaves", 1.0, 6.0, 3.0),
            ParamInfo::range("steps", "Pas de transport", 1.0, 8.0, 4.0),
            ParamInfo::range("evolution", "Évolution", 0.0, 2.0, 0.2),
        ),
        source: include_str!("shaders/bank/curl_noise.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "grain",
        label: "Grain",
        params: bank_params!(
            ParamInfo::range("grain_size", "Taille du grain", 0.0005, 0.05, 0.003),
            ParamInfo::range("rate", "Cadence", 1.0, 60.0, 24.0),
            ParamInfo::range("density", "Densité", 0.0, 1.0, 1.0),
        ),
        source: include_str!("shaders/bank/grain.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(3),
        memory: false,
    },
    EffectInfo {
        kind: "colored_noise",
        label: "Bruit rose / brun",
        params: bank_params!(
            ParamInfo::range("slope", "Pente du spectre", 0.0, 2.0, 1.0),
            ParamInfo::range("grain_size", "Taille du grain", 0.0005, 0.05, 0.004),
            ParamInfo::range("rate", "Cadence", 1.0, 60.0, 24.0),
        ),
        source: include_str!("shaders/bank/colored_noise.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(3),
        memory: false,
    },
    EffectInfo {
        kind: "gabor",
        label: "Bruit de Gabor",
        params: bank_params!(
            ParamInfo::range("orientation", "Orientation", -180.0, 180.0, 0.0),
            ParamInfo::range("spread", "Dispersion", 0.0, 1.0, 0.2),
            ParamInfo::range("frequency", "Fréquence", 1.0, 40.0, 8.0),
            ParamInfo::range("bandwidth", "Finesse", 0.5, 4.0, 1.5),
            ParamInfo::range("impulses", "Impulsions", 1.0, 8.0, 4.0),
            ParamInfo::range("scale", "Échelle", 1.0, 20.0, 4.0),
        ),
        source: include_str!("shaders/bank/gabor.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
        memory: false,
    },
    EffectInfo {
        kind: "chladni",
        label: "Ondes stationnaires",
        params: bank_params!(
            ParamInfo::range("mode_n", "Mode n", 1.0, 12.0, 3.0),
            ParamInfo::range("mode_m", "Mode m", 1.0, 12.0, 5.0),
            ParamInfo::choice("plate", "Plaque", PLATE_CHOICES),
            ParamInfo::range("sharpness", "Netteté", 0.0, 1.0, 0.7),
            ParamInfo::range("wobble", "Balancement", 0.0, 2.0, 0.3),
        ),
        source: include_str!("shaders/bank/chladni.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "lissajous",
        label: "Lissajous",
        params: bank_params!(
            ParamInfo::range("ratio_x", "Fréquence X", 1.0, 9.0, 3.0),
            ParamInfo::range("ratio_y", "Fréquence Y", 1.0, 9.0, 2.0),
            ParamInfo::range("phase", "Déphasage", 0.0, 1.0, 0.0),
            ParamInfo::range("thickness", "Épaisseur", 0.001, 0.05, 0.006),
            ParamInfo::range("glow", "Lueur", 0.0, 1.0, 0.3),
            ParamInfo::range("size", "Taille", 0.05, 0.5, 0.4),
        ),
        source: include_str!("shaders/bank/lissajous.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
        memory: false,
    },
    EffectInfo {
        kind: "radar",
        label: "Balayage radar",
        params: bank_params!(
            ParamInfo::range("afterglow", "Rémanence", 0.01, 1.0, 0.25),
            ParamInfo::range("rings", "Anneaux", 0.0, 12.0, 4.0),
            ParamInfo::range("targets", "Cibles", 0.0, 1.0, 0.3),
        ),
        source: include_str!("shaders/bank/radar.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(3),
        memory: false,
    },
    EffectInfo {
        kind: "ocean",
        label: "Océan",
        params: bank_params!(
            ParamInfo::range("height", "Hauteur des vagues", 0.0, 3.0, 1.0),
            ParamInfo::range("wind", "Vent", -180.0, 180.0, 0.0),
            ParamInfo::range("chop", "Crêtes", 0.0, 1.0, 0.5),
            ParamInfo::range("foam", "Écume", 0.0, 1.0, 0.3),
            ParamInfo::range("perspective", "Perspective", 0.0, 1.0, 1.0),
            ParamInfo::range("horizon", "Horizon", -0.5, 0.5, 0.2),
        ),
        source: include_str!("shaders/bank/ocean.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
        memory: false,
    },
    EffectInfo {
        kind: "oscilloscope",
        label: "Oscilloscope",
        params: bank_params!(
            ParamInfo::choice("signal", "Signal", SIGNAL_CHOICES),
            ParamInfo::range("frequency", "Fréquence", 0.5, 20.0, 3.0),
            ParamInfo::range("noise", "Bruit", 0.0, 1.0, 0.1),
            ParamInfo::range("amplitude", "Amplitude", 0.0, 0.5, 0.3),
            ParamInfo::range("glow", "Lueur", 0.0, 1.0, 0.4),
            ParamInfo::range("thickness", "Épaisseur", 0.001, 0.03, 0.004),
            ParamInfo::range("graticule", "Graticule", 0.0, 1.0, 0.3),
        ),
        source: include_str!("shaders/bank/oscilloscope.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(7),
        memory: false,
    },
    EffectInfo {
        kind: "caustics",
        label: "Caustiques",
        params: bank_params!(
            ParamInfo::range("scale", "Échelle", 1.0, 20.0, 5.0),
            ParamInfo::range("depth", "Profondeur", 0.0, 2.0, 0.6),
            ParamInfo::range("line_width", "Largeur des lignes", 0.01, 0.5, 0.12),
            ParamInfo::range("layers", "Couches", 1.0, 3.0, 2.0),
        ),
        source: include_str!("shaders/bank/caustics.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(4),
        memory: false,
    },
    EffectInfo {
        kind: "smoke",
        label: "Fumée",
        params: bank_params!(
            ParamInfo::range("scale", "Échelle", 0.5, 10.0, 2.5),
            ParamInfo::range("rise", "Montée", 0.0, 4.0, 1.0),
            ParamInfo::range("turbulence", "Turbulence", 0.0, 2.0, 0.8),
            ParamInfo::range("density", "Densité", 0.0, 2.0, 1.0),
            ParamInfo::range("spread", "Largeur de la source", 0.0, 1.0, 1.0),
        ),
        source: include_str!("shaders/bank/smoke.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "fire",
        label: "Feu",
        params: bank_params!(
            ParamInfo::range("height", "Hauteur", 0.05, 1.5, 0.8),
            ParamInfo::range("intensity", "Intensité", 0.0, 2.0, 1.0),
            ParamInfo::range("wind", "Vent", -1.0, 1.0, 0.0),
            ParamInfo::range("scale", "Échelle", 1.0, 12.0, 4.0),
            ParamInfo::range("width", "Largeur", 0.1, 2.0, 2.0),
            ParamInfo::range("flicker", "Scintillement", 0.0, 3.0, 1.0),
        ),
        source: include_str!("shaders/bank/fire.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
        memory: false,
    },
    EffectInfo {
        kind: "aurora",
        label: "Aurore",
        params: bank_params!(
            ParamInfo::range("curtains", "Rideaux", 1.0, 5.0, 3.0),
            ParamInfo::range("undulation", "Ondulation", 0.0, 1.0, 0.4),
            ParamInfo::range("ray_height", "Hauteur des rayons", 0.02, 1.0, 0.25),
            ParamInfo::range("shimmer", "Scintillement", 0.0, 1.0, 0.5),
            ParamInfo::range("position", "Position", -0.5, 0.5, -0.1),
        ),
        source: include_str!("shaders/bank/aurora.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "clouds",
        label: "Nuages",
        params: bank_params!(
            ParamInfo::range("coverage", "Couverture", 0.0, 1.0, 0.5),
            ParamInfo::range("softness", "Douceur", 0.02, 0.5, 0.15),
            ParamInfo::range("scale", "Échelle", 0.5, 8.0, 2.0),
            ParamInfo::range("wind", "Vent", 0.0, 4.0, 1.0),
            ParamInfo::range("wind_angle", "Sens du vent", -180.0, 180.0, 0.0),
            ParamInfo::range("sun", "Direction du soleil", -180.0, 180.0, 45.0),
        ),
        source: include_str!("shaders/bank/clouds.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
        memory: false,
    },
    EffectInfo {
        kind: "marble",
        label: "Marbre / bois",
        params: bank_params!(
            ParamInfo::choice("material", "Matière", MATERIAL_CHOICES),
            ParamInfo::range("veins", "Veines", 1.0, 30.0, 6.0),
            ParamInfo::range("turbulence", "Turbulence", 0.0, 4.0, 1.5),
            ParamInfo::range("sharpness", "Netteté", 0.0, 1.0, 0.5),
            ParamInfo::range("scale", "Échelle", 0.5, 8.0, 2.0),
        ),
        source: include_str!("shaders/bank/marble.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "metaballs",
        label: "Métaballes",
        params: bank_params!(
            ParamInfo::range("balls", "Boules", 2.0, 16.0, 6.0),
            ParamInfo::range("size", "Taille", 0.02, 0.3, 0.08),
            ParamInfo::range("threshold", "Seuil", 0.2, 3.0, 1.0),
            ParamInfo::range("glow", "Lueur", 0.0, 1.0, 0.3),
            ParamInfo::range("motion", "Mouvement", 0.0, 3.0, 1.0),
        ),
        source: include_str!("shaders/bank/metaballs.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "tunnel",
        label: "Tunnel",
        params: bank_params!(
            ParamInfo::choice("shape", "Forme", TUNNEL_SHAPES),
            ParamInfo::range("twist", "Torsion", -4.0, 4.0, 0.5),
            ParamInfo::choice("pattern", "Motif", TUNNEL_PATTERNS),
            ParamInfo::range("repeats", "Répétitions", 1.0, 32.0, 8.0),
            ParamInfo::range("fog", "Brouillard", 0.0, 1.0, 0.7),
        ),
        source: include_str!("shaders/bank/tunnel.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "vortex",
        label: "Vortex",
        params: bank_params!(
            ParamInfo::range("twist", "Torsion", -30.0, 30.0, 8.0),
            ParamInfo::range("arms", "Bras", 1.0, 12.0, 4.0),
            ParamInfo::range("tightness", "Resserrement", 0.0, 4.0, 1.0),
            ParamInfo::choice("shape", "Forme", WAVE_SHAPES),
            ParamInfo::range("duty", "Rapport cyclique", 0.05, 0.95, 0.5),
        ),
        source: include_str!("shaders/bank/vortex.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "truchet",
        label: "Truchet",
        params: bank_params!(
            ParamInfo::choice("tile", "Tuile", TRUCHET_CHOICES),
            ParamInfo::range("tiles", "Tuiles", 2.0, 40.0, 8.0),
            ParamInfo::range("thickness", "Épaisseur", 0.02, 0.5, 0.15),
            ParamInfo::range("change", "Changement", 0.0, 4.0, 0.0),
        ),
        source: include_str!("shaders/bank/truchet.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(4),
        memory: false,
    },
    EffectInfo {
        kind: "tiling",
        label: "Pavage",
        params: bank_params!(
            ParamInfo::choice("tiling", "Pavage", TILING_CHOICES),
            ParamInfo::range("tiles", "Tuiles", 2.0, 40.0, 8.0),
            ParamInfo::range("gap", "Joint", 0.0, 0.4, 0.06),
            ParamInfo::range("pulse", "Pulsation", 0.0, 1.0, 0.5),
            ParamInfo::choice("fill", "Remplissage", FILL_CHOICES),
        ),
        source: include_str!("shaders/bank/tiling.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "rays",
        label: "Rayons",
        params: bank_params!(
            ParamInfo::range("rays", "Rayons", 2.0, 64.0, 12.0),
            ParamInfo::range("rotation", "Rotation", -2.0, 2.0, 0.1),
            ParamInfo::choice("shape", "Forme", WAVE_SHAPES),
            ParamInfo::range("duty", "Rapport cyclique", 0.05, 0.95, 0.5),
            ParamInfo::range("softness", "Douceur des bords", 0.0, 1.0, 0.1),
            ParamInfo::range("falloff", "Centre estompé", 0.0, 1.0, 0.5),
        ),
        source: include_str!("shaders/bank/rays.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
        memory: false,
    },
    EffectInfo {
        kind: "warped_grid",
        label: "Grille déformée",
        params: bank_params!(
            ParamInfo::range("pitch", "Pas", 2.0, 60.0, 12.0),
            ParamInfo::range("thickness", "Épaisseur", 0.01, 0.5, 0.06),
            ParamInfo::range("warp", "Déformation", 0.0, 2.0, 0.4),
            ParamInfo::range("warp_scale", "Échelle de la déformation", 0.5, 8.0, 2.0),
            ParamInfo::range("evolution", "Évolution", 0.0, 2.0, 0.3),
        ),
        source: include_str!("shaders/bank/warped_grid.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "polygons",
        label: "Polygones répétés",
        params: bank_params!(
            ParamInfo::range("sides", "Côtés", 3.0, 12.0, 6.0),
            ParamInfo::range("repeats", "Répétitions", 1.0, 40.0, 10.0),
            ParamInfo::range("ring_turn", "Rotation par anneau", -30.0, 30.0, 5.0),
            ParamInfo::range("thickness", "Épaisseur", 0.05, 0.95, 0.3),
        ),
        source: include_str!("shaders/bank/polygons.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(4),
        memory: false,
    },
    EffectInfo {
        kind: "mandelbrot",
        label: "Mandelbrot",
        params: bank_params!(
            ParamInfo::range("center_x", "Centre X", -2.0, 1.0, -0.6),
            ParamInfo::range("center_y", "Centre Y", -1.5, 1.5, 0.0),
            ParamInfo::range("depth", "Profondeur", 0.0, 14.0, 0.0),
            ParamInfo::range("iterations", "Itérations", 16.0, 1000.0, 200.0),
            ParamInfo::range("cycles", "Cycles de couleur", 0.1, 20.0, 3.0),
        ),
        source: include_str!("shaders/bank/mandelbrot.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "julia",
        label: "Julia",
        params: bank_params!(
            ParamInfo::range("radius", "Rayon de c", 0.0, 1.2, 0.7885),
            ParamInfo::range("angle", "Angle de c", -180.0, 180.0, 170.0),
            ParamInfo::range("iterations", "Itérations", 16.0, 500.0, 120.0),
            ParamInfo::range("cycles", "Cycles de couleur", 0.1, 20.0, 3.0),
            ParamInfo::range("drift", "Dérive de c", 0.0, 1.0, 0.1),
        ),
        source: include_str!("shaders/bank/julia.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "fold",
        label: "Pli fractal",
        params: bank_params!(
            ParamInfo::range("folds", "Plis", 1.0, 16.0, 8.0),
            ParamInfo::range("fold_angle", "Angle", -180.0, 180.0, 30.0),
            ParamInfo::range("scale", "Échelle", 1.1, 2.5, 1.6),
            ParamInfo::range("fold_offset", "Décalage", 0.0, 1.0, 0.3),
            ParamInfo::range("thickness", "Épaisseur", 0.01, 1.0, 0.2),
        ),
        source: include_str!("shaders/bank/fold.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(5),
        memory: false,
    },
    EffectInfo {
        kind: "sierpinski",
        label: "Sierpinski",
        params: bank_params!(
            ParamInfo::range("levels", "Niveaux", 1.0, 10.0, 6.0),
            ParamInfo::choice("figure", "Figure", SIERPINSKI_CHOICES),
        ),
        source: include_str!("shaders/bank/sierpinski.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(2),
        memory: false,
    },
    EffectInfo {
        kind: "test_bars",
        label: "Mire",
        params: bank_params!(
            ParamInfo::choice("pattern", "Motif", TEST_CHOICES),
            ParamInfo::range("bars", "Barres", 2.0, 16.0, 8.0),
            ParamInfo::range("drift", "Dérive", 0.0, 2.0, 0.0),
        ),
        source: include_str!("shaders/bank/test_bars.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(3),
        memory: false,
    },
    EffectInfo {
        kind: "analog",
        label: "Signal analogique",
        params: bank_params!(
            ParamInfo::range("roll", "Défilement", 0.0, 2.0, 0.3),
            ParamInfo::range("jitter", "Décrochage", 0.0, 1.0, 0.3),
            ParamInfo::range("dropout", "Pertes", 0.0, 1.0, 0.2),
            ParamInfo::range("lines", "Lignes", 100.0, 1000.0, 480.0),
            ParamInfo::range("snow", "Neige", 0.0, 1.0, 0.3),
            ParamInfo::choice("picture", "Image", ANALOG_CHOICES),
        ),
        source: include_str!("shaders/bank/analog.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
        memory: false,
    },
    EffectInfo {
        kind: "blocks",
        label: "Blocs numériques",
        params: bank_params!(
            ParamInfo::range("blocks", "Blocs", 2.0, 64.0, 12.0),
            ParamInfo::range("density", "Densité", 0.0, 1.0, 0.3),
            ParamInfo::range("rate", "Cadence", 1.0, 30.0, 8.0),
            ParamInfo::range("stretch", "Étirement", 1.0, 8.0, 3.0),
        ),
        source: include_str!("shaders/bank/blocks.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(4),
        memory: false,
    },
    EffectInfo {
        kind: "starfield",
        label: "Champ d'étoiles",
        params: bank_params!(
            ParamInfo::range("density", "Densité", 0.0, 1.0, 0.5),
            ParamInfo::range("layers", "Couches", 1.0, 8.0, 4.0),
            ParamInfo::range("trail", "Traînées", 0.0, 1.0, 0.3),
            ParamInfo::range("size", "Taille", 0.2, 4.0, 1.0),
        ),
        source: include_str!("shaders/bank/starfield.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(4),
        memory: false,
    },
    EffectInfo {
        kind: "rain",
        label: "Pluie / neige",
        params: bank_params!(
            ParamInfo::choice("weather", "Temps", RAIN_CHOICES),
            ParamInfo::range("density", "Densité", 0.0, 1.0, 0.5),
            ParamInfo::range("angle", "Angle", -60.0, 60.0, 10.0),
            ParamInfo::range("layers", "Couches", 1.0, 6.0, 3.0),
            ParamInfo::range("length", "Longueur", 0.0, 1.0, 0.5),
            ParamInfo::range("size", "Taille", 0.3, 3.0, 1.0),
        ),
        source: include_str!("shaders/bank/rain.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
        memory: false,
    },
    EffectInfo {
        kind: "dot_grid",
        label: "Points en grille",
        params: bank_params!(
            ParamInfo::range("pitch", "Pas", 4.0, 80.0, 20.0),
            ParamInfo::range("size", "Taille", 0.0, 1.0, 0.8),
            ParamInfo::range("noise_scale", "Échelle du bruit", 0.5, 8.0, 2.0),
            ParamInfo::range("evolution", "Évolution", 0.0, 2.0, 0.3),
            ParamInfo::choice("dot", "Forme", DOT_CHOICES),
            ParamInfo::choice("follows", "Suit le bruit", FOLLOW_CHOICES),
        ),
        source: include_str!("shaders/bank/dot_grid.wgsl"),
        passes: &["fs_main"],
        text: None,
        common: Some(6),
        memory: false,
    },
    EffectInfo {
        kind: "reaction_diffusion",
        label: "Réaction-diffusion",
        params: bank_params!(
            ParamInfo::range("steps", "Pas par image", 1.0, 32.0, 8.0),
            ParamInfo::choice("resolution", "Résolution", RESOLUTION_CHOICES).with_default(1.0),
            ParamInfo::range("feed", "Apport (f)", 0.01, 0.1, 0.037),
            ParamInfo::range("kill", "Disparition (k)", 0.04, 0.07, 0.06),
            ParamInfo::range("diffusion", "Diffusion", 0.2, 1.0, 1.0),
            ParamInfo::range("seeds", "Graines", 0.0, 1.0, 0.3),
            ParamInfo::range("injection", "Injection", 0.0, 1.0, 0.0),
        ),
        source: include_str!("shaders/bank/reaction_diffusion.wgsl"),
        passes: &["fs_step", "fs_main"],
        text: None,
        common: Some(7),
        memory: true,
    },
    EffectInfo {
        kind: "fluid",
        label: "Fluide 2D",
        params: bank_params!(
            ParamInfo::range("steps", "Pas par image", 1.0, 16.0, 2.0),
            ParamInfo::choice("resolution", "Résolution", RESOLUTION_CHOICES).with_default(1.0),
            ParamInfo::range("viscosity", "Viscosité", 0.0, 1.0, 0.1),
            ParamInfo::range("force", "Remous", 0.0, 2.0, 0.6),
            ParamInfo::range("injection", "Couleur versée", 0.0, 1.0, 0.5),
            ParamInfo::range("dissipation", "Dissipation", 0.0, 1.0, 0.1),
            ParamInfo::range("scale", "Échelle des remous", 0.5, 8.0, 2.0),
        ),
        source: include_str!("shaders/bank/fluid.wgsl"),
        passes: &["fs_step", "fs_main"],
        text: None,
        common: Some(7),
        memory: true,
    },
    EffectInfo {
        kind: "flow_particles",
        label: "Particules dans un flux",
        params: bank_params!(
            ParamInfo::range("steps", "Pas par image", 1.0, 8.0, 1.0),
            ParamInfo::choice("resolution", "Résolution", RESOLUTION_CHOICES).with_default(1.0),
            ParamInfo::range("density", "Particules", 0.0, 1.0, 0.5),
            ParamInfo::range("trail", "Traînée", 0.0, 0.995, 0.97),
            ParamInfo::range("flow_speed", "Vitesse du flux", 0.0, 4.0, 1.5),
            ParamInfo::range("scale", "Échelle du flux", 0.5, 8.0, 2.0),
        ),
        source: include_str!("shaders/bank/flow_particles.wgsl"),
        passes: &["fs_step", "fs_main"],
        text: None,
        common: Some(6),
        memory: true,
    },
    EffectInfo {
        kind: "fireworks",
        label: "Feux d'artifice",
        params: bank_params!(
            ParamInfo::range("steps", "Pas par image", 1.0, 4.0, 1.0),
            ParamInfo::choice("resolution", "Résolution", RESOLUTION_CHOICES).with_default(1.0),
            ParamInfo::range("rate", "Gerbes par temps", 0.25, 4.0, 1.0),
            ParamInfo::range("size", "Taille", 0.1, 1.0, 0.4),
            ParamInfo::range("gravity", "Gravité", 0.0, 2.0, 0.6),
            ParamInfo::range("trail", "Traînée", 0.0, 0.99, 0.9),
            ParamInfo::range("sparks", "Étincelles", 8.0, 64.0, 32.0),
        ),
        source: include_str!("shaders/bank/fireworks.wgsl"),
        passes: &["fs_step", "fs_main"],
        text: None,
        common: Some(7),
        memory: true,
    },
];

/// Families of generators, for choosing one: a name and the kinds in it,
/// in the order of [`GENERATORS`] within each family.
pub const GENERATOR_CATEGORIES: &[(&str, &[&str])] = &[
    (
        "Simples",
        &[
            "solid", "gradient", "lines", "rings", "checker", "noise", "text",
        ],
    ),
    (
        "Bruits",
        &[
            "fractal_noise",
            "cells",
            "domain_warp",
            "curl_noise",
            "grain",
            "colored_noise",
            "gabor",
        ],
    ),
    (
        "Vagues",
        &[
            "plane_wave",
            "ripples",
            "interference",
            "plasma",
            "moire",
            "spiral",
            "line_waves",
            "chladni",
            "lissajous",
            "radar",
            "ocean",
            "oscilloscope",
        ],
    ),
    (
        "Fluides et matières",
        &[
            "caustics",
            "smoke",
            "fire",
            "aurora",
            "clouds",
            "marble",
            "metaballs",
            "reaction_diffusion",
            "fluid",
        ],
    ),
    (
        "Motifs",
        &[
            "tunnel",
            "vortex",
            "truchet",
            "tiling",
            "rays",
            "warped_grid",
            "polygons",
        ],
    ),
    ("Fractales", &["mandelbrot", "julia", "fold", "sierpinski"]),
    ("Signal", &["test_bars", "analog", "blocks"]),
    (
        "Particules",
        &[
            "starfield",
            "rain",
            "dot_grid",
            "flow_particles",
            "fireworks",
        ],
    ),
];

/// Name of the family of the generator `kind`.
pub fn generator_category(kind: &str) -> Option<&'static str> {
    GENERATOR_CATEGORIES
        .iter()
        .find(|(_, kinds)| kinds.contains(&kind))
        .map(|(name, _)| *name)
}

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
const PLATE_CHOICES: &[&str] = &["Carrée", "Ronde"];
const SIGNAL_CHOICES: &[&str] = &["Sinus", "Triangle", "Carré", "Dents de scie", "Bruit"];
const MATERIAL_CHOICES: &[&str] = &["Marbre", "Bois"];
const TUNNEL_SHAPES: &[&str] = &["Rond", "Carré"];
const TUNNEL_PATTERNS: &[&str] = &["Damier", "Anneaux", "Bandes", "Bruit"];
const TRUCHET_CHOICES: &[&str] = &["Arcs", "Diagonales", "Triangles"];
const TILING_CHOICES: &[&str] = &["Hexagones", "Triangles", "Carrés"];
const FILL_CHOICES: &[&str] = &["Pleines", "Contours"];
const SIERPINSKI_CHOICES: &[&str] = &["Triangle", "Tapis"];
const TEST_CHOICES: &[&str] = &["Barres", "Rampe", "Convergence", "Plaque de zones"];
const ANALOG_CHOICES: &[&str] = &["Barres", "Rampe", "Bruit"];
const RAIN_CHOICES: &[&str] = &["Pluie", "Neige"];
const DOT_CHOICES: &[&str] = &["Ronds", "Carrés"];
const FOLLOW_CHOICES: &[&str] = &["Taille", "Luminosité", "Les deux"];
const RESOLUTION_CHOICES: &[&str] = &["Pleine", "Demie", "Quart"];
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
    fn every_generator_is_in_one_family() {
        for info in GENERATORS {
            let families = GENERATOR_CATEGORIES
                .iter()
                .filter(|(_, kinds)| kinds.contains(&info.kind))
                .count();
            assert_eq!(families, 1, "{}", info.kind);
        }
        for (_, kinds) in GENERATOR_CATEGORIES {
            for kind in *kinds {
                assert!(find_generator(kind).is_some(), "{kind}");
            }
        }
    }

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
