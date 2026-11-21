use kokoda_media::BackendResult as Result;
use std::{marker::PhantomData, rc::Rc};

#[link(name = "runtimeobject")]
extern "system" {
    fn RoInitialize(init_type: u32) -> i32;
    fn RoUninitialize();
}

/// Keep the host's WinRT MTA alive between short-lived capture threads.
/// WinRT activation factories cached by windows-rs must not outlive the runtime.
/// The guard is thread-affine and must outlive every capture session.
pub struct Apartment(PhantomData<Rc<()>>);

impl Apartment {
    pub fn new() -> Result<Self> {
        // RO_INIT_MULTITHREADED. Both S_OK and S_FALSE require a matching uninitialize.
        let result = unsafe { RoInitialize(1) };
        if result < 0 {
            return Err(format!("WinRT initialization failed: 0x{:08x}", result as u32).into());
        }
        unsafe {
            windows::Win32::Media::MediaFoundation::MFStartup(
                windows::Win32::Media::MediaFoundation::MF_VERSION,
                0,
            )?
        };
        Ok(Self(PhantomData))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Media::MediaFoundation::MFShutdown();
            RoUninitialize()
        };
    }
}
