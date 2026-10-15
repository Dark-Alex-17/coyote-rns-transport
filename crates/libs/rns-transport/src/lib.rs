//! Transport boundary crate for runtime crates and daemon entrypoints.

#![allow(clippy::unwrap_used)]

extern crate alloc;

pub mod buffer;
pub mod channel;
pub mod channel_buffer;
pub mod config;
pub mod crypt;
pub mod delivery;
pub mod destination;
pub mod destination_hash;
pub mod discovery;
pub mod embedded_link;
pub mod error;
pub mod hash;
pub mod identity;
pub mod iface;
pub mod kiss;
pub mod packet;
pub mod ratchets;
pub mod receipt;
pub mod resource;
pub mod serde;
#[cfg(feature = "storage")]
pub mod storage;
pub mod time;
pub mod transport;
pub mod utils;

pub mod identity_bridge;
#[cfg(test)]
mod no_stdout_prints_tests;

pub use packet::{DestinationType, Packet, PacketContext, PacketDataBuffer, PacketType};
/// Re-exported so callers can name the `rand_core` RNG types the public constructors take.
pub use rand_core;
/// Re-exported so callers can build the exact `rmpv::Value` the public API takes.
pub use rmpv;
