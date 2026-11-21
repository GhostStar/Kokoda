#[path = "password_dialog.rs"]
mod password_dialog;
use crate::settings::{self, Settings};
use kokoda_host::sharing::Rule;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
    mem::{size_of, zeroed},
    os::windows::{io::AsRawHandle, process::CommandExt},
    path::PathBuf,
    process::{Child, Command, Stdio},
    ptr::{null, null_mut},
    sync::mpsc::{self, Receiver},
    time::Instant,
};
use winapi::shared::{ipmib::MIB_IPADDRTABLE, winerror::ERROR_ALREADY_EXISTS};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{
        combaseapi::{CoInitializeEx, CoUninitialize},
        commctrl::{InitCommonControlsEx, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX},
        commdlg::*,
        errhandlingapi::GetLastError,
        handleapi::CloseHandle,
        iphlpapi::GetIpAddrTable,
        jobapi2::*,
        libloaderapi::GetModuleHandleW,
        objbase::COINIT_APARTMENTTHREADED,
        synchapi::CreateMutexW,
        winbase::{MulDiv, CREATE_NO_WINDOW},
        wingdi::*,
        winnls::GetUserDefaultUILanguage,
        winnt::*,
        winuser::*,
    },
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const START: i32 = 101;
const STOP: i32 = 102;
const PASSWORD: i32 = 103;
const MODE: i32 = 104;
const PORT: i32 = 105;
const EXE: i32 = 106;
const TITLE: i32 = 107;
const BROWSE: i32 = 108;
const ADD: i32 = 109;
const REMOVE: i32 = 110;
const RULES: i32 = 111;
const PREVIEW: i32 = 112;
const PREVIEW_LIST: i32 = 113;
const STATUS: i32 = 114;
const ADDRESS: i32 = 115;
const DETAIL: i32 = 116;
const HELP: i32 = 117;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn t(key: &str) -> String {
    static STRINGS: std::sync::OnceLock<BTreeMap<String, String>> = std::sync::OnceLock::new();
    STRINGS
        .get_or_init(|| {
            let zh = unsafe { GetUserDefaultUILanguage() & 0x3ff == 4 };
            serde_json::from_str(if zh {
                include_str!("../../../desktop/locales/zh-CN.json")
            } else {
                include_str!("../../../desktop/locales/en.json")
            })
            .expect("compiled locale")
        })
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.into())
}
unsafe fn text(hwnd: HWND) -> String {
    let mut value = vec![0u16; GetWindowTextLengthW(hwnd).max(0) as usize + 1];
    let len = GetWindowTextW(hwnd, value.as_mut_ptr(), value.len() as i32);
    String::from_utf16_lossy(&value[..len.max(0) as usize])
}
unsafe fn set(hwnd: HWND, text: &str) {
    SetWindowTextW(hwnd, wide(text).as_ptr());
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Service {
    child: Child,
    _job: Handle,
    events: Receiver<(bool, String)>,
    started: Instant,
    ready: bool,
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Service {
    fn spawn(executable: PathBuf, args: Vec<String>) -> Result<Self> {
        unsafe {
            let job = Handle(CreateJobObjectW(null_mut(), null()));
            if job.0.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &mut limits as *mut _ as _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as _,
            ) == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            let mut child = Command::new(executable)
                .args(args)
                .creation_flags(CREATE_NO_WINDOW)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            if AssignProcessToJobObject(job.0, child.as_raw_handle() as _) == 0 {
                let error = std::io::Error::last_os_error();
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
            let (tx, events) = mpsc::sync_channel(64);
            let stdout = child.stdout.take().ok_or("Missing host output")?;
            let stderr = child.stderr.take().ok_or("Missing host diagnostics")?;
            let out = tx.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout)
                    .lines()
                    .map_while(std::result::Result::ok)
                {
                    let ready = line.starts_with("Kokoda listening on ");
                    if out.send((ready, line)).is_err() {
                        break;
                    }
                }
            });
            std::thread::spawn(move || {
                for line in BufReader::new(stderr)
                    .lines()
                    .map_while(std::result::Result::ok)
                {
                    let _ = tx.try_send((false, line.chars().take(2048).collect()));
                }
            });
            Ok(Self {
                child,
                _job: job,
                events,
                started: Instant::now(),
                ready: false,
            })
        }
    }
}
struct Control {
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}
struct App {
    hwnd: HWND,
    directory: PathBuf,
    settings: Settings,
    service: Option<Service>,
    controls: Vec<Control>,
    font: HFONT,
    dpi: u32,
    diagnostics: Vec<String>,
}
impl Drop for App {
    fn drop(&mut self) {
        self.service.take();
        unsafe {
            DeleteObject(self.font as _);
        }
    }
}
impl App {
    unsafe fn id(&self, id: i32) -> HWND {
        GetDlgItem(self.hwnd, id)
    }
    unsafe fn add(&mut self, class: &str, label: &str, style: u32, id: i32, rect: [i32; 4]) {
        let hwnd = CreateWindowExW(
            if class == "EDIT" || class == "LISTBOX" {
                WS_EX_CLIENTEDGE
            } else {
                0
            },
            wide(class).as_ptr(),
            wide(label).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            0,
            0,
            self.hwnd,
            id as _,
            GetModuleHandleW(null()),
            null_mut(),
        );
        if hwnd.is_null() {
            panic!("Could not create native host control");
        }
        self.controls.push(Control {
            hwnd,
            x: rect[0],
            y: rect[1],
            width: rect[2],
            height: rect[3],
        });
    }
    unsafe fn build(&mut self) {
        self.add("STATIC", &t("host.heading"), 0, -1, [24, 18, 690, 26]);
        self.add("STATIC", &t("host.stopped"), 0, STATUS, [24, 46, 690, 24]);
        self.add(
            "BUTTON",
            &t("host.start"),
            WS_TABSTOP | BS_DEFPUSHBUTTON,
            START,
            [24, 76, 150, 34],
        );
        self.add(
            "BUTTON",
            &t("host.stop"),
            WS_TABSTOP,
            STOP,
            [184, 76, 140, 34],
        );
        self.add(
            "BUTTON",
            &t("host.password_title"),
            WS_TABSTOP,
            PASSWORD,
            [336, 76, 220, 34],
        );
        self.add("STATIC", &t("host.port"), 0, -1, [574, 81, 50, 24]);
        self.add(
            "EDIT",
            &self.settings.port.unwrap_or(47831).to_string(),
            WS_TABSTOP | ES_NUMBER | ES_AUTOHSCROLL,
            PORT,
            [628, 77, 76, 30],
        );
        SendMessageW(self.id(PORT), EM_SETLIMITTEXT as _, 5, 0);
        self.add(
            "EDIT",
            "",
            WS_TABSTOP | ES_READONLY | ES_AUTOHSCROLL,
            ADDRESS,
            [24, 116, 686, 30],
        );
        self.add("STATIC", &t("host.scope"), 0, -1, [24, 160, 110, 24]);
        self.add(
            "COMBOBOX",
            "",
            WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL,
            MODE,
            [136, 154, 574, 120],
        );
        for value in [t("host.selected"), t("host.all")] {
            SendMessageW(self.id(MODE), CB_ADDSTRING, 0, wide(&value).as_ptr() as _);
        }
        SendMessageW(
            self.id(MODE),
            CB_SETCURSEL,
            usize::from(self.settings.all_windows),
            0,
        );
        self.add("STATIC", "", 0, HELP, [24, 194, 686, 36]);
        self.add("STATIC", &t("host.executable"), 0, -1, [24, 242, 110, 24]);
        self.add(
            "EDIT",
            "",
            WS_TABSTOP | ES_AUTOHSCROLL,
            EXE,
            [136, 236, 458, 30],
        );
        self.add(
            "BUTTON",
            &t("host.browse"),
            WS_TABSTOP,
            BROWSE,
            [604, 236, 106, 30],
        );
        self.add("STATIC", &t("host.title"), 0, -1, [24, 280, 110, 24]);
        self.add(
            "EDIT",
            "",
            WS_TABSTOP | ES_AUTOHSCROLL,
            TITLE,
            [136, 274, 458, 30],
        );
        self.add(
            "BUTTON",
            &t("host.add"),
            WS_TABSTOP,
            ADD,
            [604, 274, 106, 30],
        );
        SendMessageW(self.id(EXE), EM_SETLIMITTEXT as _, 4096, 0);
        SendMessageW(self.id(TITLE), EM_SETLIMITTEXT as _, 4096, 0);
        self.add(
            "LISTBOX",
            "",
            WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT,
            RULES,
            [24, 316, 570, 76],
        );
        self.add(
            "BUTTON",
            &t("host.remove"),
            WS_TABSTOP,
            REMOVE,
            [604, 316, 106, 32],
        );
        self.add(
            "BUTTON",
            &t("host.preview"),
            WS_TABSTOP,
            PREVIEW,
            [604, 360, 106, 32],
        );
        self.add(
            "STATIC",
            &t("host.preview_label"),
            0,
            -1,
            [24, 402, 686, 24],
        );
        self.add(
            "LISTBOX",
            "",
            WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | LBS_NOINTEGRALHEIGHT,
            PREVIEW_LIST,
            [24, 424, 686, 72],
        );
        self.add("STATIC", &t("host.instructions"), 0, -1, [24, 506, 686, 40]);
        self.add(
            "EDIT",
            "",
            WS_TABSTOP | ES_READONLY | ES_MULTILINE | ES_AUTOVSCROLL | WS_VSCROLL,
            DETAIL,
            [24, 554, 686, 50],
        );
        self.layout(self.dpi);
        self.rules();
        self.enabled();
        self.addresses();
        SetTimer(self.hwnd, 1, 250, None);
        SetFocus(self.id(START));
    }
    unsafe fn layout(&mut self, dpi: u32) {
        self.dpi = dpi;
        let px = |n: i32| MulDiv(n, dpi as _, 96);
        let mut font: LOGFONTW = zeroed();
        font.lfHeight = -px(14);
        let face = wide("Segoe UI");
        font.lfFaceName[..face.len()].copy_from_slice(&face);
        let new_font = CreateFontIndirectW(&font);
        for c in &self.controls {
            MoveWindow(c.hwnd, px(c.x), px(c.y), px(c.width), px(c.height), 1);
            SendMessageW(c.hwnd, WM_SETFONT, new_font as _, 1);
        }
        if !self.font.is_null() {
            DeleteObject(self.font as _);
        }
        self.font = new_font;
    }
    unsafe fn enabled(&self) {
        let idle = self.service.is_none();
        let selected = SendMessageW(self.id(MODE), CB_GETCURSEL, 0, 0) == 0;
        for id in [START, MODE, PORT, PASSWORD] {
            EnableWindow(self.id(id), idle as _);
        }
        EnableWindow(self.id(STOP), (!idle) as _);
        for id in [EXE, TITLE, BROWSE, ADD, REMOVE, RULES] {
            EnableWindow(self.id(id), (idle && selected) as _);
        }
        EnableWindow(self.id(PREVIEW), idle as _);
        set(
            self.id(HELP),
            &if selected {
                t("host.selected_help")
            } else {
                t("host.all_help")
            },
        );
    }
    unsafe fn rules(&self) {
        SendMessageW(self.id(RULES), LB_RESETCONTENT, 0, 0);
        for rule in &self.settings.policy.rules {
            let label = format!(
                "{}  |  {}",
                rule.executable.as_deref().unwrap_or("*"),
                rule.title_contains.as_deref().unwrap_or("*")
            );
            SendMessageW(self.id(RULES), LB_ADDSTRING, 0, wide(&label).as_ptr() as _);
        }
        SendMessageW(self.id(RULES), LB_SETHORIZONTALEXTENT, 1400, 0);
    }
    unsafe fn sync(&mut self) -> Result<()> {
        self.settings.port = Some(
            text(self.id(PORT))
                .parse::<u16>()
                .ok()
                .filter(|p| *p != 0)
                .ok_or_else(|| t("host.invalid_port"))?,
        );
        self.settings.all_windows = SendMessageW(self.id(MODE), CB_GETCURSEL, 0, 0) == 1;
        settings::save(&self.directory, &self.settings)?;
        Ok(())
    }
    unsafe fn addresses(&self) {
        let port = text(self.id(PORT));
        let mut size = 0;
        GetIpAddrTable(null_mut(), &mut size, 0);
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let table = buffer.as_mut_ptr() as *mut MIB_IPADDRTABLE;
        let mut addresses = Vec::new();
        if size > 0 && GetIpAddrTable(table, &mut size, 0) == 0 {
            for row in
                std::slice::from_raw_parts((*table).table.as_ptr(), (*table).dwNumEntries as usize)
            {
                let octets = row.dwAddr.to_ne_bytes();
                if octets[0] != 0 && octets[0] != 127 {
                    addresses.push(format!("{}:{port}", std::net::Ipv4Addr::from(octets)));
                }
            }
        }
        set(
            self.id(ADDRESS),
            &if addresses.is_empty() {
                t("host.no_network")
            } else {
                addresses.join("   ")
            },
        );
    }
    unsafe fn report(&mut self, message: String) {
        self.diagnostics.push(message.chars().take(2048).collect());
        if self.diagnostics.len() > 8 {
            self.diagnostics.remove(0);
        }
        set(self.id(DETAIL), &self.diagnostics.join("\r\n"));
    }
    unsafe fn error(&mut self, error: impl std::fmt::Display) {
        self.report(format!("{}: {error}", t("host.error")));
    }
    unsafe fn preview(&mut self) -> Result<()> {
        self.sync()?;
        let sharing = if self.settings.all_windows {
            kokoda_host::windows::sharing::Sharing::all_development()
        } else {
            kokoda_host::windows::sharing::Sharing::load(&self.directory.join("sharing.json"))?
        };
        let windows = sharing.catalog()?;
        SendMessageW(self.id(PREVIEW_LIST), LB_RESETCONTENT, 0, 0);
        if windows.is_empty() {
            SendMessageW(
                self.id(PREVIEW_LIST),
                LB_ADDSTRING,
                0,
                wide(&t("host.no_windows")).as_ptr() as _,
            );
        }
        for window in windows {
            SendMessageW(
                self.id(PREVIEW_LIST),
                LB_ADDSTRING,
                0,
                wide(&window.title).as_ptr() as _,
            );
        }
        Ok(())
    }
    unsafe fn start(&mut self) -> Result<()> {
        if self.service.is_some() {
            return Ok(());
        }
        self.sync()?;
        if !self.settings.all_windows {
            kokoda_host::windows::sharing::Sharing::load(&self.directory.join("sharing.json"))?;
        }
        let record =
            kokoda_platform_windows::credentials::load(&self.directory.join("password.dat"))
                .map_err(|_| t("host.password_required"))?;
        kokoda_transport::password::Credentials::decode(&record)?;
        let exe = std::env::current_exe()?.with_file_name("kokoda-windows-host.exe");
        self.diagnostics.clear();
        set(self.id(DETAIL), "");
        self.service = Some(Service::spawn(
            exe,
            self.settings.arguments(&self.directory),
        )?);
        set(self.id(STATUS), &t("host.starting"));
        self.enabled();
        self.addresses();
        Ok(())
    }
    unsafe fn stop(&mut self) {
        self.service.take();
        set(self.id(STATUS), &t("host.stopped"));
        self.enabled();
    }
    unsafe fn poll(&mut self) {
        let status_control = self.id(STATUS);
        let mut messages = Vec::new();
        let mut ended = None;
        if let Some(service) = &mut self.service {
            while let Ok((ready, message)) = service.events.try_recv() {
                if ready {
                    service.ready = true;
                    set(status_control, &t("host.running"));
                } else {
                    messages.push(message);
                }
            }
            match service.child.try_wait() {
                Ok(Some(status)) => ended = Some(format!("{} ({status})", t("host.exited"))),
                Err(error) => ended = Some(error.to_string()),
                _ => {}
            }
            if !service.ready && service.started.elapsed().as_secs() >= 15 {
                ended = Some(t("host.start_timeout"));
            }
        }
        for message in messages {
            self.report(message);
        }
        if let Some(reason) = ended {
            self.stop();
            set(self.id(STATUS), &t("host.failed"));
            self.error(reason);
        }
    }
    unsafe fn command(&mut self, id: i32) -> Result<()> {
        match id {
            START => self.start()?,
            STOP => self.stop(),
            MODE => self.enabled(),
            ADD => {
                let exe = text(self.id(EXE));
                let title = text(self.id(TITLE));
                let rule = Rule {
                    executable: if exe.is_empty() {
                        None
                    } else {
                        let path = PathBuf::from(&exe);
                        if !path.is_absolute() || !path.is_file() {
                            return Err(t("host.invalid_executable").into());
                        }
                        Some(exe)
                    },
                    title_contains: if title.is_empty() { None } else { Some(title) },
                };
                let mut candidate = self.settings.policy.clone();
                candidate.rules.push(rule);
                candidate.validate().map_err(|_| t("host.invalid_rule"))?;
                self.settings.policy = candidate;
                self.rules();
                set(self.id(EXE), "");
                set(self.id(TITLE), "");
            }
            REMOVE => {
                let index = SendMessageW(self.id(RULES), LB_GETCURSEL, 0, 0);
                if index >= 0 {
                    self.settings.policy.rules.remove(index as usize);
                    self.rules();
                }
            }
            BROWSE => {
                let mut file = vec![0u16; 32768];
                let mut dialog: OPENFILENAMEW = zeroed();
                dialog.lStructSize = size_of::<OPENFILENAMEW>() as _;
                dialog.hwndOwner = self.hwnd;
                let filter = wide(&format!("{}\0*.exe\0\0", t("host.application_filter")));
                dialog.lpstrFilter = filter.as_ptr();
                dialog.lpstrFile = file.as_mut_ptr();
                dialog.nMaxFile = file.len() as _;
                dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
                if GetOpenFileNameW(&mut dialog) != 0 {
                    let len = file.iter().position(|c| *c == 0).unwrap_or(0);
                    set(self.id(EXE), &String::from_utf16_lossy(&file[..len]));
                }
            }
            PREVIEW => self.preview()?,
            PASSWORD => {
                if self.service.is_some() {
                    return Ok(());
                }
                KillTimer(self.hwnd, 1);
                let password = password_dialog::show(self.hwnd);
                SetTimer(self.hwnd, 1, 250, None);
                if let Some(password) = password? {
                    let record = kokoda_transport::password::enroll(&password)?;
                    kokoda_platform_windows::credentials::save(
                        &self.directory.join("password.dat"),
                        &record,
                    )?;
                    self.report(t("host.password_saved"));
                }
            }
            _ => {}
        }
        Ok(())
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let app = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if !app.is_null() {
        match msg {
            WM_COMMAND => {
                let id = LOWORD(wp as _) as i32;
                let event = HIWORD(wp as _);
                if (id == MODE && event == CBN_SELCHANGE)
                    || (event == BN_CLICKED
                        && matches!(
                            id,
                            START | STOP | PASSWORD | BROWSE | ADD | REMOVE | PREVIEW
                        ))
                {
                    if let Err(error) = (*app).command(id) {
                        (*app).error(error);
                    }
                }
                return 0;
            }
            WM_TIMER => {
                (*app).poll();
                return 0;
            }
            WM_DPICHANGED => {
                let rect = &*(lp as *const RECT);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                (*app).layout(LOWORD(wp as _) as _);
                return 0;
            }
            WM_CLOSE => {
                (*app).stop();
                if let Err(error) = (*app).sync() {
                    (*app).error(error);
                }
                DestroyWindow(hwnd);
                return 0;
            }
            WM_DESTROY => {
                KillTimer(hwnd, 1);
                PostQuitMessage(0);
                return 0;
            }
            _ => {}
        }
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}
pub fn run() {
    if let Err(error) = unsafe { launch() } {
        unsafe {
            MessageBoxW(
                null_mut(),
                wide(&format!("{}\n{error}", t("host.error"))).as_ptr(),
                wide("Kokoda").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
unsafe fn launch() -> Result<()> {
    // Shell dialogs and accessible native controls belong to an STA. Capture runs
    // in the child host, which owns its own MTA/Media Foundation lifetime.
    let result = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED);
    if result < 0 {
        return Err(format!("OLE initialization failed: 0x{result:08x}").into());
    }
    struct Ole;
    impl Drop for Ole {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _ole = Ole;
    let controls = INITCOMMONCONTROLSEX {
        dwSize: size_of::<INITCOMMONCONTROLSEX>() as _,
        dwICC: ICC_STANDARD_CLASSES,
    };
    if InitCommonControlsEx(&controls) == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    kokoda_platform_windows::windows::initialize_dpi();
    let directory = if let Some(path) = std::env::var_os("KOKODA_HOST_DATA_DIR") {
        PathBuf::from(path)
    } else {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA unavailable")?)
            .join("Kokoda")
    };
    // One settings owner per user data directory; test directories remain isolated.
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    directory.hash(&mut hash);
    let mutex = Handle(CreateMutexW(
        null_mut(),
        0,
        wide(&format!("Local\\KokodaHost-{:x}", hash.finish())).as_ptr(),
    ));
    if mutex.0.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    if GetLastError() == ERROR_ALREADY_EXISTS {
        return Err(t("host.already_open").into());
    }
    let settings = settings::load(&directory)?;
    let instance = GetModuleHandleW(null());
    let class = wide("KokodaHostSettings");
    let mut wc: WNDCLASSW = zeroed();
    wc.lpfnWndProc = Some(procedure);
    wc.hInstance = instance;
    wc.lpszClassName = class.as_ptr();
    wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    wc.hIcon = LoadIconW(instance, MAKEINTRESOURCEW(1));
    if wc.hIcon.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    wc.hbrBackground = (COLOR_BTNFACE + 1) as _;
    if RegisterClassW(&wc) == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let dpi = GetDpiForSystem();
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: MulDiv(734, dpi as _, 96),
        bottom: MulDiv(620, dpi as _, 96),
    };
    AdjustWindowRectExForDpi(&mut rect, style, 0, 0, dpi);
    let hwnd = CreateWindowExW(
        WS_EX_CONTROLPARENT,
        class.as_ptr(),
        wide(&t("host.window_title")).as_ptr(),
        style,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        rect.right - rect.left,
        rect.bottom - rect.top,
        null_mut(),
        null_mut(),
        instance,
        null_mut(),
    );
    if hwnd.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut app = Box::new(App {
        hwnd,
        directory,
        settings,
        service: None,
        controls: Vec::new(),
        font: null_mut(),
        dpi,
        diagnostics: Vec::new(),
    });
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *app as *mut App as _);
    app.build();
    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);
    let mut msg: MSG = zeroed();
    loop {
        let status = GetMessageW(&mut msg, null_mut(), 0, 0);
        if status <= 0 {
            break;
        }
        if IsDialogMessageW(hwnd, &mut msg) == 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
