//! Display-only metadata, enriched only after the host has filtered its catalog.
//! Executable paths remain local. No foreign window messages or shell handlers run.
use base64::{engine::general_purpose::STANDARD, Engine};
use kokoda_app_model::AppWindow;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    mem,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use winapi::{
    shared::windef::HWND,
    um::{shellapi::ExtractIconExW, wingdi::*, winuser::*},
};

// DrawIconEx: draw both the icon image and its mask (DI_IMAGE | DI_MASK).
const DI_NORMAL: u32 = 3;

type IconCache = HashMap<String, (Instant, Option<String>)>;
static ICONS: OnceLock<Mutex<IconCache>> = OnceLock::new();

pub fn describe(windows: Vec<AppWindow>) -> Vec<Value> {
    let mut processes = HashMap::new();
    windows
        .into_iter()
        .filter_map(|w| {
            let details = processes
                .entry(w.process_id)
                .or_insert_with(|| crate::windows::process_details(w.process_id).ok());
            // An HWND may disappear or be reused while process metadata is collected.
            let current = crate::windows::inspect(w.id)?;
            // An owner stripped by the privacy policy must never be restored here.
            if current.process_id != w.process_id
                || current.thread_id != w.thread_id
                || current.title != w.title
                || (w.owner.is_some() && current.owner != w.owner)
            {
                return None;
            }
            let p = crate::window_management::presentation(w.id);
            let mut value = serde_json::to_value(&w).ok()?;
            let mut metadata = json!({"dpi":p.dpi,"role":p.role});
            if let Some((path, _)) = details {
                let basename = std::path::Path::new(path)
                    .file_name()?
                    .to_string_lossy()
                    .into_owned();
                metadata["executable"] = json!(basename);
                if let Some(icon) = icon(path) {
                    metadata["icon_rgba32"] = json!(icon);
                }
            }
            unsafe {
                let hwnd = w.id as usize as HWND;
                let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
                let mut cloaked: u32 = 0;
                winapi::um::dwmapi::DwmGetWindowAttribute(
                    hwnd,
                    14,
                    &mut cloaked as *mut _ as *mut _,
                    4,
                );
                let mut name = [0u16; 128];
                let n = GetClassNameW(hwnd, name.as_mut_ptr(), 128).max(0) as usize;
                let class = String::from_utf16_lossy(&name[..n]).to_ascii_lowercase();
                let shell = matches!(
                    class.as_str(),
                    "progman"
                        | "workerw"
                        | "shell_traywnd"
                        | "shell_secondarytraywnd"
                        | "#32768"
                        | "tooltips_class32"
                        | "combolbox"
                );
                // Preserve legitimate titleless normal windows; exclude only auxiliary
                // traits in the default view. The client can reveal all authorized rows.
                metadata["auxiliary"] = json!(
                    cloaked != 0
                        || shell
                        || (ex & WS_EX_APPWINDOW == 0
                            && (ex & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) != 0
                                || (w.title.trim().is_empty()
                                    && style & (WS_CAPTION | WS_THICKFRAME) == 0)))
                );
                let mut bounds = w.bounds.clone();
                if w.minimized {
                    let mut placement: WINDOWPLACEMENT = mem::zeroed();
                    placement.length = mem::size_of::<WINDOWPLACEMENT>() as u32;
                    if GetWindowPlacement(hwnd, &mut placement) != 0 {
                        let r = placement.rcNormalPosition;
                        bounds.left = r.left;
                        bounds.top = r.top;
                        bounds.right = r.right;
                        bounds.bottom = r.bottom;
                    }
                }
                metadata["width"] = json!((bounds.right - bounds.left).max(0));
                metadata["height"] = json!((bounds.bottom - bounds.top).max(0));
            }
            value["catalog"] = metadata;
            Some(value)
        })
        .collect()
}

fn icon(path: &str) -> Option<String> {
    let mut cache = ICONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()?;
    cache.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(60));
    if let Some((_, icon)) = cache.get(path) {
        return icon.clone();
    }
    if cache.len() >= 128 {
        cache.clear();
    }
    let result = unsafe { extract_icon(path) };
    cache.insert(path.to_owned(), (Instant::now(), result.clone()));
    result
}

unsafe fn extract_icon(path: &str) -> Option<String> {
    let wide: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
    let mut icon = std::ptr::null_mut();
    if ExtractIconExW(wide.as_ptr(), 0, &mut icon, std::ptr::null_mut(), 1) == 0 || icon.is_null() {
        return None;
    }
    let dc = CreateCompatibleDC(std::ptr::null_mut());
    if dc.is_null() {
        DestroyIcon(icon);
        return None;
    }
    let mut info: BITMAPINFO = mem::zeroed();
    info.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = 32;
    info.bmiHeader.biHeight = -32;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    let mut bits = std::ptr::null_mut();
    let bitmap = CreateDIBSection(
        dc,
        &info,
        DIB_RGB_COLORS,
        &mut bits,
        std::ptr::null_mut(),
        0,
    );
    if bitmap.is_null() {
        DeleteDC(dc);
        DestroyIcon(icon);
        return None;
    }
    let old = SelectObject(dc, bitmap as _);
    let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, 32 * 32 * 4);
    pixels.fill(0);
    let ok = DrawIconEx(dc, 0, 0, icon, 32, 32, 0, std::ptr::null_mut(), DI_NORMAL) != 0;
    let black = pixels.to_vec();
    pixels.fill(255);
    let ok = ok && DrawIconEx(dc, 0, 0, icon, 32, 32, 0, std::ptr::null_mut(), DI_NORMAL) != 0;
    let mut rgba = vec![0u8; 32 * 32 * 4];
    // Black/white compositing also handles legacy mask icons without an alpha channel.
    for ((b, w), out) in black
        .chunks_exact(4)
        .zip(pixels.chunks_exact(4))
        .zip(rgba.chunks_exact_mut(4))
    {
        let alpha = 255 - w[0].saturating_sub(b[0]);
        out[3] = alpha;
        if alpha > 0 {
            for i in 0..3 {
                out[i] = ((b[2 - i] as u32 * 255 / alpha as u32).min(255)) as u8;
            }
        }
    }
    SelectObject(dc, old);
    DeleteObject(bitmap as _);
    DeleteDC(dc);
    DestroyIcon(icon);
    ok.then(|| STANDARD.encode(rgba))
}

#[cfg(test)]
mod tests {
    #[test]
    fn icons_are_bounded_and_missing_resources_are_optional() {
        assert!(super::icon(r"C:\Kokoda-missing-fixture\missing.exe").is_none());
        let path = std::env::var("WINDIR").unwrap() + r"\System32\notepad.exe";
        if let Some(icon) = super::icon(&path) {
            use base64::Engine;
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(icon)
                    .unwrap()
                    .len(),
                4096
            );
        }
    }
}
