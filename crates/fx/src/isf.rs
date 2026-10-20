//! Shaders in the ISF format (Interactive Shader Format): a GLSL fragment
//! shader after a JSON header that lists its inputs.
//!
//! The header's inputs become settings of the effect; the GLSL code is
//! wrapped with the names ISF defines (`TIME`, `RENDERSIZE`,
//! `isf_FragNormCoord`, `IMG_NORM_PIXEL`…) and translated to WGSL by naga.
//! Supported: one pass, the `inputImage` of filters, inputs of type float,
//! bool, event, long, color and point2D. Not supported: several passes,
//! other images, audio inputs, custom vertex shaders.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use prism_core::Effect;
use serde_json::Value;

use crate::catalog::{EffectInfo, MAX_PARAMS, ParamInfo};

/// Kind of effects read from an ISF file, see [`Effect::file`].
pub const ISF_KIND: &str = "isf";

/// Entry point of the translated fragment shader.
pub(crate) const ENTRY_POINT: &str = "main";

/// Why an ISF file cannot be used.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IsfError {
    /// The file cannot be read.
    #[error("fichier illisible : {0}")]
    Io(String),
    /// The JSON header is missing or invalid.
    #[error("en-tête JSON invalide : {0}")]
    Header(String),
    /// The shader uses something Prism does not handle yet.
    #[error("non pris en charge : {0}")]
    Unsupported(String),
    /// The GLSL code does not compile; lines are those of the file.
    #[error("le shader ne compile pas : {0}")]
    Compile(String),
}

/// An ISF shader made ready for the effect chain.
#[derive(Clone, Debug)]
pub struct IsfShader {
    /// Settings and WGSL source, as for a built-in effect.
    pub info: &'static EffectInfo,
    /// Whether it reads an image (a filter) rather than drawing its own (a
    /// generator).
    pub filter: bool,
    /// Description from the header, if any.
    pub description: Option<String>,
}

