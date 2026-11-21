//! Transparency is a pixel property, not a main/auxiliary-window distinction.
//! Opaque frames have no sidecar. Cache stable masks (corners/shadows) so motion
//! inside an ordinary window never causes full-frame PNG color compression.
use kokoda_media::BackendResult as Result;
use std::sync::Arc;
use windows_capture::encoder::{ImageEncoder, ImageEncoderPixelFormat, ImageFormat};

#[derive(Clone, Copy)]
pub struct LayerAlpha {
    pub opacity: u8,
    pub color_key: Option<[u8; 3]>, // RGB, when the native layered window uses a color key.
}
impl Default for LayerAlpha {
    fn default() -> Self {
        Self {
            opacity: 255,
            color_key: None,
        }
    }
}

pub struct AlphaMask {
    encoder: ImageEncoder,
    size: (u32, u32),
    previous: Vec<u8>,
    png: Option<Arc<Vec<u8>>>,
}
impl AlphaMask {
    pub fn new() -> Result<Self> {
        Ok(Self {
            encoder: ImageEncoder::new(ImageFormat::Png, ImageEncoderPixelFormat::Bgra8)?,
            size: (0, 0),
            previous: Vec::new(),
            png: None,
        })
    }
    pub fn encode(
        &mut self,
        bgra: &[u8],
        size: (u32, u32),
        layer: LayerAlpha,
    ) -> Result<Option<Arc<Vec<u8>>>> {
        let count = u64::from(size.0) * u64::from(size.1);
        if count == 0 || count > 33_177_600 || bgra.len() as u64 != count * 4 {
            return Err("Invalid alpha frame dimensions".into());
        }
        let alpha_value = |p: &[u8]| -> u8 {
            if layer.color_key == Some([p[2], p[1], p[0]]) {
                0
            } else {
                ((u16::from(p[3]) * u16::from(layer.opacity) + 127) / 255) as u8
            }
        };
        if layer.opacity == 255
            && layer.color_key.is_none()
            && bgra.chunks_exact(4).all(|p| p[3] == 255)
        {
            self.previous.clear();
            self.png = None;
            self.size = size;
            return Ok(None);
        }
        if self.size == size
            && self.previous.len() == count as usize
            && bgra
                .chunks_exact(4)
                .zip(&self.previous)
                .all(|(p, a)| alpha_value(p) == *a)
        {
            return Ok(self.png.clone());
        }
        let alpha: Vec<u8> = bgra.chunks_exact(4).map(alpha_value).collect();
        // An opaque grayscale PNG is a luminance mask; it is independent of the
        // video's chroma subsampling and can be scaled to its decoded extent.
        let mask: Vec<u8> = alpha.iter().flat_map(|a| [*a, *a, *a, 255]).collect();
        let png = self.encoder.encode(&mask, size.0, size.1)?;
        if png.len() > 6 * 1024 * 1024 {
            return Err("Alpha mask exceeds limit".into());
        }
        self.size = size;
        self.previous = alpha;
        self.png = Some(Arc::new(png));
        Ok(self.png.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_motion_has_no_mask_and_alpha_changes_invalidate_cache() {
        let _apartment = crate::runtime::Apartment::new().unwrap();
        let mut mask = AlphaMask::new().unwrap();
        let mut pixels = vec![255; 16 * 16 * 4];
        assert!(mask
            .encode(&pixels, (16, 16), LayerAlpha::default())
            .unwrap()
            .is_none());
        pixels[3] = 0;
        pixels[7] = 128;
        let first = mask
            .encode(&pixels, (16, 16), LayerAlpha::default())
            .unwrap()
            .unwrap();
        assert!(first.starts_with(b"\x89PNG"));
        pixels[8] = 0; // Color animation must reuse the same encoded mask.
        assert!(Arc::ptr_eq(
            &first,
            &mask
                .encode(&pixels, (16, 16), LayerAlpha::default())
                .unwrap()
                .unwrap()
        ));
        pixels[7] = 255;
        assert!(!Arc::ptr_eq(
            &first,
            &mask
                .encode(&pixels, (16, 16), LayerAlpha::default())
                .unwrap()
                .unwrap()
        ));
        pixels[3] = 255;
        assert!(mask
            .encode(&pixels, (16, 16), LayerAlpha::default())
            .unwrap()
            .is_none());
        assert!(mask.encode(&pixels, (8, 8), LayerAlpha::default()).is_err());
        assert!(mask
            .encode(
                &pixels,
                (16, 16),
                LayerAlpha {
                    opacity: 128,
                    color_key: None
                }
            )
            .unwrap()
            .is_some());
        assert!(mask.previous.iter().all(|a| *a == 128));
        mask.encode(
            &pixels,
            (16, 16),
            LayerAlpha {
                opacity: 255,
                color_key: Some([255, 255, 255]),
            },
        )
        .unwrap();
        assert_eq!(mask.previous[0], 0);
    }
}
