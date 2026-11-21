//! Native window facts and actions. Called only after host scope/generation checks.
use kokoda_app_model::window_management::{
    role, Presentation, ProcessFact, WindowRole, WindowTraits,
};
use kokoda_media::BackendResult as Result;
use std::{collections::BTreeSet, io, mem};
use winapi::{
    shared::{
        minwindef::DWORD,
        windef::{HWND, RECT},
    },
    um::{
        handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
        tlhelp32::*,
        winuser::*,
    },
};

pub fn presentation(id: u64) -> Presentation {
    unsafe {
        let hwnd = id as usize as HWND;
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let owner = GetWindow(hwnd, GW_OWNER);
        let mut name = [0u16; 128];
        let n = GetClassNameW(hwnd, name.as_mut_ptr(), 128);
        let class = String::from_utf16_lossy(&name[..n.max(0) as usize]).to_ascii_lowercase();
        let transient = matches!(class.as_str(), "#32768" | "tooltips_class32" | "combolbox");
        let role = role(WindowTraits {
            owned: !owner.is_null(),
            tool: ex & WS_EX_TOOLWINDOW != 0,
            app: ex & WS_EX_APPWINDOW != 0,
            owner_disabled: !owner.is_null() && IsWindowEnabled(owner) == 0,
            transient,
        });
        let menu = GetSystemMenu(hwnd, 0);
        let close_state = if menu.is_null() {
            u32::MAX
        } else {
            GetMenuState(menu, SC_CLOSE as u32, MF_BYCOMMAND)
        };
        Presentation {
            // Capture bounds are physical even for DPI-unaware applications.
            dpi: capture_dpi(hwnd),
            role,
            owner: (!owner.is_null()).then_some(owner as usize as u64),
            enabled: IsWindowEnabled(hwnd) != 0,
            minimized: IsIconic(hwnd) != 0,
            maximized: IsZoomed(hwnd) != 0,
            can_maximize: role == WindowRole::Independent && style & WS_MAXIMIZEBOX != 0,
            can_close: role != WindowRole::Transient
                && style & WS_SYSMENU != 0
                && close_state != u32::MAX
                && close_state & (MF_DISABLED | MF_GRAYED) == 0,
            can_minimize: role == WindowRole::Independent && style & WS_MINIMIZEBOX != 0,
            can_resize: role != WindowRole::Transient
                && style & WS_THICKFRAME != 0
                && IsZoomed(hwnd) == 0,
        }
    }
}

#[link(name = "shcore")]
extern "system" {
    fn GetScaleFactorForMonitor(monitor: winapi::shared::windef::HMONITOR, scale: *mut u32) -> i32;
}

unsafe fn capture_dpi(hwnd: HWND) -> u32 {
    let mut scale = 100;
    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    if GetScaleFactorForMonitor(monitor, &mut scale) >= 0 {
        ((scale * 96 + 50) / 100).max(96)
    } else {
        GetDpiForWindow(hwnd).max(96)
    }
}

