//! `aprs-ax25` — AX.25 framing (FCS and addressing), without KISS/RF.
//!
//! Provides the frame check sequence and the 7-octet address-field encoding
//! used by AX.25. KISS framing and radio/modem integration are intentionally
//! out of scope for this crate.

#![no_std]
#![forbid(unsafe_code)]

pub mod address;
pub mod fcs;

pub use address::{decode_address, encode_address, ADDRESS_LEN, C_BIT, H_BIT};
pub use fcs::{fcs, fcs_update, fcs_valid};