/// What the header says.
#[derive(Debug, PartialEq)]
struct Header {
    description: Option<String>,
    inputs: Vec<Input>,
    filter: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Input {
    name: String,
    label: String,
    kind: InputKind,
}

#[derive(Clone, Debug, PartialEq)]
enum InputKind {
    Float {
        min: f32,
        max: f32,
        default: f32,
    },
    Bool {
        default: bool,
    },
    Long {
        values: Vec<i32>,
        labels: Vec<String>,
        default: usize,
    },
    Color {
        default: [f32; 4],
    },
    /// In pixels, unless a range is given: then in that range.
    Point {
        min: [f32; 2],
        max: [f32; 2],
        default: [f32; 2],
        pixels: bool,
    },
}

impl InputKind {
    /// Settings it takes.
    fn width(&self) -> usize {
        match self {
            Self::Color { .. } => 4,
            Self::Point { .. } => 2,
            _ => 1,
        }
    }
}

/// Splits an ISF file into its header and its GLSL code, with the number
/// of lines of the file before the code.
fn split(source: &str) -> Result<(&str, &str, usize), IsfError> {
    let start = source
        .find("/*")
        .filter(|start| source[..*start].trim().is_empty())
        .ok_or_else(|| IsfError::Header("pas de commentaire /* … */ en tête".into()))?;
    let end = source[start..]
        .find("*/")
        .ok_or_else(|| IsfError::Header("commentaire d'en-tête non fermé".into()))?;
    let code = start + end + 2;
    Ok((
        &source[start + 2..start + end],
        &source[code..],
        source[..code].matches('\n').count(),
    ))
}

fn number(value: Option<&Value>) -> Option<f32> {
    value.and_then(Value::as_f64).map(|value| value as f32)
}

fn numbers<const N: usize>(value: Option<&Value>) -> Option<[f32; N]> {
    let array = value?.as_array()?;
    let mut out = [0.0; N];
    for (slot, value) in out.iter_mut().zip(array) {
        *slot = value.as_f64()? as f32;
    }
    (array.len() >= N).then_some(out)
}

fn parse_header(json: &str) -> Result<Header, IsfError> {
    let value: Value =
        serde_json::from_str(json).map_err(|error| IsfError::Header(error.to_string()))?;
    if value
        .get("PASSES")
        .and_then(Value::as_array)
        .is_some_and(|passes| passes.len() > 1)
    {
        return Err(IsfError::Unsupported("plusieurs passes".into()));
    }
    if value
        .get("IMPORTED")
        .is_some_and(|imported| imported.as_object().is_some_and(|map| !map.is_empty()))
    {
        return Err(IsfError::Unsupported("images importées".into()));
    }
    let mut inputs = Vec::new();
    let mut filter = false;
    for input in value
        .get("INPUTS")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = input
            .get("NAME")
            .and_then(Value::as_str)
            .ok_or_else(|| IsfError::Header("entrée sans NAME".into()))?;
        let valid = name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            return Err(IsfError::Header(format!(
                "nom d'entrée invalide « {name} »"
            )));
        }
        let label = input
            .get("LABEL")
            .and_then(Value::as_str)
            .unwrap_or(name)
            .to_owned();
        let kind = match input.get("TYPE").and_then(Value::as_str).unwrap_or("") {
            "image" if name == "inputImage" => {
                filter = true;
                continue;
            }
            "image" => return Err(IsfError::Unsupported(format!("image « {name} »"))),
            "audio" | "audioFFT" => {
                return Err(IsfError::Unsupported(format!("entrée audio « {name} »")));
            }
            "float" => {
                let min = number(input.get("MIN")).unwrap_or(0.0);
                let max = number(input.get("MAX")).unwrap_or(1.0).max(min);
                let default = number(input.get("DEFAULT")).unwrap_or(min).clamp(min, max);
                InputKind::Float { min, max, default }
            }
            "bool" | "event" => InputKind::Bool {
                default: input.get("DEFAULT").is_some_and(|value| {
                    value.as_bool() == Some(true) || value.as_f64().is_some_and(|v| v > 0.0)
                }),
            },
            "long" => {
                let values: Vec<i32> = match input.get("VALUES").and_then(Value::as_array) {
                    Some(values) => values
                        .iter()
                        .filter_map(|value| value.as_i64().map(|value| value as i32))
                        .collect(),
                    None => {
                        let min = number(input.get("MIN")).unwrap_or(0.0) as i32;
                        let max = (number(input.get("MAX")).unwrap_or(1.0) as i32).max(min);
                        (min..=max.min(min + 255)).collect()
                    }
                };
                if values.is_empty() {
                    return Err(IsfError::Header(format!("entrée « {name} » sans valeurs")));
                }
                let labels: Vec<String> = match input.get("LABELS").and_then(Value::as_array) {
                    Some(labels) if labels.len() == values.len() => labels
                        .iter()
                        .map(|label| label.as_str().unwrap_or("").to_owned())
                        .collect(),
                    _ => values.iter().map(ToString::to_string).collect(),
                };
                let wanted = number(input.get("DEFAULT")).unwrap_or(values[0] as f32) as i32;
                let default = values
                    .iter()
                    .position(|value| *value == wanted)
                    .unwrap_or(0);
                InputKind::Long {
                    values,
                    labels,
                    default,
                }
            }
            "color" => InputKind::Color {
                default: numbers::<4>(input.get("DEFAULT")).unwrap_or([1.0; 4]),
            },
            "point2D" => {
                let range = numbers::<2>(input.get("MIN")).zip(numbers::<2>(input.get("MAX")));
                let default = numbers::<2>(input.get("DEFAULT"));
                match range {
                    Some((min, max)) => InputKind::Point {
                        min,
                        max: [max[0].max(min[0]), max[1].max(min[1])],
                        default: default.unwrap_or(min),
                        pixels: false,
                    },
                    // Without a range, a place in the image, from 0 to 1,
                    // turned into pixels for the shader. A default in
                    // pixels cannot be placed without the image size: the
                    // center.
                    None => InputKind::Point {
                        min: [0.0; 2],
                        max: [1.0; 2],
                        default: [0.5; 2],
                        pixels: true,
                    },
                }
            }
            other => return Err(IsfError::Unsupported(format!("entrée de type « {other} »"))),
        };
        inputs.push(Input {
            name: name.to_owned(),
            label,
            kind,
        });
    }
    let width: usize = inputs.iter().map(|input| input.kind.width()).sum();
    if width > MAX_PARAMS {
        return Err(IsfError::Unsupported(format!(
            "{width} réglages, {MAX_PARAMS} au plus"
        )));
    }
    Ok(Header {
        description: value
            .get("DESCRIPTION")
            .and_then(Value::as_str)
            .map(str::to_owned),
        inputs,
        filter,
    })
}