pub fn action(id: u64, request: &serde_json::Value) -> Result<()> {
    let action = request["action"].as_str().ok_or("Expected window action")?;
    let p = presentation(id);
    if !p.enabled {
        return Err("Window is blocked by a modal dialog".into());
    }
    unsafe {
        let hwnd = id as usize as HWND;
        match action {
            "close" if p.can_close => {
                if PostMessageW(hwnd, WM_CLOSE, 0, 0) == 0 {
                    return Err(io::Error::last_os_error().into());
                }
            }
            "minimize" if p.can_minimize => {
                if ShowWindowAsync(hwnd, SW_MINIMIZE) == 0 {
                    return Err(io::Error::last_os_error().into());
                }
            }
            "maximize" if p.can_maximize => {
                if ShowWindowAsync(hwnd, SW_MAXIMIZE) == 0 {
                    return Err(io::Error::last_os_error().into());
                }
            }
            "restore" if p.minimized || p.maximized => {
                if ShowWindowAsync(hwnd, SW_RESTORE) == 0 {
                    return Err(io::Error::last_os_error().into());
                }
            }
            "restore" => {}
            "resize" if p.can_resize && !p.minimized => {
                let w = request["width"].as_i64().ok_or("Expected window width")?;
                let h = request["height"].as_i64().ok_or("Expected window height")?;
                if !(64..=8192).contains(&w) || !(64..=8192).contains(&h) || w * h > 33_177_600 {
                    return Err("Invalid window size".into());
                }
                let mut outer: RECT = mem::zeroed();
                if GetWindowRect(hwnd, &mut outer) == 0 {
                    return Err(io::Error::last_os_error().into());
                }
                let mut window = crate::windows::inspect(id).ok_or("Window unavailable")?;
                crate::windows::capture_bounds(&mut window);
                let b = window.bounds;
                let border_x = (outer.right - outer.left) - (b.right - b.left);
                let border_y = (outer.bottom - outer.top) - (b.bottom - b.top);
                let mut limits: MINMAXINFO = mem::zeroed();
                limits.ptMinTrackSize.x = GetSystemMetrics(SM_CXMINTRACK);
                limits.ptMinTrackSize.y = GetSystemMetrics(SM_CYMINTRACK);
                limits.ptMaxTrackSize.x = 8192;
                limits.ptMaxTrackSize.y = 8192;
                let mut result = 0;
                if SendMessageTimeoutW(
                    hwnd,
                    WM_GETMINMAXINFO,
                    0,
                    &mut limits as *mut _ as isize,
                    SMTO_ABORTIFHUNG | SMTO_BLOCK,
                    100,
                    &mut result,
                ) == 0
                {
                    return Err("Window size constraints unavailable".into());
                }
                let width = (w as i32 + border_x).clamp(
                    limits.ptMinTrackSize.x.max(64).min(8192),
                    limits
                        .ptMaxTrackSize
                        .x
                        .clamp(limits.ptMinTrackSize.x.max(64).min(8192), 8192),
                );
                let height = (h as i32 + border_y).clamp(
                    limits.ptMinTrackSize.y.max(64).min(8192),
                    limits
                        .ptMaxTrackSize
                        .y
                        .clamp(limits.ptMinTrackSize.y.max(64).min(8192), 8192),
                );
                let coordinate = |key: &str, fallback: i32| -> Result<i32> {
                    let v = if request.get(key).is_some() {
                        request[key].as_i64().ok_or("Invalid window position")?
                    } else {
                        i64::from(fallback)
                    };
                    if !(-131072..=131072).contains(&v) {
                        return Err("Invalid window position".into());
                    }
                    Ok(v as i32)
                };
                let left = coordinate("left", b.left)? + outer.left - b.left;
                let top = coordinate("top", b.top)? + outer.top - b.top;
                if SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    left,
                    top,
                    width,
                    height,
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS,
                ) == 0
                {
                    return Err(io::Error::last_os_error().into());
                }
            }
            _ => return Err("Window action is not supported".into()),
        }
    }
    Ok(())
}

/// Read the selected family tree only; executable paths never leave the host.
pub fn process_facts(seeds: &[u32]) -> io::Result<Vec<ProcessFact>> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut entry: PROCESSENTRY32W = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as DWORD;
        let mut pairs = Vec::new();
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 && pairs.len() < 16384 {
            pairs.push((entry.th32ProcessID, entry.th32ParentProcessID));
            ok = Process32NextW(snapshot, &mut entry)
        }
        CloseHandle(snapshot);
        let mut candidates: BTreeSet<_> = seeds.iter().copied().collect();
        for _ in 0..16 {
            let before = candidates.len();
            for (pid, parent) in &pairs {
                if candidates.len() >= 256 {
                    break;
                }
                if candidates.contains(parent) {
                    candidates.insert(*pid);
                }
            }
            if before == candidates.len() {
                break;
            }
        }
        Ok(pairs
            .into_iter()
            .filter(|(pid, _)| candidates.contains(pid))
            .filter_map(|(pid, parent)| {
                let (executable, started) = crate::windows::process_details(pid).ok()?;
                Some(ProcessFact {
                    pid,
                    parent,
                    started,
                    executable,
                })
            })
            .collect())
    }
}

/// System menus/dropdowns receive input through their existing foreground UI
/// thread; trying to activate their popup HWND would dismiss the menu instead.
pub fn transient_in_foreground(id: u64) -> bool {
    unsafe {
        let hwnd = id as usize as HWND;
        let mut class = [0u16; 128];
        let n = GetClassNameW(hwnd, class.as_mut_ptr(), 128);
        let name = String::from_utf16_lossy(&class[..n.max(0) as usize]).to_ascii_lowercase();
        if !matches!(name.as_str(), "#32768" | "combolbox") {
            return false;
        }
        let foreground = GetForegroundWindow();
        !foreground.is_null()
            && GetWindowThreadProcessId(hwnd, std::ptr::null_mut())
                == GetWindowThreadProcessId(foreground, std::ptr::null_mut())
    }
}
