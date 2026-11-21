//! Encrypted Kokoda connections with bounded length-prefixed framing.
mod bytes_codec;
pub use bytes_codec::{BytesCodec, MAX_FRAME_LENGTH};

pub mod connection;
pub mod tls;

pub mod password;
