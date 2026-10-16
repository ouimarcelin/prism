//! Sound of the clips, played on an output of the computer.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::{AudioError, Mixer};

/// Volume of the output, shared with the interface, from 0 to 1.
#[derive(Debug)]
pub struct Volume(AtomicU32);

impl Default for Volume {
    fn default() -> Self {
        Self(AtomicU32::new(1.0f32.to_bits()))
    }
}

impl Volume {
    /// Current volume.
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    /// Sets the volume, within 0 to 1.
    pub fn set(&self, volume: f32) {
        let volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.0.store(volume.to_bits(), Ordering::Relaxed);
    }
}

/// A running output. The sound stops when it is dropped.
pub struct AudioOutput {
    _stream: cpal::Stream,
    name: String,
}

impl AudioOutput {
    /// Plays what `mixer` mixes on the default output, at `volume`.
    pub fn start(mut mixer: Mixer, volume: Arc<Volume>) -> Result<Self, AudioError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| AudioError::NotFound("aucune sortie".into()))?;
        let name = device.to_string();
        let supported = device
            .default_output_config()
            .map_err(|error| AudioError::Device(error.to_string()))?;
        let config = supported.config();
        let channels = usize::from(config.channels.max(1));
        let rate = config.sample_rate;
        let on_error = |error| eprintln!("prism: audio output: {error}");
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    mixer.render(data, channels, rate, volume.get());
                },
                on_error,
                None,
            ),
            cpal::SampleFormat::I16 => {
                let mut buffer = Vec::new();
                device.build_output_stream(
                    config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        buffer.resize(data.len(), 0.0);
                        mixer.render(&mut buffer, channels, rate, volume.get());
                        for (out, sample) in data.iter_mut().zip(&buffer) {
                            *out = (sample * 32767.0) as i16;
                        }
                    },
                    on_error,
                    None,
                )
            }
            other => return Err(AudioError::Device(format!("format {other} non géré"))),
        }
        .map_err(|error| AudioError::Device(error.to_string()))?;
        stream
            .play()
            .map_err(|error| AudioError::Device(error.to_string()))?;
        Ok(Self {
            _stream: stream,
            name,
        })
    }

    /// Name of the device.
    pub fn name(&self) -> &str {
        &self.name
    }
}
