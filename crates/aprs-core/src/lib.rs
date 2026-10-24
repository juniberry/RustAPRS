//! `aprs-core` — dependency-free APRS primitives.
//!
//! This crate provides the shared data model, address representation,
//! data-type detection, base-91 utilities, and the TNC2 text parser used by
//! the higher-level decoder and framing crates.
//!
//! It is `no_std` and depends only on `core`, so it is suitable for bare-metal
//! embedded targets.

#![no_std]
#![forbid(unsafe_code)]

pub mod address;
pub mod base91;
pub mod data_type;
pub mod error;
pub mod mice;
pub mod packet;
pub mod tnc2;

pub use address::Address;
pub use data_type::DataType;
pub use error::Error;
pub use mice::MicEMessage;
pub use packet::Packet;
