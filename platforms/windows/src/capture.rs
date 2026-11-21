use std::{error::Error, io, path::PathBuf, sync::mpsc, time::Duration};
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    encoder::ImageFormat,
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
    window::Window,
};

type CaptureError = Box<dyn Error + Send + Sync>;
type CaptureFlags = (PathBuf, mpsc::Sender<Result<(), String>>);

struct Snapshot {
    path: PathBuf,
    result: mpsc::Sender<Result<(), String>>,
}

impl GraphicsCaptureApiHandler for Snapshot {
    type Flags = CaptureFlags;
    type Error = CaptureError;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            path: ctx.flags.0,
            result: ctx.flags.1,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let result = frame
            .save_as_image(&self.path, ImageFormat::Png)
            .map_err(|error| error.to_string());
        control.stop();
        self.result.send(result)?;
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        // A frame result already queued before closure takes precedence.
        let _ = self
            .result
            .send(Err("Target window closed before capture completed".into()));
        Ok(())
    }
}

pub fn snapshot(id: u64, path: PathBuf) -> Result<(), CaptureError> {
    let windows = crate::windows::enumerate()?;
    let selected = windows
        .iter()
        .find(|window| window.id == id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Window is no longer available"))?;
    if selected.minimized {
        return Err(io::Error::other("Restore the window before capturing").into());
    }
    if path.exists() {
        return Err(
            io::Error::new(io::ErrorKind::AlreadyExists, "Output file already exists").into(),
        );
    }
    let window = Window::from_raw_hwnd(id as usize as *mut std::ffi::c_void);
    let (sender, receiver) = mpsc::channel();
    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::Default,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        (path, sender),
    );
    let capture = Snapshot::start_free_threaded(settings)?;
    let result = receiver.recv_timeout(Duration::from_secs(15));
    capture.stop()?;
    result?.map_err(|message| io::Error::other(message).into())
}