/// Settings of the effect, one per value the shader reads.
fn params(inputs: &[Input]) -> Vec<ParamInfo> {
    // Names live as long as the program: an ISF file is read once per
    // change, see `load`.
    let leak = |text: String| -> &'static str { Box::leak(text.into_boxed_str()) };
    let mut params = Vec::new();
    for input in inputs {
        let id = |suffix: &str| leak(format!("{}{suffix}", input.name));
        let label = |suffix: &str| leak(format!("{}{suffix}", input.label));
        match &input.kind {
            InputKind::Float { min, max, default } => {
                params.push(ParamInfo::range(id(""), label(""), *min, *max, *default));
            }
            InputKind::Bool { default } => {
                let mut param = ParamInfo::choice(id(""), label(""), &["Non", "Oui"]);
                param.default = f32::from(u8::from(*default));
                params.push(param);
            }
            InputKind::Long {
                labels, default, ..
            } => {
                let choices: Vec<&'static str> = labels.iter().cloned().map(leak).collect();
                let mut param =
                    ParamInfo::choice(id(""), label(""), Box::leak(choices.into_boxed_slice()));
                param.default = *default as f32;
                params.push(param);
            }
            InputKind::Color { default } => {
                for (index, (suffix, name)) in
                    [(".r", " R"), (".g", " V"), (".b", " B"), (".a", " A")]
                        .iter()
                        .enumerate()
                {
                    params.push(ParamInfo::range(
                        id(suffix),
                        label(name),
                        0.0,
                        1.0,
                        default[index],
                    ));
                }
            }
            InputKind::Point {
                min, max, default, ..
            } => {
                for (index, (suffix, name)) in [(".x", " X"), (".y", " Y")].iter().enumerate() {
                    params.push(ParamInfo::range(
                        id(suffix),
                        label(name),
                        min[index],
                        max[index],
                        default[index].clamp(min[index], max[index]),
                    ));
                }
            }
        }
    }
    params
}

/// Declarations before the ISF code. Bindings match `prelude.wgsl`.
const WRAPPER_HEAD: &str = r"#version 450
layout(set = 0, binding = 0) uniform PrismParams {
    vec4 prism_values[8];
    vec2 prism_size;
    float prism_time;
    float prism_beat;
    float prism_mask_line;
};
layout(set = 0, binding = 1) uniform texture2D prism_source;
layout(set = 0, binding = 2) uniform sampler prism_sampler;
layout(location = 0) in vec2 prism_uv;
layout(location = 0) out vec4 prism_color;

vec4 prism_frag_coord;
vec2 prism_norm_coord;

float prism_value(int index) {
    return prism_values[index / 4][index % 4];
}

// ISF images have their origin at the bottom, with straight alpha; ours at
// the top, premultiplied.
vec4 prism_sample(vec2 coord) {
    vec4 color = texture(sampler2D(prism_source, prism_sampler), vec2(coord.x, 1.0 - coord.y));
    return color.a > 0.0 ? vec4(color.rgb / color.a, color.a) : vec4(0.0);
}

