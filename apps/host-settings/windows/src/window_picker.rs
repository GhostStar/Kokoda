use super::{set, t, text, wide, Result};
use crate::window_catalog::{self, Sample};
use kokoda_host::sharing::Rule;
use std::{
    mem::zeroed,
    ptr::{null, null_mut},
};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{libloaderapi::GetModuleHandleW, winbase::MulDiv, winuser::*},
};
const SEARCH: i32 = 301;
const LIST: i32 = 302;
const MODE: i32 = 303;
const PREVIEW: i32 = 304;
const USE: i32 = 305;
const REFRESH: i32 = 306;
const DETAIL: i32 = 307;
const STATUS: i32 = 308;
struct Dialog {
    samples: Vec<Sample>,
    shown: Vec<Sample>,
    result: Option<Rule>,
}
impl Dialog {
    unsafe fn selected(&self, hwnd: HWND) -> Option<&Sample> {
        let i = SendMessageW(GetDlgItem(hwnd, LIST), LB_GETCURSEL, 0, 0);
        if i < 0 {
            None
        } else {
            self.shown.get(i as usize)
        }
    }
    unsafe fn refresh(&mut self, hwnd: HWND, reload: bool) {
        let selected = self.selected(hwnd).cloned();
        if reload {
            match window_catalog::enumerate() {
                Ok(rows) => self.samples = rows,
                Err(_) => {
                    self.samples.clear();
                    set(GetDlgItem(hwnd, STATUS), &t("host.picker_failed"));
                }
            }
        }
        let query = text(GetDlgItem(hwnd, SEARCH)).to_lowercase();
        let shown: Vec<_> = self
            .samples
            .iter()
            .filter(|s| {
                query.is_empty()
                    || s.title.to_lowercase().contains(&query)
                    || s.executable.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        if shown != self.shown {
            SendMessageW(GetDlgItem(hwnd, LIST), LB_RESETCONTENT, 0, 0);
            self.shown = shown;
            for (i, sample) in self.shown.iter().enumerate() {
                let title = if sample.title.is_empty() {
                    t("windows.untitled")
                } else {
                    sample.title.clone()
                };
                let app = std::path::Path::new(&sample.executable)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let label = format!("{title}  —  {app}  [PID {}]", sample.pid);
                SendMessageW(
                    GetDlgItem(hwnd, LIST),
                    LB_ADDSTRING,
                    0,
                    wide(&label).as_ptr() as _,
                );
                if selected
                    .as_ref()
                    .is_some_and(|old| old.same_instance(sample))
                {
                    SendMessageW(GetDlgItem(hwnd, LIST), LB_SETCURSEL, i, 0);
                }
            }
        }
        self.preview(hwnd);
    }
    unsafe fn preview(&self, hwnd: HWND) {
        SendMessageW(GetDlgItem(hwnd, PREVIEW), LB_RESETCONTENT, 0, 0);
        EnableWindow(GetDlgItem(hwnd, USE), 0);
        set(GetDlgItem(hwnd, DETAIL), "");
        let Some(sample) = self.selected(hwnd) else {
            set(GetDlgItem(hwnd, STATUS), &t("host.picker_select"));
            return;
        };
        set(GetDlgItem(hwnd, DETAIL), &sample.executable);
        let mode = SendMessageW(GetDlgItem(hwnd, MODE), CB_GETCURSEL, 0, 0) as usize;
        let Ok(rule) = sample.rule(mode) else {
            set(GetDlgItem(hwnd, STATUS), &t("host.picker_empty_title"));
            return;
        };
        let matches = window_catalog::preview(rule, &self.samples);
        for (id, direct) in &matches {
            if let Some(s) = self.samples.iter().find(|s| s.id == *id) {
                let label = format!(
                    "{} · {}",
                    if *direct {
                        t("host.picker_direct")
                    } else {
                        t("host.picker_owned")
                    },
                    if s.title.is_empty() {
                        t("windows.untitled")
                    } else {
                        s.title.clone()
                    }
                );
                SendMessageW(
                    GetDlgItem(hwnd, PREVIEW),
                    LB_ADDSTRING,
                    0,
                    wide(&label).as_ptr() as _,
                );
            }
        }
        // Preview is fresh local evidence, never a permanent authorization grant.
        set(GetDlgItem(hwnd, STATUS), &t("host.picker_preview_current"));
        EnableWindow(GetDlgItem(hwnd, USE), 1);
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Dialog;
    if !state.is_null() {
        match msg {
            WM_TIMER => {
                (*state).refresh(hwnd, true);
                return 0;
            }
            WM_COMMAND => {
                let id = LOWORD(wp as _) as i32;
                let event = HIWORD(wp as _);
                if id == SEARCH && event == EN_CHANGE as u16 {
                    (*state).refresh(hwnd, false);
                    return 0;
                }
                if (id == LIST && event == LBN_SELCHANGE as u16)
                    || (id == MODE && event == CBN_SELCHANGE)
                {
                    (*state).preview(hwnd);
                    return 0;
                }
                if event == BN_CLICKED {
                    match id {
                        REFRESH => {
                            (*state).refresh(hwnd, true);
                            return 0;
                        }
                        USE | IDOK => {
                            if let Some(sample) = (*state).selected(hwnd) {
                                if !window_catalog::revalidate(sample) {
                                    (*state).refresh(hwnd, true);
                                    set(GetDlgItem(hwnd, STATUS), &t("host.picker_stale"));
                                    return 0;
                                }
                                let mode = SendMessageW(GetDlgItem(hwnd, MODE), CB_GETCURSEL, 0, 0)
                                    as usize;
                                if let Ok(rule) = sample.rule(mode) {
                                    (*state).result = Some(rule);
                                } else {
                                    return 0;
                                }
                            } else {
                                return 0;
                            }
                        }
                        IDCANCEL => {}
                        _ => return DefWindowProcW(hwnd, msg, wp, lp),
                    }
                    DestroyWindow(hwnd);
                    return 0;
                }
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                return 0;
            }
            WM_DESTROY => {
                KillTimer(hwnd, 1);
                return 0;
            }
            _ => {}
        }
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}
pub unsafe fn show(owner: HWND) -> Result<Option<Rule>> {
    let class = wide("KokodaWindowPicker");
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
        right: px(700),
        bottom: px(560),
    };
    AdjustWindowRectExForDpi(&mut rect, style, 0, WS_EX_DLGMODALFRAME, dpi);
    let mut parent: RECT = zeroed();
    GetWindowRect(owner, &mut parent);
    let hwnd = CreateWindowExW(
        WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
        class.as_ptr(),
        wide(&t("host.picker_title")).as_ptr(),
        style,
        parent.left + px(12),
        parent.top + px(12),
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
    let mut state = Dialog {
        samples: vec![],
        shown: vec![],
        result: None,
    };
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut state as *mut _ as _);
    let font = SendMessageW(GetDlgItem(owner, 101), WM_GETFONT, 0, 0);
    for (class, label, style, id, [x, y, w, h]) in [
        (
            "STATIC",
            t("host.picker_explanation"),
            0,
            309,
            [18, 12, 664, 44],
        ),
        ("STATIC", t("host.picker_search"), 0, 310, [18, 66, 94, 24]),
        (
            "EDIT",
            String::new(),
            WS_TABSTOP | ES_AUTOHSCROLL,
            SEARCH,
            [116, 60, 432, 30],
        ),
        (
            "BUTTON",
            t("windows.refresh"),
            WS_TABSTOP,
            REFRESH,
            [558, 60, 124, 30],
        ),
        (
            "LISTBOX",
            String::new(),
            WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT,
            LIST,
            [18, 100, 664, 140],
        ),
        (
            "EDIT",
            String::new(),
            WS_TABSTOP | ES_READONLY | ES_AUTOHSCROLL,
            DETAIL,
            [18, 248, 664, 28],
        ),
        ("STATIC", t("host.picker_match"), 0, 311, [18, 288, 94, 24]),
        (
            "COMBOBOX",
            String::new(),
            WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL,
            MODE,
            [116, 282, 566, 120],
        ),
        (
            "STATIC",
            t("host.picker_future"),
            0,
            312,
            [18, 320, 664, 42],
        ),
        (
            "LISTBOX",
            String::new(),
            WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | LBS_NOINTEGRALHEIGHT,
            PREVIEW,
            [18, 368, 664, 100],
        ),
        ("STATIC", String::new(), 0, STATUS, [18, 476, 664, 30]),
        (
            "BUTTON",
            t("host.picker_use"),
            WS_TABSTOP | BS_DEFPUSHBUTTON,
            USE,
            [380, 516, 176, 30],
        ),
        (
            "BUTTON",
            t("dialog.cancel"),
            WS_TABSTOP,
            IDCANCEL,
            [568, 516, 114, 30],
        ),
    ] {
        let control = CreateWindowExW(
            if class == "EDIT" || class == "LISTBOX" {
                WS_EX_CLIENTEDGE
            } else {
                0
            },
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
        if class == "LISTBOX" {
            SendMessageW(control, LB_SETHORIZONTALEXTENT, px(1600) as _, 0);
        }
    }
    SendMessageW(GetDlgItem(hwnd, SEARCH), EM_SETLIMITTEXT as _, 512, 0);
    for label in [
        t("host.picker_both"),
        t("host.picker_process"),
        t("host.picker_title_only"),
    ] {
        SendMessageW(
            GetDlgItem(hwnd, MODE),
            CB_ADDSTRING,
            0,
            wide(&label).as_ptr() as _,
        );
    }
    SendMessageW(GetDlgItem(hwnd, MODE), CB_SETCURSEL, 0, 0);
    state.refresh(hwnd, true);
    SetTimer(hwnd, 1, 3000, None);
    EnableWindow(owner, 0);
    ShowWindow(hwnd, SW_SHOW);
    SetFocus(GetDlgItem(hwnd, SEARCH));
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
