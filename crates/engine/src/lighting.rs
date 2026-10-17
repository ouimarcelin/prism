//! Pixel mapping in the render thread: the colours of the fixtures taken
//! from the program image, sent as DMX at the rate asked.

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use prism_core::{DmxProtocol, Lighting, Point};
use prism_dmx::{DmxSender, Protocol};
use prism_render::{Gpu, PixelSampler, wgpu};

/// Sends the colours of the fixtures of the composition.
pub(crate) struct LightingOutput {
    sampler: PixelSampler,
    /// Sender, with the protocol and target it was made for.
    sender: Option<(DmxProtocol, String, DmxSender)>,
    last_sent: Option<Instant>,
    /// Why DMX is not sent, if it is not.
    pub(crate) error: Option<String>,
    /// Universes sent, counted.
    pub(crate) sent: u64,
}

/// Where `target` (« IP » or « IP:port ») sends; `None` for everyone.
fn parse_target(target: &str) -> Result<Option<SocketAddr>, String> {
    let target = target.trim();
    if target.is_empty() {
        return Ok(None);
    }
    if let Ok(address) = target.parse::<SocketAddr>() {
        return Ok(Some(address));
    }
    target
        .parse::<IpAddr>()
        .map(|address| Some(SocketAddr::new(address, 0)))
        .map_err(|_| format!("adresse DMX invalide : « {target} »"))
}

impl LightingOutput {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self {
            sampler: PixelSampler::new(device),
            sender: None,
            last_sent: None,
            error: None,
            sent: 0,
        }
    }

    /// Records the sampling of the fixtures' colours from `program`.
    pub(crate) fn sample(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        program: &wgpu::TextureView,
        lighting: &Lighting,
    ) {
        if !lighting.enabled {
            self.sender = None;
            self.error = None;
            return;
        }
        let points: Vec<Point> = lighting
            .fixtures()
            .iter()
            .filter(|fixture| fixture.enabled)
            .flat_map(|fixture| (0..fixture.pixels).map(|index| fixture.pixel_point(index)))
            .collect();
        self.sampler
            .sample(&gpu.device, &gpu.queue, encoder, program, &points);
    }

    /// After the frame is submitted: sends the colours read back, at most
    /// `lighting.rate` times per second.
    pub(crate) fn send(&mut self, gpu: &Gpu, lighting: &Lighting) {
        if !lighting.enabled {
            return;
        }
        self.sampler.map_submitted();
        let Some(colors) = self.sampler.read(&gpu.device) else {
            return;
        };
        let period = Duration::from_secs_f32(1.0 / lighting.rate.clamp(1.0, 60.0));
        if self.last_sent.is_some_and(|last| last.elapsed() < period) {
            return;
        }
        if self.sender.as_ref().is_none_or(|(protocol, target, _)| {
            *protocol != lighting.protocol || *target != lighting.target
        }) {
            let protocol = match lighting.protocol {
                DmxProtocol::ArtNet => Protocol::ArtNet,
                DmxProtocol::Sacn => Protocol::Sacn,
            };
            let sender = parse_target(&lighting.target).and_then(|target| {
                match target {
                    // An address without a port: the protocol's port.
                    Some(address) if address.port() == 0 => {
                        DmxSender::new(protocol, Some(address.ip()))
                    }
                    target => DmxSender::to(protocol, target),
                }
                .map_err(|error| error.to_string())
            });
            match sender {
                Ok(sender) => {
                    self.sender = Some((lighting.protocol, lighting.target.clone(), sender));
                    self.error = None;
                }
                Err(error) => {
                    self.sender = None;
                    self.error = Some(error);
                    return;
                }
            }
        }
        let Some((_, _, sender)) = &mut self.sender else {
            return;
        };
        self.last_sent = Some(Instant::now());
        for (universe, channels) in prism_dmx::universes(lighting.fixtures(), &colors) {
            match sender.send(universe, &channels) {
                Ok(()) => self.sent += 1,
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_are_an_address_with_or_without_port() {
        assert_eq!(parse_target(""), Ok(None));
        assert_eq!(
            parse_target("192.168.1.20"),
            Ok(Some("192.168.1.20:0".parse().unwrap()))
        );
        assert_eq!(
            parse_target(" 10.0.0.5:6455 "),
            Ok(Some("10.0.0.5:6455".parse().unwrap()))
        );
        assert!(parse_target("console").is_err());
    }
}
