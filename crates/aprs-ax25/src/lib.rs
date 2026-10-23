//! `aprs-ax25` — AX.25 framing (FCS and addressing), without KISS/RF.
//!
//! Provides the frame check sequence and the 7-octet address-field encoding
//! used by AX.25. KISS framing and radio/modem integration are intentionally
//! out of scope for this crate.

#![no_std]
#![forbid(unsafe_code)]

pub mod address;
pub mod fcs;
pub mod frame;
pub mod kiss;

pub use address::{decode_address, encode_address, ADDRESS_LEN, C_BIT, EXTENSION_BIT, H_BIT};
pub use fcs::{fcs, fcs_update, fcs_valid};
pub use frame::{
    decode_frame, decode_frame_no_fcs, encode_frame, encode_frame_no_fcs, Frame, CONTROL_UI,
    MAX_DIGIPEATERS, PID_NO_L3,
};
pub use kiss::{
    decode as decode_kiss, decode_payload as decode_kiss_payload, encode as encode_kiss, CMD_DATA,
    CMD_RETURN, FEND, FESC, TFEND, TFESC,
};