vec2 prism_image_size() {
    return vec2(textureSize(sampler2D(prism_source, prism_sampler), 0));
}

#define gl_FragColor prism_color
#define gl_FragCoord prism_frag_coord
#define isf_FragNormCoord prism_norm_coord
#define RENDERSIZE prism_size
#define TIME prism_time
#define TIMEDELTA (1.0 / 60.0)
#define FRAMEINDEX int(prism_time * 60.0)
#define PASSINDEX 0
#define DATE vec4(0.0)
#define IMG_NORM_PIXEL(image, coord) prism_sample(coord)
#define IMG_THIS_NORM_PIXEL(image) prism_sample(prism_norm_coord)
#define IMG_THIS_PIXEL(image) prism_sample(prism_norm_coord)
#define IMG_PIXEL(image, coord) prism_sample((coord) / prism_image_size())
#define IMG_SIZE(image) prism_image_size()
#define texture2D(image, coord) prism_sample(coord)
";

/// After the ISF code: the real entry point, around the ISF one.
const WRAPPER_TAIL: &str = r"
#undef main
#undef gl_FragCoord
#undef gl_FragColor
void main() {
    prism_frag_coord = vec4(gl_FragCoord.x, prism_size.y - gl_FragCoord.y, gl_FragCoord.z, gl_FragCoord.w);
    prism_norm_coord = vec2(prism_uv.x, 1.0 - prism_uv.y);
    prism_color = vec4(0.0);
    prism_isf_main();
    prism_color = vec4(clamp(prism_color.rgb, 0.0, 1.0) * prism_color.a, prism_color.a);
}
";

/// The whole GLSL shader, and the line the ISF code starts on.
fn wrap(inputs: &[Input], code: &str) -> (String, usize) {
    let mut glsl = String::from(WRAPPER_HEAD);
    let mut slot = 0;
    for input in inputs {
        let name = &input.name;
        let value = |offset: usize| format!("prism_value({})", slot + offset);
        let _ = match &input.kind {
            InputKind::Float { .. } => writeln!(glsl, "#define {name} {}", value(0)),
            InputKind::Bool { .. } => writeln!(glsl, "#define {name} ({} > 0.5)", value(0)),
            InputKind::Long { values, .. } => {
                let mut function = format!(
                    "int prism_long_{name}(float index) {{\n    int i = int(index + 0.5);\n"
                );
                for (index, number) in values.iter().enumerate() {
                    let _ = writeln!(function, "    if (i == {index}) {{ return {number}; }}");
                }
                let _ = writeln!(function, "    return {};\n}}", values[0]);
                glsl.push_str(&function);
                writeln!(glsl, "#define {name} prism_long_{name}({})", value(0))
            }
            InputKind::Color { .. } => writeln!(
                glsl,
                "#define {name} vec4({}, {}, {}, {})",
                value(0),
                value(1),
                value(2),
                value(3)
            ),
            InputKind::Point { pixels: true, .. } => {
                writeln!(
                    glsl,
                    "#define {name} (vec2({}, {}) * prism_size)",
                    value(0),
                    value(1)
                )
            }
            InputKind::Point { pixels: false, .. } => {
                writeln!(glsl, "#define {name} vec2({}, {})", value(0), value(1))
            }
        };
        slot += input.kind.width();
    }
    glsl.push_str("#define main prism_isf_main\n");
    let first_line = glsl.lines().count() + 1;
    glsl.push_str(code);
    glsl.push_str(WRAPPER_TAIL);
    (glsl, first_line)
}

