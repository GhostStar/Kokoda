use kokoda_media::BackendResult as Result;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use windows_capture::{
    capture::{CaptureControl, Context, GraphicsCaptureApiHandler},
    encoder::{ImageEncoder, ImageEncoderPixelFormat, ImageFormat},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
    window::Window,
};

#[derive(Clone)]
pub struct EncodedFrame {
    pub sequence: u64,
    pub epoch: u64,
    pub width: u32,
    pub height: u32,
    pub encoded_width: u32,
    pub encoded_height: u32,
    pub data: Arc<Vec<u8>>,
    pub alpha_png: Option<Arc<Vec<u8>>>,
    pub format: &'static str,
    pub encoder: &'static str,
    pub encode_ms: f64,
}

type State = Arc<Mutex<std::result::Result<Option<EncodedFrame>, String>>>;

struct Handler {
    state: State,
    next_frame: Option<Instant>,
    sequence: u64,
    encoder: ImageEncoder,
    packed: Vec<u8>,
    format: &'static str,
    fps: Arc<std::sync::atomic::AtomicU32>,
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = (
        State,
        bool,
        Option<crate::video::VideoOptions>,
        Arc<std::sync::atomic::AtomicU32>,
    );
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self> {
        Ok(Self {
            state: ctx.flags.0,
            format: if ctx.flags.2.is_some() {
                "bgra"
            } else if ctx.flags.1 {
                "png"
            } else {
                "jpeg"
            },
            fps: ctx.flags.3,
            next_frame: None,
            sequence: 0,
            packed: Vec::new(),
            encoder: ImageEncoder::new(
                if ctx.flags.1 {
                    ImageFormat::Png
                } else {
                    ImageFormat::Jpeg
                },
                ImageEncoderPixelFormat::Bgra8,
            )?,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> Result<()> {
        let now = Instant::now();
        if !capture_due(
            &mut self.next_frame,
            now,
            self.fps.load(std::sync::atomic::Ordering::Relaxed),
        ) {
            return Ok(());
        }
        let result = (|| -> Result<EncodedFrame> {
            let width = frame.width();
            let height = frame.height();
            if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 33_177_600 {
                return Err("Unsupported window dimensions".into());
            }
            let buffer = frame.buffer()?;
            let pixels = buffer.as_nopadding_buffer(&mut self.packed);
            let jpeg = if self.format == "bgra" {
                pixels.to_vec()
            } else {
                self.encoder.encode(pixels, width, height)?
            };
            self.sequence += 1;
            Ok(EncodedFrame {
                sequence: self.sequence,
                epoch: 0,
                width,
                height,
                encoded_width: width,
                encoded_height: height,
                data: Arc::new(jpeg),
                alpha_png: None,
                format: self.format,
                encoder: "image",
                encode_ms: 0.0,
            })
        })();
        let mut state = self.state.lock().map_err(|_| "Capture state poisoned")?;
        match result {
            Ok(frame) => {
                let duplicate = state
                    .as_ref()
                    .ok()
                    .and_then(|v| v.as_ref())
                    .is_some_and(|previous| same_image(previous, &frame));
                if !duplicate {
                    *state = Ok(Some(frame));
                }
            }
            Err(error) => {
                *state = Err(error.to_string());
                control.stop();
            }
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<()> {
        *self.state.lock().map_err(|_| "Capture state poisoned")? = Err("Window closed".into());
        Ok(())
    }
}

// Only independent image frames can be suppressed. Raw video still needs to
// reach the encoder; never drop encoded H.264 reference frames here.
fn same_image(previous: &EncodedFrame, next: &EncodedFrame) -> bool {
    matches!(next.format, "png" | "jpeg")
        && previous.format == next.format
        && previous.width == next.width
        && previous.height == next.height
        && previous.data == next.data
}

pub struct LiveCapture {
    state: State,
    fps: Arc<std::sync::atomic::AtomicU32>,
    control: Option<CaptureControl<Handler, Box<dyn std::error::Error + Send + Sync>>>,
}

impl LiveCapture {
    pub fn start(
        id: u64,
        transparent: bool,
        video: Option<crate::video::VideoOptions>,
    ) -> Result<Self> {
        let state = Arc::new(Mutex::new(Ok(None)));
        let fps = Arc::new(std::sync::atomic::AtomicU32::new(
            video.map_or(15, |v| v.fps),
        ));
        let settings = Settings::new(
            Window::from_raw_hwnd(id as usize as *mut std::ffi::c_void),
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::Default,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            (state.clone(), transparent, video, fps.clone()),
        );
        let control = Handler::start_free_threaded(settings)?;
        Ok(Self {
            state,
            fps,
            control: Some(control),
        })
    }

    pub fn set_fps(&self, fps: u32) {
        self.fps.store(fps, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn latest(&self, after: u64) -> Result<Option<EncodedFrame>> {
        let state = self.state.lock().map_err(|_| "Capture state poisoned")?;
        match &*state {
            Ok(Some(frame)) if frame.sequence > after => Ok(Some(frame.clone())),
            Ok(_) => Ok(None),
            Err(error) => Err(std::io::Error::other(error.clone()).into()),
        }
    }
}

impl Drop for LiveCapture {
    fn drop(&mut self) {
        if let Some(control) = self.control.take() {
            if let Err(error) = control.stop() {
                eprintln!("Capture shutdown failed: {error}");
            }
        }
    }
}

// Start deadlines keep readback cost and normal capture jitter from halving FPS.
fn capture_due(next: &mut Option<Instant>, now: Instant, fps: u32) -> bool {
    let period = Duration::from_secs_f64(1.0 / fps as f64);
    if next.is_some_and(|due| now + Duration::from_millis(1) < due) {
        return false;
    }
    *next = Some(
        next.map(|due| due + period)
            .filter(|due| *due > now)
            .unwrap_or(now + period),
    );
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_images_are_suppressed_but_video_and_size_changes_are_not() {
        let mut next = EncodedFrame {
            sequence: 2,
            epoch: 0,
            width: 2,
            height: 2,
            encoded_width: 2,
            encoded_height: 2,
            data: Arc::new(vec![1, 2, 3]),
            format: "png",
            alpha_png: None,
            encoder: "image",
            encode_ms: 0.0,
        };
        let previous = next.clone();
        assert!(same_image(&previous, &next));
        next.width = 4;
        assert!(!same_image(&previous, &next));
        next = previous.clone();
        next.data = Arc::new(vec![1, 2, 4]);
        assert!(!same_image(&previous, &next));
        next.format = "bgra";
        assert!(!same_image(&next, &next));
        next.format = "h264";
        assert!(!same_image(&next, &next));
    }
    #[test]
    fn thirty_hz_jitter_does_not_halve_capture_rate() {
        let start = Instant::now();
        let mut next = None;
        for i in 0..300 {
            assert!(capture_due(
                &mut next,
                start + Duration::from_micros(i * 33333),
                30
            ));
        }
        assert!(!capture_due(
            &mut next,
            start + Duration::from_micros(299 * 33333 + 1000),
            30
        ));
    }
}
