//! Platform media boundary. Transport and native handles stay outside UI state.
pub use kokoda_app_model::{Format, Window};

/// Host preference order, after its backend has probed runtime availability.
pub fn negotiate(host: &[Format], player: &[Format]) -> Option<Format> {
    host.iter().copied().find(|f| player.contains(f))
}

pub struct Frame<'a> {
    pub window: Window,
    pub stream_epoch: u64,
    pub sequence: u64,
    pub format: Format,
    pub keyframe: bool,
    pub payload: &'a [u8],
}

/// Instance bound to one authorized window. The host rechecks before delivery.
pub trait Capture {
    type Error;
    fn next_frame(&mut self) -> Result<Option<Frame<'_>>, Self::Error>;
}

/// Instance bound to a native viewer surface. Clear on revoke or disconnect.
pub trait Player {
    type Error;
    fn present(&mut self, frame: Frame<'_>) -> Result<(), Self::Error>;
    fn clear(&mut self);
}

/// Shared error type for native adapters; no transport dependency.
pub type BackendResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub const MAX_ENCODED_FRAME: usize = 16 * 1024 * 1024;

/// A preset's bitrate describes its reference 16:9 canvas. Retina surfaces above
/// that area get a bounded extra budget; small windows retain the preset budget.
pub fn resolution_bitrate(base: u32, reference_width: u32, size: (u32, u32)) -> u32 {
    let reference = u64::from(reference_width.clamp(1, 8192)).pow(2) * 9 / 16;
    let pixels = u64::from(size.0) * u64::from(size.1);
    let factor_pixels = pixels.clamp(reference.max(1), reference.max(1) * 4);
    let budget = (u64::from(base) * factor_pixels / reference.max(1)).min(20_000_000);
    // Avoid rebuilding the encoder for every few pixels of live resizing.
    ((budget.div_ceil(250_000) * 250_000).min(20_000_000) as u32).max(base.min(20_000_000))
}

#[cfg(test)]
mod tests {
    #[test]
    fn retina_budget_scales_pixels_but_is_bounded_and_quantized() {
        assert_eq!(
            super::resolution_bitrate(6_000_000, 1920, (960, 540)),
            6_000_000
        );
        assert_eq!(
            super::resolution_bitrate(6_000_000, 1920, (2560, 1440)),
            10_750_000
        );
        assert_eq!(
            super::resolution_bitrate(6_000_000, 1920, (3840, 2160)),
            20_000_000
        );
        assert_eq!(
            super::resolution_bitrate(2_000_000, 1280, (3840, 2160)),
            8_000_000
        );
        assert_eq!(
            super::resolution_bitrate(6_000_000, 1920, (2561, 1440)),
            10_750_000
        );
    }
}
