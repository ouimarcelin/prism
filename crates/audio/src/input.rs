//! Sound from an input of the computer (microphone, line in, virtual
//! device), measured as it arrives.

use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use prism_core::BAND_COUNT;

use crate::{Analyzer, AudioLevels};

/// Why the input cannot start.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    /// No input device of that name, or none at all.
    #[error("entrée audio introuvable : {0}")]
    NotFound(String),
    /// The device refused, or its format is not handled.
    #[error("entrée audio inutilisable : {0}")]
    Device(String),
}

/// Names of the input devices of the computer, the default one first.
pub fn input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let default = host.default_input_device().map(|device| device.to_string());
    let mut names: Vec<String> = host
        .input_devices()
        .map(|devices| devices.map(|device| device.to_string()).collect())
        .unwrap_or_default();
    if let Some(default) = default {
        names.retain(|name| *name != default);
        names.insert(0, default);
    }
    names
}

/// A running input. Measuring stops when it is dropped; the levels fall to
/// zero.
pub struct AudioInput {
    _stream: cpal::Stream,
    name: String,
    levels: Arc<AudioLevels>,
}

impl AudioInput {
    /// Starts measuring the input named `name`, or the default input, into
    /// `levels`.
    pub fn start(name: Option<&str>, levels: Arc<AudioLevels>) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = match name {
            Some(name) => host
                .input_devices()
                .map_err(|error| AudioError::Device(error.to_string()))?
                .find(|device| device.to_string() == name)
                .ok_or_else(|| AudioError::NotFound(name.to_owned()))?,
            None => host
                .default_input_device()
                .ok_or_else(|| AudioError::NotFound("aucune entrée".into()))?,
        };
        let name = device.to_string();
        let supported = device
            .default_input_config()
            .map_err(|error| AudioError::Device(error.to_string()))?;
        let config = supported.config();
        let channels = usize::from(config.channels.max(1));
        let mut analyzer = Analyzer::new(config.sample_rate);
        let shared = Arc::clone(&levels);
        let mut feed = move |frame_sum: f32| {
            if analyzer.push(frame_sum / channels as f32, shared.gain_db()) {
                shared.set(analyzer.levels());
            }
        };
        let on_error = |error| eprintln!("prism: audio input: {error}");
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    for frame in data.chunks(channels) {
                        feed(frame.iter().sum());
                    }
                },
                on_error,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    for frame in data.chunks(channels) {
                        feed(frame.iter().map(|s| f32::from(*s) / 32768.0).sum());
                    }
                },
                on_error,
                None,
            ),
            other => {
                return Err(AudioError::Device(format!("format {other} non géré")));
            }
        }
        .map_err(|error| AudioError::Device(error.to_string()))?;
        stream
            .play()
            .map_err(|error| AudioError::Device(error.to_string()))?;
        Ok(Self {
            _stream: stream,
            name,
            levels,
        })
    }

    /// Name of the device.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for AudioInput {
    fn drop(&mut self) {
        self.levels.set([0.0; BAND_COUNT]);
    }
}
