//! Thread-affine capture/encoder with bounded output. Encoded H264 reference
//! frames are never discarded; while blocked, capture retains the newest raw frame.
use crate::{
    live_capture::{EncodedFrame, LiveCapture},
    video::{Encoder, VideoOptions},
};
use kokoda_media::BackendResult as Result;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
struct Config {
    viewport: Option<(u32, u32)>,
    video: Option<VideoOptions>,
    epoch: u64,
}

pub struct VideoWorker {
    output: mpsc::Receiver<std::result::Result<EncodedFrame, String>>,
    config: Arc<Mutex<Config>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl VideoWorker {
    pub fn start(
        id: u64,
        transparent: bool,
        video: Option<VideoOptions>,
        viewport: Option<(u32, u32)>,
        epoch: u64,
        permitted: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    ) -> Self {
        let (tx, output) = mpsc::sync_channel(1);
        let config = Arc::new(Mutex::new(Config {
            viewport,
            video,
            epoch,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (v, stopped) = (config.clone(), stop.clone());
        let thread = thread::spawn(move || {
            let run = || -> Result<()> {
                let _apartment = crate::runtime::Apartment::new()?;
                let identity = crate::input::Target::new(id)?;
                let check = || -> Result<()> {
                    identity.validate()?;
                    if permitted.as_ref().is_some_and(|p| !p()) {
                        return Err("Window sharing permission revoked".into());
                    }
                    Ok(())
                };
                check()?;
                let mut checked = Instant::now();
                let capture = LiveCapture::start(id, transparent, video)?;
                let mut encoder: Option<Encoder> = None;
                let mut alpha = crate::alpha::AlphaMask::new()?;
                let mut previous_viewport = viewport;
                let mut previous_epoch = epoch;
                let mut hardware_failed = false;
                let mut empty_outputs = 0;
                let (mut source_sequence, mut output_sequence) = (0, 0);
                while !stopped.load(Ordering::Relaxed) {
                    if checked.elapsed() >= Duration::from_millis(100) {
                        check()?;
                        checked = Instant::now();
                    }
                    let config = *v.lock().unwrap();
                    let current_viewport = config.viewport;
                    if config.epoch != previous_epoch {
                        encoder = None;
                        source_sequence = 0;
                        empty_outputs = 0;
                        previous_epoch = config.epoch;
                        capture.set_fps(config.video.map_or(15, |v| v.fps));
                    }
                    if current_viewport != previous_viewport {
                        source_sequence = 0;
                        previous_viewport = current_viewport;
                    }
                    let Some(mut frame) = capture.latest(source_sequence)? else {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    };
                    check()?;
                    source_sequence = frame.sequence;
                    if let Some(mut options) = config.video {
                        options.hardware &= !hardware_failed;
                        let source = (frame.width, frame.height);
                        let viewport = config.viewport;
                        let started = Instant::now();
                        if encoder.as_ref().is_none_or(|e| {
                            e.source != source
                                || e.size != crate::video::encoded_size(source, options, viewport)
                        }) {
                            encoder = Some(Encoder::new(source, options, viewport)?);
                            if options.hardware
                                && encoder.as_ref().unwrap().backend() == "software-mf"
                            {
                                hardware_failed = true;
                                eprintln!("H264: software fallback for this window; hardware probing will not repeat on resize or preset changes");
                            }
                        }
                        let mut result = encoder.as_mut().unwrap().encode(&frame.data);
                        if matches!(result, Ok(None)) {
                            empty_outputs += 1;
                            if empty_outputs >= 8 {
                                result =
                                    Err("Encoder produced no output after eight inputs".into());
                            }
                        } else {
                            empty_outputs = 0;
                        }
                        let data = match result {
                            Ok(data) => data,
                            Err(error) if encoder.as_ref().unwrap().backend() != "software-mf" => {
                                eprintln!(
                                    "Hardware encoder failed, falling back to software: {error}"
                                );
                                hardware_failed = true;
                                options.hardware = false;
                                encoder = Some(Encoder::new(source, options, viewport)?);
                                encoder.as_mut().unwrap().encode(&frame.data)?
                            }
                            Err(error) => return Err(error),
                        };
                        let Some(data) = data else {
                            source_sequence = 0;
                            thread::sleep(Duration::from_millis(4));
                            continue;
                        };
                        empty_outputs = 0;
                        let e = encoder.as_ref().unwrap();
                        if transparent {
                            frame.alpha_png = alpha.encode(
                                &frame.data,
                                source,
                                crate::windows::layer_alpha(id),
                            )?;
                        }
                        frame.data = Arc::new(data);
                        frame.format = "h264";
                        frame.encoder = e.backend();
                        frame.encode_ms = started.elapsed().as_secs_f64() * 1000.0;
                        (frame.encoded_width, frame.encoded_height) = e.size;
                    }
                    output_sequence += 1;
                    frame.sequence = output_sequence;
                    frame.epoch = config.epoch;
                    check()?;
                    checked = Instant::now();
                    let mut message = Ok(frame);
                    loop {
                        if stopped.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        if checked.elapsed() >= Duration::from_millis(50) {
                            check()?;
                            checked = Instant::now();
                        }
                        match tx.try_send(message) {
                            Ok(()) => break,
                            Err(mpsc::TrySendError::Full(value)) => {
                                message = value;
                                thread::sleep(Duration::from_millis(2));
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => return Ok(()),
                        }
                    }
                }
                Ok(())
            };
            if let Err(error) = run() {
                let _ = tx.try_send(Err(error.to_string()));
            }
        });
        Self {
            output,
            config,
            stop,
            thread: Some(thread),
        }
    }
    pub fn frame(
        &self,
        viewport: Option<(u32, u32)>,
        video: Option<VideoOptions>,
        epoch: u64,
    ) -> Result<Option<EncodedFrame>> {
        *self.config.lock().unwrap() = Config {
            viewport,
            video,
            epoch,
        };
        match self.output.try_recv() {
            Ok(Ok(frame)) => Ok(Some(frame)),
            Ok(Err(e)) => Err(e.into()),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err("Video worker stopped".into()),
        }
    }
}
impl Drop for VideoWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