/// Translates the GLSL shader to WGSL. The ISF code starts on line
/// `first_line` of `glsl`, after `lines_before` lines of the file.
fn translate(glsl: &str, first_line: usize, lines_before: usize) -> Result<String, IsfError> {
    let mut frontend = naga::front::glsl::Frontend::default();
    let options = naga::front::glsl::Options::from(naga::ShaderStage::Fragment);
    let module = frontend.parse(&options, glsl).map_err(|errors| {
        let messages: Vec<String> = errors
            .errors
            .iter()
            .map(|error| {
                let line = error
                    .meta
                    .to_range()
                    .map(|range| glsl[..range.start.min(glsl.len())].lines().count())
                    .filter(|line| *line >= first_line)
                    .map(|line| format!("ligne {} : ", line + 1 - first_line + lines_before))
                    .unwrap_or_default();
                format!("{line}{}", error.kind)
            })
            .collect();
        IsfError::Compile(messages.join(" ; "))
    })?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|error| IsfError::Compile(error.as_inner().to_string()))?;
    naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .map_err(|error| IsfError::Compile(error.to_string()))
}

/// Whether ISF source text is a filter, and its description, from its
/// header alone.
pub(crate) fn summary(source: &str) -> Result<(bool, Option<String>), IsfError> {
    let (json, _, _) = split(source)?;
    let header = parse_header(json)?;
    Ok((header.filter, header.description))
}

/// Reads ISF source text into a shader for the effect chain.
pub fn compile(source: &str, label: &str) -> Result<IsfShader, IsfError> {
    let (json, code, lines_before) = split(source)?;
    let header = parse_header(json)?;
    let (glsl, first_line) = wrap(&header.inputs, code);
    let wgsl = translate(&glsl, first_line, lines_before)?;
    let info = EffectInfo {
        kind: ISF_KIND,
        label: Box::leak(label.to_owned().into_boxed_str()),
        params: Box::leak(params(&header.inputs).into_boxed_slice()),
        text: None,
        source: Box::leak(wgsl.into_boxed_str()),
        passes: &[ENTRY_POINT],
        common: None,
    };
    Ok(IsfShader {
        info: Box::leak(Box::new(info)),
        filter: header.filter,
        description: header.description,
    })
}

type Loaded = (Option<SystemTime>, Result<IsfShader, IsfError>);

fn library() -> &'static Mutex<HashMap<PathBuf, Loaded>> {
    static LIBRARY: OnceLock<Mutex<HashMap<PathBuf, Loaded>>> = OnceLock::new();
    LIBRARY.get_or_init(Mutex::default)
}

