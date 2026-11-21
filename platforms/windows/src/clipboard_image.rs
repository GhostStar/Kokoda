//! Bounded native PNG / DIB conversion. No external codec process is required.
use kokoda_media::BackendResult as Result;
use std::ptr;
use windows::Win32::{
    Graphics::Imaging::*,
    System::Com::{StructuredStorage::*, *},
};
pub const MAX_PNG: usize = 8 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
pub fn dimensions(w: u32, h: u32) -> Result<()> {
    if w == 0 || h == 0 || w > 16384 || h > 16384 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err("Clipboard image exceeds 16 megapixels or has invalid dimensions".into());
    }
    Ok(())
}
fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = factory.CreateStream()?;
        stream.InitializeFromMemory(bytes)?;
        let decoder = factory.CreateDecoderFromStream(
            &stream,
            ptr::null(),
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = decoder.GetFrame(0)?;
        let (mut w, mut h) = (0, 0);
        frame.GetSize(&mut w, &mut h)?;
        dimensions(w, h)?;
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        let mut pixels = vec![0; w as usize * h as usize * 4];
        converter.CopyPixels(ptr::null(), w * 4, &mut pixels)?;
        Ok((w, h, pixels))
    }
}
pub fn validate_png(png: &[u8]) -> Result<()> {
    decode_png(png).map(|_| ())
}
fn decode_png(png: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    if png.len() > MAX_PNG || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Invalid PNG or image exceeds 8 MiB".into());
    }
    decode(png)
}
pub fn png_to_dibs(png: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let (w, h, pixels) = decode_png(png)?;
    // Top-down BITMAPV5HEADER, unassociated BGRA alpha and explicit sRGB masks.
    let mut v5 = vec![0u8; 124];
    for (offset, value) in [
        (0, 124),
        (4, w),
        (8, (-(h as i32)) as u32),
        (16, 3),
        (20, pixels.len() as u32),
        (40, 0x00ff0000),
        (44, 0x0000ff00),
        (48, 0x000000ff),
        (52, 0xff000000),
        (56, 0x73524742),
        (108, 4),
    ] {
        v5[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    v5[12..14].copy_from_slice(&1u16.to_le_bytes());
    v5[14..16].copy_from_slice(&32u16.to_le_bytes());
    v5.extend_from_slice(&pixels);
    // Legacy consumers often ignore alpha. Supply an opaque white-composited 24bpp DIB too.
    let stride = (w as usize * 3 + 3) & !3;
    let mut dib = vec![0u8; 40 + stride * h as usize];
    for (offset, value) in [
        (0, 40),
        (4, w),
        (8, (-(h as i32)) as u32),
        (20, (stride * h as usize) as u32),
    ] {
        dib[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    dib[12..14].copy_from_slice(&1u16.to_le_bytes());
    dib[14..16].copy_from_slice(&24u16.to_le_bytes());
    for y in 0..h as usize {
        for x in 0..w as usize {
            let src = (y * w as usize + x) * 4;
            let dst = 40 + y * stride + x * 3;
            let a = pixels[src + 3] as u32;
            for c in 0..3 {
                dib[dst + c] = ((pixels[src + c] as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
            }
        }
    }
    Ok((v5, dib))
}
pub fn dib_to_png(dib: &[u8]) -> Result<Vec<u8>> {
    // Wrap a packed DIB in a BMP file header for the OS codec. Reject offsets and
    // palettes outside the allocation before handing it to WIC.
    if dib.len() < 40 || dib.len() > MAX_PIXELS as usize * 4 + 4096 {
        return Err("Invalid or oversized DIB".into());
    }
    let u32at = |n| u32::from_le_bytes(dib[n..n + 4].try_into().unwrap());
    let header = u32at(0) as usize;
    if ![40, 108, 124].contains(&header) || header > dib.len() {
        return Err("Unsupported DIB header".into());
    }
    let w = i32::from_le_bytes(dib[4..8].try_into()?);
    let h = i32::from_le_bytes(dib[8..12].try_into()?);
    if w <= 0 || h == i32::MIN {
        return Err("Invalid DIB dimensions".into());
    }
    dimensions(w as u32, h.unsigned_abs())?;
    let bpp = u16::from_le_bytes(dib[14..16].try_into()?);
    let compression = u32at(16);
    if ![1, 4, 8, 16, 24, 32].contains(&bpp) || ![0, 3].contains(&compression) {
        return Err("Unsupported DIB layout".into());
    }
    let colors = if u32at(32) > 0 {
        u32at(32) as usize
    } else if bpp <= 8 {
        1usize << bpp
    } else {
        0
    };
    if colors > 256 {
        return Err("Invalid DIB palette".into());
    }
    let mut offset = header
        + colors * 4
        + if header == 40 && compression == 3 {
            12
        } else {
            0
        };
    let stride = (w as usize * bpp as usize).div_ceil(32) * 4;
    // Windows clipboard synthesis can append a duplicate RGB mask table after
    // a V4/V5 header. It is not pixel data. Recognize only the exact extra-table
    // layout; ordinary V5 images keep masks inside the header.
    let pixel_bytes = stride * h.unsigned_abs() as usize;
    if header >= 108
        && compression == 3
        && colors == 0
        && dib.len() == offset + 12 + pixel_bytes
        && dib[offset..offset + 12] == dib[40..52]
    {
        offset += 12;
    }

    if offset
        .checked_add(stride * h.unsigned_abs() as usize)
        .is_none_or(|n| n > dib.len())
    {
        return Err("Truncated DIB pixels".into());
    }
    let mut bmp = vec![0u8; 14];
    bmp[..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&((dib.len() + 14) as u32).to_le_bytes());
    bmp[10..14].copy_from_slice(&((offset + 14) as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    let (w, h, pixels) = decode(&bmp)?;
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = CreateStreamOnHGlobal(Default::default(), true)?;
        let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let (mut frame, mut properties) = (None, None);
        encoder.CreateNewFrame(&mut frame, &mut properties)?;
        let frame = frame.ok_or("PNG encoder returned no frame")?;
        frame.Initialize(properties.as_ref())?;
        frame.SetSize(w, h)?;
        let mut format = GUID_WICPixelFormat32bppBGRA;
        frame.SetPixelFormat(&mut format)?;
        if format != GUID_WICPixelFormat32bppBGRA {
            return Err("PNG encoder does not support alpha".into());
        }
        frame.WritePixels(h, w * 4, &pixels)?;
        frame.Commit()?;
        encoder.Commit()?;
        let mut stat = STATSTG::default();
        stream.Stat(&mut stat, STATFLAG_NONAME)?;
        let size = usize::try_from(stat.cbSize)?;
        if size > MAX_PNG {
            return Err("Clipboard PNG exceeds 8 MiB".into());
        }
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        let mut png = vec![0u8; size];
        let mut read = 0;
        stream
            .Read(png.as_mut_ptr() as _, size as u32, Some(&mut read))
            .ok()?;
        if read as usize != size {
            return Err("Incomplete PNG encoding".into());
        }
        Ok(png)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_dib_roundtrip_preserves_alpha_and_legacy_background() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let _apartment = crate::runtime::Apartment::new().unwrap();
        let png=STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP4z8DwHwgbGMA0EAAAP9cIeY8TiooAAAAASUVORK5CYII=").unwrap();
        let (v5, legacy) = png_to_dibs(&png).unwrap();
        let rgba = decode(&png).unwrap().2;
        assert_eq!(decode(&dib_to_png(&v5).unwrap()).unwrap().2, rgba);
        let opaque = decode(&dib_to_png(&legacy).unwrap()).unwrap().2;
        assert_eq!(&opaque[8..12], &[255, 255, 255, 255]);
        assert_eq!(opaque[7], 255);
        assert!(opaque[4] >= 126 && opaque[4] <= 128);
        assert_eq!(opaque[5], 255);
    }
    #[test]
    fn clipboard_v5_duplicate_masks_are_not_pixels() {
        let _apartment = crate::runtime::Apartment::new().unwrap();
        let mut dib = vec![0u8; 124];
        for (offset, value) in [
            (0, 124u32),
            (4, 2),
            (8, 1),
            (16, 3),
            (20, 8),
            (40, 0xff0000),
            (44, 0xff00),
            (48, 0xff),
        ] {
            dib[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        dib[12..14].copy_from_slice(&1u16.to_le_bytes());
        dib[14..16].copy_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&dib[40..52].to_vec());
        dib.extend_from_slice(&[0, 0, 255, 0, 0, 255, 0, 0]);
        let (_, _, pixels) = decode(&dib_to_png(&dib).unwrap()).unwrap();
        assert_eq!(pixels, vec![0, 0, 255, 255, 0, 255, 0, 255]);
    }
    #[test]
    fn malformed_dib_is_rejected_before_allocation() {
        assert!(dib_to_png(&[]).is_err());
        let mut data = vec![0; 40];
        data[..4].copy_from_slice(&40u32.to_le_bytes());
        data[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(dib_to_png(&data).is_err());
        assert!(dimensions(16384, 16384).is_err());
        assert!(dimensions(3840, 2160).is_ok());
    }
}
