//! DMX over the network: [Art-Net](artnet) and [sACN](sacn) (ANSI E1.31)
//! packets of one universe of 512 channels, and a [`DmxSender`] that sends
//! universes over UDP.
//!
//! Both protocols are public; the packet layouts here follow their
//! specifications.

#![forbid(unsafe_code)]

pub mod artnet;
pub mod sacn;
mod sender;

pub use sender::{DmxSender, Protocol};

/// Channels of a DMX universe.
pub const UNIVERSE_SIZE: usize = 512;

/// The 512 channel values of a universe.
pub type Universe = [u8; UNIVERSE_SIZE];