/// The shader in the ISF file at `path`, read again when the file changed
/// since the last call: saving the file updates the effect.
pub fn load(path: &Path) -> Result<IsfShader, IsfError> {
    let modified = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok();
    let mut library = library()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((seen, shader)) = library.get(path)
        && *seen == modified
        && modified.is_some()
    {
        return shader.clone();
    }
    let label = path.file_stem().map_or_else(
        || "Shader".into(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let shader = std::fs::read_to_string(path)
        .map_err(|error| IsfError::Io(error.to_string()))
        .and_then(|source| compile(&source, &label));
    library.insert(path.to_path_buf(), (modified, shader.clone()));
    shader
}

/// The ISF shader of an effect of kind [`ISF_KIND`].
pub fn shader_of(effect: &Effect) -> Result<IsfShader, IsfError> {
    match &effect.file {
        Some(path) => load(path),
        None => Err(IsfError::Io("aucun fichier".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INVERT: &str = r#"/*{
        "DESCRIPTION": "Inverts colors",
        "ISFVSN": "2",
        "INPUTS": [
            { "NAME": "inputImage", "TYPE": "image" },
            { "NAME": "amount", "TYPE": "float", "MIN": 0.0, "MAX": 1.0, "DEFAULT": 1.0 },
            { "NAME": "tint", "LABEL": "Teinte", "TYPE": "color", "DEFAULT": [1.0, 0.5, 0.0, 1.0] },
            { "NAME": "mode", "TYPE": "long", "VALUES": [0, 10], "LABELS": ["Plein", "Moitié"], "DEFAULT": 10 },
            { "NAME": "flip", "TYPE": "bool", "DEFAULT": true },
            { "NAME": "center", "TYPE": "point2D" }
        ]
    }*/
    void main() {
        vec4 color = IMG_THIS_PIXEL(inputImage);
        vec3 inverted = mix(color.rgb, 1.0 - color.rgb, amount);
        if (mode == 10 && flip) {
            inverted *= tint.rgb;
        }
        float near = distance(gl_FragCoord.xy, center) / RENDERSIZE.x;
        gl_FragColor = vec4(inverted * (1.0 - 0.0 * near), color.a);
    }
    "#;

    #[test]
    fn header_inputs_become_settings() {
        let shader = compile(INVERT, "invert").unwrap();
        assert!(shader.filter);
        assert_eq!(shader.description.as_deref(), Some("Inverts colors"));
        let ids: Vec<&str> = shader.info.params.iter().map(|param| param.id).collect();
        assert_eq!(
            ids,
            [
                "amount", "tint.r", "tint.g", "tint.b", "tint.a", "mode", "flip", "center.x",
                "center.y"
            ]
        );
        let tint = &shader.info.params[2];
        assert_eq!((tint.label, tint.default), ("Teinte V", 0.5));
        let mode = &shader.info.params[5];
        assert_eq!(mode.choices, ["Plein", "Moitié"]);
        assert_eq!(mode.default, 1.0);
        assert_eq!(shader.info.params[6].default, 1.0);
        assert!(
            shader.info.source.contains("fn main"),
            "{}",
            shader.info.source
        );
    }

    #[test]
    fn generators_have_no_input_image() {
        let source = r#"/*{ "INPUTS": [] }*/
        void main() { gl_FragColor = vec4(isf_FragNormCoord, sin(TIME), 1.0); }"#;
        let shader = compile(source, "gradient").unwrap();
        assert!(!shader.filter);
        assert!(shader.info.params.is_empty());
    }

    #[test]
    fn errors_are_reported() {
        assert!(matches!(
            compile("void main() {}", "x"),
            Err(IsfError::Header(_))
        ));
        assert!(matches!(
            compile("/*{ nope }*/", "x"),
            Err(IsfError::Header(_))
        ));
        let passes = r#"/*{ "PASSES": [{}, {}] }*/ void main() {}"#;
        assert!(matches!(
            compile(passes, "x"),
            Err(IsfError::Unsupported(_))
        ));
        let audio = r#"/*{ "INPUTS": [{ "NAME": "a", "TYPE": "audioFFT" }] }*/ void main() {}"#;
        assert!(matches!(compile(audio, "x"), Err(IsfError::Unsupported(_))));
        let many = format!(
            r#"/*{{ "INPUTS": [{}] }}*/ void main() {{}}"#,
            // Four channels each: more than 32 settings.
            (0..9)
                .map(|i| format!(r#"{{ "NAME": "c{i}", "TYPE": "color" }}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(matches!(compile(&many, "x"), Err(IsfError::Unsupported(_))));
        // A GLSL error: on its line of the file.
        let broken = "/*{\n}*/\nvoid main() {\n    gl_FragColor = vec4(oops);\n}";
        match compile(broken, "x") {
            Err(IsfError::Compile(message)) => assert!(message.contains("ligne 4 "), "{message}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn files_are_read_again_when_changed() {
        let dir = std::env::temp_dir().join(format!("prism-isf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("solid.fs");
        std::fs::write(&path, r#"/*{}*/ void main() { gl_FragColor = vec4(1.0); }"#).unwrap();
        let first = load(&path).unwrap();
        assert_eq!(first.info.label, "solid");
        assert!(std::ptr::eq(first.info, load(&path).unwrap().info));
        // A later modification time: compiled again.
        std::fs::write(
            &path,
            r#"/*{ "INPUTS": [{ "NAME": "level", "TYPE": "float" }] }*/
            void main() { gl_FragColor = vec4(level); }"#,
        )
        .unwrap();
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() + std::time::Duration::from_secs(5))
            .unwrap();
        let second = load(&path).unwrap();
        assert_eq!(second.info.params.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(load(&path), Err(IsfError::Io(_))));
    }
}
