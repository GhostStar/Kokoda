//! Prefer compatible hardware H.264 MFTs; retain software fallback for VMs.
use kokoda_media::BackendResult as Result;
use std::{mem::ManuallyDrop, time::Instant};
use windows::{
    core::Interface,
    Win32::{
        Media::MediaFoundation::*,
        System::{Com::*, Variant::VARIANT},
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VideoOptions {
    pub fps: u32,
    pub bitrate: u32,
    pub max_width: u32,
    pub hardware: bool,
    pub resolution_bitrate: bool,
}
impl VideoOptions {
    pub fn for_surface(mut self, source: (u32, u32), viewport: Option<(u32, u32)>) -> Self {
        if self.resolution_bitrate {
            self.bitrate = kokoda_media::resolution_bitrate(
                self.bitrate,
                self.max_width,
                encoded_size(source, self, viewport),
            );
        }
        self
    }
    pub fn parse(request: &serde_json::Value) -> Result<Option<Self>> {
        match request
            .get("codec")
            .and_then(|x| x.as_str())
            .unwrap_or("images")
        {
            "images" => Ok(None),
            "h264" => {
                let number = |key, default, min, max| -> Result<u32> {
                    let n = match request.get(key) {
                        Some(v) => v.as_u64().ok_or("Invalid video setting")?,
                        None => default,
                    };
                    if !(min..=max).contains(&n) {
                        return Err("Video setting out of range".into());
                    }
                    Ok(n as u32)
                };
                Ok(Some(Self {
                    fps: number("fps", 30, 5, 60)?,
                    bitrate: number("bitrate", 4_000_000, 250_000, 20_000_000)?,
                    max_width: number("max_width", 1280, 320, 2560)?,
                    hardware: request.get("encoder").and_then(|v| v.as_str()) != Some("software"),
                    resolution_bitrate: request["resolution_bitrate"].as_bool().unwrap_or(false),
                }))
            }
            _ => Err("Unsupported codec".into()),
        }
    }
}

/// Fit the Mac backing-pixel viewport without upscaling the source. Bound the
/// software encoder workload independently of untrusted client dimensions.
pub fn encoded_size(
    source: (u32, u32),
    options: VideoOptions,
    viewport: Option<(u32, u32)>,
) -> (u32, u32) {
    let (sw, sh) = (source.0 as f64, source.1 as f64);
    let scale = if let Some((w, h)) = viewport {
        (w as f64 / sw).min(h as f64 / sh)
    } else {
        options.max_width as f64 / sw
    };
    let scale = scale.min(1.0).min(3840.0 / sw).min(2160.0 / sh);
    (
        ((sw * scale) as u32 / 2 * 2).max(16),
        ((sh * scale) as u32 / 2 * 2).max(16),
    )
}

enum Backend {
    Ffmpeg(crate::ffmpeg_encoder::Encoder),
    MediaFoundation(MfEncoder),
}
pub struct Encoder {
    backend: Backend,
    pub source: (u32, u32),
    pub size: (u32, u32),
}
impl Encoder {
    pub fn new(
        source: (u32, u32),
        options: VideoOptions,
        viewport: Option<(u32, u32)>,
    ) -> Result<Self> {
        let size = encoded_size(source, options, viewport);
        if options.hardware {
            match crate::ffmpeg_encoder::Encoder::new(size, options.fps, options.bitrate) {
                Ok(encoder) => {
                    eprintln!("H264: FFmpeg {} selected", encoder.backend());
                    return Ok(Self {
                        backend: Backend::Ffmpeg(encoder),
                        source,
                        size,
                    });
                }
                Err(error) => eprintln!(
                    "FFmpeg hardware encoders unavailable: {error}; trying Media Foundation"
                ),
            }
        }
        Ok(Self {
            backend: Backend::MediaFoundation(MfEncoder::new(source, options, viewport)?),
            source,
            size,
        })
    }
    pub fn backend(&self) -> &'static str {
        match &self.backend {
            Backend::Ffmpeg(e) => e.backend(),
            Backend::MediaFoundation(e) => e.backend(),
        }
    }
    pub fn encode(&mut self, bgra: &[u8]) -> Result<Option<Vec<u8>>> {
        match &mut self.backend {
            Backend::Ffmpeg(encoder) => encoder.encode(&nv12(bgra, self.source, self.size)?),
            Backend::MediaFoundation(encoder) => encoder.encode(bgra),
        }
    }
}

struct MfEncoder {
    transform: IMFTransform,
    pub source: (u32, u32),
    pub size: (u32, u32),
    fps: u32,
    start: Instant,
    hardware: bool,
    events: Option<IMFMediaEventGenerator>,
    need_input: bool,
    have_output: bool,
}
impl MfEncoder {
    pub fn backend(&self) -> &'static str {
        if self.hardware {
            "hardware-mf"
        } else {
            "software-mf"
        }
    }
    pub fn new(
        source: (u32, u32),
        options: VideoOptions,
        viewport: Option<(u32, u32)>,
    ) -> Result<Self> {
        unsafe {
            if options.hardware {
                for transform in hardware_encoders() {
                    match Self::configure(transform, source, options, viewport, true) {
                        Ok(encoder) => {
                            eprintln!("H264: hardware MFT selected");
                            return Ok(encoder);
                        }
                        Err(e) => eprintln!("H264 hardware candidate rejected: {e}"),
                    }
                }
            }
            let transform = CoCreateInstance(&CLSID_MSH264EncoderMFT, None, CLSCTX_INPROC_SERVER)?;
            Self::configure(transform, source, options, viewport, false)
        }
    }
    unsafe fn configure(
        transform: IMFTransform,
        source: (u32, u32),
        options: VideoOptions,
        viewport: Option<(u32, u32)>,
        hardware: bool,
    ) -> Result<Self> {
        unsafe {
            let (width, height) = encoded_size(source, options, viewport);
            let stage = |name: &str,
                         error: windows::core::Error|
             -> Box<dyn std::error::Error + Send + Sync> {
                format!(
                    "{name} ({width}x{height}, {} FPS, {} bps): {error}",
                    options.fps, options.bitrate
                )
                .into()
            };
            let attributes = transform.GetAttributes()?;
            let asynchronous = attributes.GetUINT32(&MF_TRANSFORM_ASYNC).unwrap_or(0) != 0;
            if asynchronous {
                attributes
                    .SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1)
                    .map_err(|e| stage("unlock async MFT", e))?;
            }
            let _ = attributes.SetUINT32(&MF_LOW_LATENCY, 1);
            if let Ok(api) = transform.cast::<ICodecAPI>() {
                let _ = api.SetValue(&CODECAPI_AVLowLatencyMode, &VARIANT::from(true));
                // Baseline profile below excludes B pictures. Some drivers reject
                // this optional codec property before media type negotiation.
                let _ = api.SetValue(&CODECAPI_AVEncMPVDefaultBPictureCount, &VARIANT::from(0u32));
            }
            let media = |subtype| -> Result<IMFMediaType> {
                let t = MFCreateMediaType()?;
                t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
                t.SetGUID(&MF_MT_SUBTYPE, &subtype)?;
                t.SetUINT64(
                    &MF_MT_FRAME_SIZE,
                    (u64::from(width) << 32) | u64::from(height),
                )?;
                t.SetUINT64(&MF_MT_FRAME_RATE, (u64::from(options.fps) << 32) | 1)?;
                t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
                t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
                // NV12 conversion below is limited-range BT.601 from sRGB desktop pixels.
                if !hardware {
                    t.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT601.0 as u32)?;
                    t.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
                    t.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32)?;
                    t.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_sRGB.0 as u32)?;
                }
                Ok(t)
            };
            let output = media(MFVideoFormat_H264)?;
            output.SetUINT32(&MF_MT_AVG_BITRATE, options.bitrate)?;
            output.SetUINT32(&MF_MT_MPEG2_PROFILE, 66)?; // baseline: no reordered B frames
            transform
                .SetOutputType(0, &output, 0)
                .map_err(|e| stage("SetOutputType H264 baseline", e))?;
            let input = media(MFVideoFormat_NV12)?;
            transform
                .SetInputType(0, &input, 0)
                .map_err(|e| stage("SetInputType NV12", e))?;
            transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
                .map_err(|e| stage("begin streaming", e))?;
            transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)
                .map_err(|e| stage("start stream", e))?;
            Ok(Self {
                transform: transform.clone(),
                source,
                size: (width, height),
                fps: options.fps,
                start: Instant::now(),
                hardware,
                events: if asynchronous {
                    Some(transform.cast()?)
                } else {
                    None
                },
                need_input: !asynchronous,
                have_output: false,
            })
        }
    }
    pub fn encode(&mut self, bgra: &[u8]) -> Result<Option<Vec<u8>>> {
        unsafe {
            if self.events.is_some() {
                self.wait_event(false)?;
                if self.have_output {
                    self.have_output = false;
                    return self.take_output();
                }
            }
            let pixels = nv12(bgra, self.source, self.size)?;
            let buffer = MFCreateMemoryBuffer(pixels.len() as u32)?;
            let mut pointer = std::ptr::null_mut();
            buffer.Lock(&mut pointer, None, None)?;
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), pointer, pixels.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(pixels.len() as u32)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(
                (self.start.elapsed().as_nanos() / 100).min(i64::MAX as u128) as i64,
            )?;
            sample.SetSampleDuration(10_000_000 / i64::from(self.fps))?;
            self.transform.ProcessInput(0, &sample, 0)?;
            self.need_input = false;
            if self.events.is_some() {
                self.wait_event(true)?;
                if !self.have_output {
                    return Ok(None);
                }
                self.have_output = false;
            }
            self.take_output()
        }
    }
    // Async MFTs deliver readiness through their event queue. Never block the
    // control thread, and bound a broken driver's event wait for fallback.
    fn wait_event(&mut self, after_input: bool) -> Result<()> {
        let started = Instant::now();
        loop {
            let event = unsafe {
                self.events
                    .as_ref()
                    .unwrap()
                    .GetEvent(MF_EVENT_FLAG_NO_WAIT)
            };
            match event {
                Ok(event) => unsafe {
                    event.GetStatus()?.ok()?;
                    match event.GetType()? {
                        kind if kind == METransformNeedInput.0 as u32 => self.need_input = true,
                        kind if kind == METransformHaveOutput.0 as u32 => self.have_output = true,
                        _ => {}
                    }
                },
                Err(e) if e.code() == MF_E_NO_EVENTS_AVAILABLE => {}
                Err(e) => return Err(e.into()),
            }
            if self.have_output
                || (self.need_input && (!after_input || started.elapsed().as_millis() >= 8))
            {
                return Ok(());
            }
            if started.elapsed().as_millis() > 500 {
                return Err("Hardware encoder event timeout".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    fn take_output(&mut self) -> Result<Option<Vec<u8>>> {
        unsafe {
            let mut pointer = std::ptr::null_mut();
            let info = self.transform.GetOutputStreamInfo(0)?;
            let sample = if info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0 {
                None
            } else {
                let sample = MFCreateSample()?;
                sample.AddBuffer(&MFCreateMemoryBuffer(
                    info.cbSize.max(self.size.0 * self.size.1 * 2),
                )?)?;
                Some(sample)
            };
            let mut output = [MFT_OUTPUT_DATA_BUFFER {
                dwStreamID: 0,
                pSample: ManuallyDrop::new(sample),
                ..Default::default()
            }];
            let mut status = 0;
            let result = self.transform.ProcessOutput(0, &mut output, &mut status);
            let sample = ManuallyDrop::take(&mut output[0].pSample);
            drop(ManuallyDrop::take(&mut output[0].pEvents));
            match result {
                Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(None),
                Err(e) => Err(e.into()),
                Ok(()) => {
                    let buffer = sample
                        .ok_or("H264 output missing sample")?
                        .ConvertToContiguousBuffer()?;
                    let mut len = 0;
                    buffer.Lock(&mut pointer, None, Some(&mut len))?;
                    let bytes = std::slice::from_raw_parts(pointer, len as usize).to_vec();
                    buffer.Unlock()?;
                    Ok(Some(bytes))
                }
            }
        }
    }
}
fn hardware_encoders() -> Vec<IMFTransform> {
    unsafe {
        let input = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_NV12,
        };
        let output = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_H264,
        };
        let mut array = std::ptr::null_mut();
        let mut count = 0;
        if MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            Some(&input),
            Some(&output),
            &mut array,
            &mut count,
        )
        .is_err()
        {
            return vec![];
        }
        let mut encoders = Vec::new();
        if !array.is_null() {
            for item in std::slice::from_raw_parts_mut(array, count as usize) {
                if let Some(activate) = item.take() {
                    if let Ok(transform) = activate.ActivateObject::<IMFTransform>() {
                        encoders.push(transform);
                    }
                }
            }
            CoTaskMemFree(Some(array.cast()));
        }
        encoders
    }
}

