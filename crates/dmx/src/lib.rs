//! DMX over the network: [Art-Net](artnet) and [sACN](sacn) (ANSI E1.31)
//! packets of one universe of 512 channels, and a [`DmxSender`] that sends
//! universes over UDP; [`universes`] lays the colours of the fixtures of
//! a [`prism_core::Lighting`] out in universes.
//!
//! Both protocols are public; the packet layouts here follow their
//! specifications.

#![forbid(unsafe_code)]

pub mod artnet;
mod mapping;
pub mod sacn;
mod sender;

pub use mapping::{pixel_count, universes};
pub use sender::{DmxSender, Protocol};

/// Channels of a DMX universe.
pub const UNIVERSE_SIZE: usize = 512;

/// The 512 channel values of a universe.
pub type Universe = [u8; UNIVERSE_SIZE];
