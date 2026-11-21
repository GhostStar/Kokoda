//! Shared host lifecycle. The host event loop owns this state serially.
//! Authentication and window authorization must be enforced before dispatch.
mod admission;
pub use admission::{Admission, AdmissionError, Connection};

#[cfg(all(windows, feature = "windows-runtime"))]
pub mod windows;

pub mod sharing;

pub mod flow;