impl Drop for MfEncoder {
    fn drop(&mut self) {
        unsafe {
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
            let _ = self
                .transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
        }
    }
}

// Windows' native Fant filter integrates source pixel areas, preserving thin
// strokes without nearest-neighbor aliasing. Use the OS optimized implementation
// rather than spending the frame budget in Rust loops under x64 emulation.
fn area_resize(source: &[u8], sw: usize, sh: usize, w: usize, h: usize) -> Result<Vec<u8>> {
    unsafe {
        use windows::Win32::Graphics::Imaging::*;
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let bitmap = factory.CreateBitmapFromMemory(
            sw as u32,
            sh as u32,
            &GUID_WICPixelFormat32bppBGR,
            sw as u32 * 4,
            source,
        )?;
        let scaler = factory.CreateBitmapScaler()?;
        scaler.Initialize(&bitmap, w as u32, h as u32, WICBitmapInterpolationModeFant)?;
        let mut output = vec![0; w * h * 4];
        scaler.CopyPixels(std::ptr::null(), w as u32 * 4, &mut output)?;
        Ok(output)
    }
}

// CPU conversion is a bounded experimental path; GPU scaling/conversion comes later.
fn nv12(bgra: &[u8], source: (u32, u32), size: (u32, u32)) -> Result<Vec<u8>> {
    let (sw, sh) = (source.0 as usize, source.1 as usize);
    let (w, h) = (size.0 as usize, size.1 as usize);
    if sw == 0 || sh == 0 || bgra.len() != sw * sh * 4 {
        return Err("Invalid BGRA frame".into());
    }
    let resized;
    let pixels = if (sw, sh) == (w, h) {
        bgra
    } else {
        resized = area_resize(bgra, sw, sh, w, h)?;
        &resized
    };
    let mut result = vec![0; w * h * 3 / 2];
    let rgb = |x: usize, y: usize| {
        let i = (y * w + x) * 4;
        (pixels[i + 2] as i32, pixels[i + 1] as i32, pixels[i] as i32)
    };
    let clamp = |v: i32| v.clamp(0, 255) as u8;
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = rgb(x, y);
            result[y * w + x] = clamp(((66 * r + 129 * g + 25 * b + 128) >> 8) + 16);
            if x % 2 == 0 && y % 2 == 0 {
                let mut sum = (0, 0, 0);
                for dy in 0..2 {
                    for dx in 0..2 {
                        let c = rgb(x + dx, y + dy);
                        sum.0 += c.0;
                        sum.1 += c.1;
                        sum.2 += c.2;
                    }
                }
                let (r, g, b) = (sum.0 / 4, sum.1 / 4, sum.2 / 4);
                result[w * h + (y / 2) * w + x] =
                    clamp(((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128);
                result[w * h + (y / 2) * w + x + 1] =
                    clamp(((112 * r - 94 * g - 18 * b + 128) >> 8) + 128);
            }
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn area_filter_preserves_subpixel_lines() {
        let _runtime = crate::runtime::Apartment::new().unwrap();
        let mut checker = vec![0u8; 8 * 8 * 4];
        for y in 0..8 {
            for x in 0..8 {
                for c in 0..3 {
                    checker[(y * 8 + x) * 4 + c] = if (x + y) % 2 == 0 { 255 } else { 0 };
                }
            }
        }
        let small = area_resize(&checker, 8, 8, 4, 4).unwrap();
        assert!(small.chunks_exact(4).all(|p| p[..3] == [128, 128, 128]));
        let uneven = area_resize(&checker, 8, 8, 3, 3).unwrap();
        assert!(uneven.chunks_exact(4).all(|p| (119..=136).contains(&p[0])));
    }
    #[test]
    fn black_and_white_nv12() {
        assert_eq!(
            nv12(&[0; 16], (2, 2), (2, 2)).unwrap(),
            vec![16, 16, 16, 16, 128, 128]
        );
        assert_eq!(
            nv12(&[255; 16], (2, 2), (2, 2)).unwrap(),
            vec![235, 235, 235, 235, 128, 128]
        );
        assert!(nv12(&[], (2, 2), (2, 2)).is_err());
    }
    #[test]
    fn viewport_fits_backing_pixels_without_upscaling() {
        let options = VideoOptions {
            fps: 30,
            bitrate: 6_000_000,
            max_width: 1280,
            hardware: false,
            resolution_bitrate: false,
        };
        assert_eq!(
            encoded_size((2400, 1600), options, Some((2000, 1400))),
            (2000, 1332)
        );
        assert_eq!(
            encoded_size((2400, 1600), options, Some((1000, 700))),
            (1000, 666)
        );
        assert_eq!(
            encoded_size((1600, 900), options, Some((3000, 2000))),
            (1600, 900)
        );
        assert_eq!(
            encoded_size((7680, 4320), options, Some((8192, 8192))),
            (3840, 2160)
        );
        assert_eq!(encoded_size((2400, 1600), options, None), (1280, 852));
    }
    #[test]
    fn options_are_bounded() {
        assert!(VideoOptions::parse(&serde_json::json!({"codec":"h264","fps":0})).is_err());
        assert!(
            VideoOptions::parse(&serde_json::json!({"codec":"h264","fps":30}))
                .unwrap()
                .is_some()
        );
        assert!(VideoOptions::parse(&serde_json::json!({}))
            .unwrap()
            .is_none());
    }
}
