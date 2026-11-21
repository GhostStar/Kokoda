//! FFmpeg hardware encoders. Vendor SDK handling stays inside libavcodec.
use kokoda_media::BackendResult as Result;
use std::{
    ffi::{c_char, c_void, CStr},
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
};
extern "C" {
    fn kokoda_ffmpeg_create(
        name: *const c_char,
        w: u32,
        h: u32,
        fps: u32,
        bitrate: u32,
        error: *mut c_char,
        capacity: usize,
    ) -> *mut c_void;
    fn kokoda_ffmpeg_encode(
        handle: *mut c_void,
        pixels: *const u8,
        length: usize,
        output: *mut u8,
        capacity: usize,
        written: *mut usize,
        error: *mut c_char,
        error_capacity: usize,
    ) -> i32;
    fn kokoda_ffmpeg_destroy(handle: *mut c_void);
}
pub struct Encoder {
    handle: NonNull<c_void>,
    size: (u32, u32),
    name: &'static str,
    _thread: PhantomData<Rc<()>>,
}
impl Encoder {
    pub fn new(size: (u32, u32), fps: u32, bitrate: u32) -> Result<Self> {
        let mut errors = Vec::new();
        for name in ["h264_nvenc", "h264_amf", "h264_qsv"] {
            match Self::named(name, size, fps, bitrate) {
                Ok(encoder) => return Ok(encoder),
                Err(error) => errors.push(format!("{name}: {error}")),
            }
        }
        Err(errors.join("; ").into())
    }
    pub fn backend(&self) -> &'static str {
        match self.name {
            "h264_nvenc" => "nvenc",
            "h264_amf" => "amf",
            "h264_qsv" => "qsv",
            "h264_mf" => "software-ffmpeg",
            _ => unreachable!(),
        }
    }
    pub fn named(name: &'static str, size: (u32, u32), fps: u32, bitrate: u32) -> Result<Self> {
        if size.0 < 16
            || size.1 < 16
            || size.0 > 3840
            || size.1 > 2160
            || size.0 % 2 != 0
            || size.1 % 2 != 0
            || !(5..=60).contains(&fps)
        {
            return Err("Invalid FFmpeg dimensions or frame rate".into());
        }
        if !["h264_nvenc", "h264_amf", "h264_qsv", "h264_mf"].contains(&name) {
            return Err("Unsupported FFmpeg encoder".into());
        }
        let c_name = std::ffi::CString::new(name)?;
        let mut error = [0 as c_char; 1024];
        let handle = unsafe {
            kokoda_ffmpeg_create(
                c_name.as_ptr(),
                size.0,
                size.1,
                fps,
                bitrate,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        let handle = NonNull::new(handle).ok_or_else(|| {
            unsafe { CStr::from_ptr(error.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        })?;
        Ok(Self {
            handle,
            size,
            name,
            _thread: PhantomData,
        })
    }
    pub fn encode(&mut self, nv12: &[u8]) -> Result<Option<Vec<u8>>> {
        if nv12.len() != self.size.0 as usize * self.size.1 as usize * 3 / 2 {
            return Err("Invalid FFmpeg NV12 buffer".into());
        }
        self.transfer(Some(nv12))
    }
    pub fn finish(&mut self) -> Result<Option<Vec<u8>>> {
        self.transfer(None)
    }
    fn transfer(&mut self, nv12: Option<&[u8]>) -> Result<Option<Vec<u8>>> {
        let capacity = (self.size.0 as usize * self.size.1 as usize * 2 + 65536)
            .min(kokoda_media::MAX_ENCODED_FRAME);
        let mut output = vec![0; capacity];
        let mut written = 0;
        let mut error = [0 as c_char; 1024];
        let result = unsafe {
            kokoda_ffmpeg_encode(
                self.handle.as_ptr(),
                nv12.map_or(std::ptr::null(), |data| data.as_ptr()),
                nv12.map_or(0, |data| data.len()),
                output.as_mut_ptr(),
                capacity,
                &mut written,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if result != 0 {
            return Err(unsafe { CStr::from_ptr(error.as_ptr()) }
                .to_string_lossy()
                .into_owned()
                .into());
        }
        if written > capacity {
            return Err("Invalid FFmpeg output length".into());
        }
        output.truncate(written);
        Ok((written != 0).then_some(output))
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { kokoda_ffmpeg_destroy(self.handle.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_invalid_dimensions_before_library_or_driver_access() {
        match super::Encoder::new((1, 720), 60, 8_000_000) {
            Ok(_) => panic!("Invalid FFmpeg size accepted"),
            Err(error) => assert!(error.to_string().contains("Invalid FFmpeg dimensions")),
        }
    }
}
