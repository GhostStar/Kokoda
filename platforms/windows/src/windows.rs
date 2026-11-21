use kokoda_app_model::{AppWindow, WindowBounds};
use std::{io, mem};
use winapi::{
    shared::{
        minwindef::{BOOL, LPARAM, TRUE},
        windef::{HWND, RECT},
    },
    um::winuser::{
        EnumWindows, GetDesktopWindow, GetShellWindow, GetWindow, GetWindowRect,
        GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        GW_OWNER,
    },
};

unsafe extern "system" fn collect(hwnd: HWND, context: LPARAM) -> BOOL {
    if IsWindowVisible(hwnd) == 0 || hwnd == GetDesktopWindow() || hwnd == GetShellWindow() {
        return TRUE;
    }
    let mut process_id = 0;
    let thread_id = GetWindowThreadProcessId(hwnd, &mut process_id);
    let mut rect: RECT = mem::zeroed();
    if thread_id == 0 || GetWindowRect(hwnd, &mut rect) == 0 {
        return TRUE;
    }
    let length = GetWindowTextLengthW(hwnd);
    let mut title = vec![0u16; length.max(0) as usize + 1];
    let copied = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
    let owner = GetWindow(hwnd, GW_OWNER);
    let windows = &mut *(context as *mut Vec<AppWindow>);
    windows.push(AppWindow {
        id: hwnd as usize as u64,
        process_id,
        thread_id,
        owner: if owner.is_null() {
            None
        } else {
            Some(owner as usize as u64)
        },
        title: String::from_utf16_lossy(&title[..copied.max(0) as usize]),
        bounds: WindowBounds {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        },
        minimized: IsIconic(hwnd) != 0,
    });
    TRUE
}

pub fn enumerate() -> io::Result<Vec<AppWindow>> {
    let mut windows = Vec::<AppWindow>::new();
    // EnumWindows invokes the callback synchronously on this thread.
    if unsafe { EnumWindows(Some(collect), &mut windows as *mut _ as LPARAM) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(windows)
}

/// Capture and input use physical DWM bounds, excluding invisible resize borders.
pub fn capture_bounds(window: &mut AppWindow) {
    let mut rect: RECT = unsafe { mem::zeroed() };
    let result = unsafe {
        winapi::um::dwmapi::DwmGetWindowAttribute(
            window.id as usize as HWND,
            winapi::um::dwmapi::DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut _,
            mem::size_of::<RECT>() as u32,
        )
    };
    if result >= 0 {
        window.bounds = WindowBounds {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        };
    }
}

pub fn foreground() -> u64 {
    unsafe { winapi::um::winuser::GetForegroundWindow() as usize as u64 }
}

/// Must be called before enumerating or capturing windows.
pub fn initialize_dpi() {
    unsafe {
        winapi::um::shellscalingapi::SetProcessDpiAwareness(2);
    }
}
pub fn visible(id: u64) -> bool {
    unsafe { IsWindowVisible(id as usize as HWND) != 0 }
}

/// A service/session-0 process cannot capture the signed-in user's desktop.
/// Fail before listening instead of returning a misleading empty catalog.
pub fn require_interactive_session() -> io::Result<()> {
    let mut session = 0;
    if unsafe {
        winapi::um::processthreadsapi::ProcessIdToSessionId(std::process::id(), &mut session)
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if session == 0 {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "Start Kokoda in the signed-in user's interactive session; Session 0 cannot share application windows"));
    }
    Ok(())
}

/// Host-local facts for sharing rules; never serialized into the remote catalog.
pub fn process_details(pid: u32) -> io::Result<(String, u64)> {
    use winapi::um::{
        handleapi::CloseHandle,
        processthreadsapi::{GetProcessTimes, OpenProcess},
        winbase::QueryFullProcessImageNameW,
        winnt::PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut path = vec![0u16; 32768];
        let mut length = path.len() as u32;
        let mut created = mem::zeroed();
        let mut exited = mem::zeroed();
        let mut kernel = mem::zeroed();
        let mut user = mem::zeroed();
        let result = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length) != 0
            && GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) != 0;
        let error = io::Error::last_os_error();
        CloseHandle(process);
        if !result {
            return Err(error);
        }
        let path = String::from_utf16(&path[..length as usize]).map_err(io::Error::other)?;
        let path = std::fs::canonicalize(path)?
            .into_os_string()
            .into_string()
            .map_err(|_| io::Error::other("Executable path is not Unicode"))?;
        Ok((
            path,
            ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64,
        ))
    }
}

/// Inspect one live shareable top-level window without enumerating unrelated apps.
pub fn inspect(id: u64) -> Option<AppWindow> {
    let mut windows = Vec::<AppWindow>::new();
    unsafe {
        collect(id as usize as HWND, &mut windows as *mut _ as LPARAM);
    }
    windows.pop()
}

#[cfg(test)]
mod sharing_tests {
    #[test]
    fn process_facts_use_the_resolved_executable_and_stable_creation_time() {
        let first = super::process_details(std::process::id()).unwrap();
        assert_eq!(first, super::process_details(std::process::id()).unwrap());
        assert_ne!(first.1, 0);
        assert_eq!(
            first.0,
            std::fs::canonicalize(std::env::current_exe().unwrap())
                .unwrap()
                .to_str()
                .unwrap()
        );
    }
}

/// WGC captures window content, but native uniform opacity/color-key composition
/// may be applied later by DWM. Carry those actual attributes into the mask.
pub fn layer_alpha(id: u64) -> crate::alpha::LayerAlpha {
    use winapi::um::winuser::{GetLayeredWindowAttributes, LWA_ALPHA, LWA_COLORKEY};
    let (mut key, mut opacity, mut flags) = (0, 255, 0);
    if unsafe {
        GetLayeredWindowAttributes(id as usize as HWND, &mut key, &mut opacity, &mut flags)
    } == 0
    {
        return crate::alpha::LayerAlpha::default();
    }
    crate::alpha::LayerAlpha {
        opacity: if flags & LWA_ALPHA != 0 { opacity } else { 255 },
        color_key: (flags & LWA_COLORKEY != 0).then_some([
            key as u8,
            (key >> 8) as u8,
            (key >> 16) as u8,
        ]),
    }
}
