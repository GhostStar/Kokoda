//! Authenticated text/image clipboard transactions and scoped change polling.
//! An owned message-only HWND is required when
//! replacing clipboard data (OpenClipboard(NULL) cannot establish ownership).
use kokoda_media::BackendResult as Result;
use std::{
    ffi::c_void,
    mem, ptr, thread,
    time::{Duration, Instant},
};
use winapi::{shared::windef::HWND, um::winuser::*};
const MAX_TEXT: usize = 1024 * 1024;
#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    fn GlobalFree(memory: *mut c_void) -> *mut c_void;
    fn GlobalLock(memory: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(memory: *mut c_void) -> i32;
    fn GlobalSize(memory: *mut c_void) -> usize;
}
// Outer errors (e.g. busy clipboard) can be retried. An observed payload error
// carries its sequence so the session reports it once instead of decoding it forever.
pub type Poll = (u32, Result<Option<Content>>);
pub enum Content {
    Text(String),
    Png(Vec<u8>),
}
fn png_format() -> u32 {
    let name: Vec<u16> = "PNG\0".encode_utf16().collect();
    unsafe { RegisterClipboardFormatW(name.as_ptr()) }
}
struct Allocation(*mut c_void);
impl Drop for Allocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                GlobalFree(self.0);
            }
        }
    }
}
impl Allocation {
    fn new(data: &[u8]) -> Result<Self> {
        unsafe {
            let memory = Self(GlobalAlloc(0x42, data.len()));
            if memory.0.is_null() {
                return Err("Clipboard allocation failed".into());
            }
            let target = GlobalLock(memory.0) as *mut u8;
            if target.is_null() {
                return Err("Clipboard memory lock failed".into());
            }
            ptr::copy_nonoverlapping(data.as_ptr(), target, data.len());
            GlobalUnlock(memory.0);
            Ok(memory)
        }
    }
}
pub struct Clipboard {
    window: HWND,
    owner_thread: u32,
    worker: Option<thread::JoinHandle<()>>,
}
struct Open;
impl Drop for Open {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}
impl Clipboard {
    pub fn new() -> Result<Self> {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("clipboard-owner".into())
            .spawn(move || unsafe {
                let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
                let window = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
                if window.is_null() {
                    let _ = send.send(Err(std::io::Error::last_os_error().to_string()));
                    return;
                }
                let tid = winapi::um::processthreadsapi::GetCurrentThreadId();
                if send.send(Ok((window as usize, tid))).is_err() {
                    DestroyWindow(window);
                    return;
                }
                // Clipboard ownership sends synchronous messages even when all formats
                // are eagerly supplied. Never put this HWND on the blocking TLS thread.
                let mut message: MSG = mem::zeroed();
                while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                DestroyWindow(window);
            })?;
        let (window, owner_thread) = receive.recv()?.map_err(std::io::Error::other)?;
        Ok(Self {
            window: window as HWND,
            owner_thread,
            worker: Some(worker),
        })
    }
    fn open(&self) -> Result<Open> {
        for _ in 0..20 {
            if unsafe { OpenClipboard(self.window) } != 0 {
                return Ok(Open);
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err("Clipboard is busy; try again".into())
    }
    pub fn sequence(&self) -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }
    pub fn set(&self, text: &str) -> Result<()> {
        if text.len() > MAX_TEXT || text.contains('\0') {
            return Err("Text clipboard limit is 1 MiB; NUL is unsupported".into());
        }
        let normalized = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\n', "\r\n");
        let data: Vec<u16> = normalized.encode_utf16().chain(Some(0)).collect();
        if data.len() * 2 > MAX_TEXT * 2 {
            return Err("Clipboard text is too large".into());
        }
        unsafe {
            let memory = GlobalAlloc(0x42, data.len() * 2); // GMEM_MOVEABLE | GMEM_ZEROINIT
            if memory.is_null() {
                return Err("Clipboard allocation failed".into());
            }
            let target = GlobalLock(memory) as *mut u16;
            if target.is_null() {
                GlobalFree(memory);
                return Err("Clipboard memory lock failed".into());
            }
            ptr::copy_nonoverlapping(data.as_ptr(), target, data.len());
            GlobalUnlock(memory);
            let open = match self.open() {
                Ok(v) => v,
                Err(e) => {
                    GlobalFree(memory);
                    return Err(e);
                }
            };
            if EmptyClipboard() == 0 || SetClipboardData(CF_UNICODETEXT, memory as _).is_null() {
                GlobalFree(memory);
                return Err("Cannot set Windows clipboard".into());
            }
            drop(open); // Windows now owns the allocation.
        }
        Ok(())
    }
    pub fn set_png(&self, encoded: &str) -> Result<()> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        if encoded.len() > crate::clipboard_image::MAX_PNG.div_ceil(3) * 4 {
            return Err("Clipboard PNG exceeds 8 MiB".into());
        }
        let png = STANDARD.decode(encoded)?;
        let (v5, dib) = crate::clipboard_image::png_to_dibs(&png)?;
        let format = png_format();
        if format == 0 {
            return Err("Cannot register PNG format".into());
        }
        let mut data = vec![
            (format, Allocation::new(&png)?),
            (CF_DIBV5, Allocation::new(&v5)?),
            (CF_DIB, Allocation::new(&dib)?),
        ];
        let _open = self.open()?;
        unsafe {
            if EmptyClipboard() == 0 {
                return Err("Cannot empty clipboard".into());
            }
            for (format, memory) in &mut data {
                if SetClipboardData(*format, memory.0 as _).is_null() {
                    return Err("Cannot set image clipboard".into());
                }
                memory.0 = ptr::null_mut(); // Ownership transferred to Windows.
            }
        }
        Ok(())
    }
    fn read_bytes(&self, format: u32, limit: usize) -> Result<Vec<u8>> {
        unsafe {
            let memory = GetClipboardData(format);
            if memory.is_null() {
                return Err("Cannot read clipboard image".into());
            }
            let size = GlobalSize(memory as _);
            if size == 0 || size > limit {
                return Err("Clipboard image exceeds size limit".into());
            }
            let data = GlobalLock(memory as _) as *const u8;
            if data.is_null() {
                return Err("Cannot lock clipboard image".into());
            }
            let result = std::slice::from_raw_parts(data, size).to_vec();
            GlobalUnlock(memory as _);
            Ok(result)
        }
    }
    fn read_content(&self) -> Result<Option<Content>> {
        unsafe {
            if IsClipboardFormatAvailable(CF_HDROP) != 0 {
                return Err("File clipboard is not supported".into());
            }
            let png = png_format();
            if png != 0 && IsClipboardFormatAvailable(png) != 0 {
                let data = self.read_bytes(png, crate::clipboard_image::MAX_PNG)?;
                crate::clipboard_image::validate_png(&data)?;
                return Ok(Some(Content::Png(data)));
            }
            for format in [CF_DIBV5, CF_DIB] {
                if IsClipboardFormatAvailable(format) != 0 {
                    let dib = self.read_bytes(
                        format,
                        crate::clipboard_image::MAX_PIXELS as usize * 4 + 4096,
                    )?;
                    return Ok(Some(Content::Png(crate::clipboard_image::dib_to_png(
                        &dib,
                    )?)));
                }
            }
            if IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 {
                return Ok(self.read_text()?.map(Content::Text));
            }
        }
        Err("Clipboard contains neither text nor supported image data".into())
    }
    pub fn changed_content(&self, before: u32) -> Result<Option<Content>> {
        let start = Instant::now();
        while self.sequence() == before {
            if start.elapsed() > Duration::from_millis(700) {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _open = self.open()?;
        self.read_content()
    }
    // Check ownership and read under the same clipboard lock. Ignore our own writes
    // and changes made by applications outside the selected window group.
    pub fn poll(&self, before: u32, allowed: &[u32]) -> Result<Poll> {
        if self.sequence() == before {
            return Ok((before, Ok(None)));
        }
        let _open = self.open()?;
        let sequence = self.sequence();
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(GetClipboardOwner(), &mut pid);
        }
        if sequence == before || pid == 0 || !allowed.contains(&pid) {
            return Ok((sequence, Ok(None)));
        }
        // Unsupported native formats do not repeatedly interrupt the session.
        if unsafe { IsClipboardFormatAvailable(CF_HDROP) } != 0 {
            return Ok((sequence, Ok(None)));
        }
        let supported = unsafe {
            IsClipboardFormatAvailable(CF_UNICODETEXT) != 0
                || IsClipboardFormatAvailable(CF_DIB) != 0
                || IsClipboardFormatAvailable(CF_DIBV5) != 0
                || IsClipboardFormatAvailable(png_format()) != 0
        };
        if !supported {
            return Ok((sequence, Ok(None)));
        }
        Ok((sequence, self.read_content()))
    }
    fn read_text(&self) -> Result<Option<String>> {
        unsafe {
            if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
                return Err("Clipboard has no Unicode text".into());
            }
            let memory = GetClipboardData(CF_UNICODETEXT);
            if memory.is_null() {
                return Err("Cannot read Windows clipboard".into());
            }
            let bytes = GlobalSize(memory as _);
            if bytes < 2 || bytes > MAX_TEXT * 2 + 2 {
                return Err("Clipboard text is too large or invalid".into());
            }
            let data = GlobalLock(memory as _) as *const u16;
            if data.is_null() {
                return Err("Clipboard memory lock failed".into());
            }
            let slice = std::slice::from_raw_parts(data, bytes / mem::size_of::<u16>());
            let result = match slice.iter().position(|&v| v == 0) {
                Some(end) => String::from_utf16(&slice[..end])
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                None => Err("Clipboard text is not terminated".into()),
            };
            GlobalUnlock(memory as _);
            let text = result?;
            if text.len() > MAX_TEXT {
                return Err("Text clipboard limit is 1 MiB".into());
            }
            Ok(Some(text.replace("\r\n", "\n").replace('\r', "\n")))
        }
    }
}
impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            PostThreadMessageW(self.owner_thread, WM_QUIT, 0, 0);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
