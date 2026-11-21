//! Platform-independent identities and user settings shared by both endpoints.
//! These domain types are not a serialized wire format.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Window {
    pub id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    Original,
    Smooth,
    #[default]
    Clear,
    HighFrameRate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    H264,
    Png,
}

mod application;
pub use application::{application_scope_windows, application_windows, AppWindow, WindowBounds};
pub mod input;

pub mod window_management;
