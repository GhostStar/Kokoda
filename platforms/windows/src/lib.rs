//! Windows capture, codec, input and clipboard adapters.
#![cfg(windows)]
pub mod alpha;
pub mod capture;
pub mod clipboard;
pub mod clipboard_image;
pub mod ffmpeg_encoder;
pub mod input;
pub mod live_capture;
pub mod runtime;
pub mod video;
pub mod video_worker;
pub mod windows;

pub mod credentials;

pub mod window_management;

pub mod catalog;
