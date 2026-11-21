use super::{t, wide};
use std::{
    mem::zeroed,
    ptr::{null, null_mut},
};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{libloaderapi::GetModuleHandleW, winbase::MulDiv, winuser::*},
};
use zeroize::Zeroizing;
struct Dialog {
    result: Option<Zeroizing<String>>,
}
unsafe fn secret(hwnd: HWND) -> Zeroizing<String> {
    let mut bytes = Zeroizing::new(vec![0u16; GetWindowTextLengthW(hwnd).max(0) as usize + 1]);
    let len = GetWindowTextW(hwnd, bytes.as_mut_ptr(), bytes.len() as _);
    Zeroizing::new(String::from_utf16_lossy(&bytes[..len.max(0) as usize]))
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Dialog;
    if !state.is_null() {
        match msg {
            WM_COMMAND if HIWORD(wp as _) == BN_CLICKED => {
                match LOWORD(wp as _) as i32 {
                    203 | IDOK => {
                        let first = secret(GetDlgItem(hwnd, 201));
                        let second = secret(GetDlgItem(hwnd, 202));
                        let error = if !kokoda_transport::password::validate_password(&first) {
                            Some(t("host.password_requirements"))
                        } else if *first != *second {
                            Some(t("host.password_mismatch"))
                        } else {
                            None
                        };
                        if let Some(error) = error {
                            super::set(GetDlgItem(hwnd, 205), &error);
                            return 0;
                        }
                        (*state).result = Some(first);
                    }
                    204 | IDCANCEL => {}
                    _ => return DefWindowProcW(hwnd, msg, wp, lp),
                }
                super::set(GetDlgItem(hwnd, 201), "");
                super::set(GetDlgItem(hwnd, 202), "");
                DestroyWindow(hwnd);
                return 0;
            }
            WM_CLOSE => {
                super::set(GetDlgItem(hwnd, 201), "");
                super::set(GetDlgItem(hwnd, 202), "");
                DestroyWindow(hwnd);
                return 0;
            }
            _ => {}
        }
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}
pub unsafe fn show(owner: HWND) -> super::Result<Option<Zeroizing<String>>> {
    let class = wide("KokodaPasswordDialog");
    let instance = GetModuleHandleW(null());
    let mut wc: WNDCLASSW = zeroed();
    wc.hInstance = instance;
    wc.lpszClassName = class.as_ptr();
    wc.lpfnWndProc = Some(procedure);
    wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    wc.hbrBackground = (COLOR_BTNFACE + 1) as _;
    RegisterClassW(&wc);
    let dpi = GetDpiForWindow(owner);
    let px = |n| MulDiv(n, dpi as _, 96);
    let style = WS_CAPTION | WS_SYSMENU;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: px(470),
        bottom: px(244),
    };
    AdjustWindowRectExForDpi(&mut rect, style, 0, WS_EX_DLGMODALFRAME, dpi);
    let mut parent: RECT = zeroed();
    GetWindowRect(owner, &mut parent);
    let hwnd = CreateWindowExW(
        WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
        class.as_ptr(),
        wide(&t("host.password_title")).as_ptr(),
        style,
        parent.left + px(70),
        parent.top + px(70),
        rect.right - rect.left,
        rect.bottom - rect.top,
        owner,
        null_mut(),
        instance,
        null_mut(),
    );
    if hwnd.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut state = Dialog { result: None };
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut state as *mut _ as _);
    let font = SendMessageW(GetDlgItem(owner, 101), WM_GETFONT, 0, 0);
    for (class, label, style, id, [x, y, w, h]) in [
        (
            "STATIC",
            t("host.password_requirements"),
            0,
            200,
            [20, 16, 430, 40],
        ),
        ("STATIC", t("host.password_new"), 0, 206, [20, 66, 100, 24]),
        (
            "EDIT",
            String::new(),
            WS_TABSTOP | ES_PASSWORD | ES_AUTOHSCROLL,
            201,
            [130, 60, 320, 30],
        ),
        (
            "STATIC",
            t("host.password_confirm"),
            0,
            207,
            [20, 104, 100, 24],
        ),
        (
            "EDIT",
            String::new(),
            WS_TABSTOP | ES_PASSWORD | ES_AUTOHSCROLL,
            202,
            [130, 98, 320, 30],
        ),
        ("STATIC", String::new(), 0, 205, [20, 138, 430, 40]),
        (
            "BUTTON",
            t("host.password_save"),
            WS_TABSTOP | BS_DEFPUSHBUTTON,
            203,
            [210, 192, 116, 32],
        ),
        (
            "BUTTON",
            t("dialog.cancel"),
            WS_TABSTOP,
            204,
            [336, 192, 114, 32],
        ),
    ] {
        let control = CreateWindowExW(
            if class == "EDIT" { WS_EX_CLIENTEDGE } else { 0 },
            wide(class).as_ptr(),
            wide(&label).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            px(x),
            px(y),
            px(w),
            px(h),
            hwnd,
            id as _,
            instance,
            null_mut(),
        );
        SendMessageW(control, WM_SETFONT, font as _, 1);
        if class == "EDIT" {
            SendMessageW(control, EM_SETLIMITTEXT as _, 256, 0);
        }
    }
    EnableWindow(owner, 0);
    ShowWindow(hwnd, SW_SHOW);
    SetFocus(GetDlgItem(hwnd, 201));
    let mut msg: MSG = zeroed();
    while IsWindow(hwnd) != 0 {
        let code = GetMessageW(&mut msg, null_mut(), 0, 0);
        if code <= 0 {
            if code == 0 {
                PostQuitMessage(msg.wParam as _);
            }
            break;
        }
        if IsDialogMessageW(hwnd, &mut msg) == 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    if IsWindow(hwnd) != 0 {
        DestroyWindow(hwnd);
    }
    EnableWindow(owner, 1);
    SetForegroundWindow(owner);
    Ok(state.result)
}
