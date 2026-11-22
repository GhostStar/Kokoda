//! Interactive synthetic pointer regression. No network or private windows.
#[cfg(not(windows))]
fn main() {}
#[cfg(windows)]
fn main() {
    use kokoda_app_model::input::InputEvent;
    use kokoda_platform_windows::{input::Target, windows};
    use std::{
        mem, ptr,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    };
    use winapi::{
        shared::{
            minwindef::{LPARAM, LRESULT, UINT, WPARAM},
            windef::HWND,
        },
        um::{libloaderapi::GetModuleHandleW, winuser::*},
    };
    static RELOCATE: AtomicBool = AtomicBool::new(false);
    static CLICKS: AtomicUsize = AtomicUsize::new(0);
    static WHEELS: AtomicUsize = AtomicUsize::new(0);
    static TARGET: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "system" fn proc(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT {
        if matches!(
            msg,
            WM_LBUTTONDOWN | WM_LBUTTONUP | WM_NCLBUTTONDOWN | WM_MOUSEWHEEL
        ) {
            println!(
                "Synthetic mouse message: {msg}, target={}",
                hwnd as usize == TARGET.load(SeqCst)
            );
        }
        if hwnd as usize == TARGET.load(SeqCst) {
            if msg == WM_ACTIVATE
                && w & 0xffff != WA_INACTIVE as usize
                && RELOCATE.swap(false, SeqCst)
            {
                SetWindowPos(
                    hwnd,
                    ptr::null_mut(),
                    420,
                    80,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            if msg == WM_LBUTTONDOWN {
                CLICKS.fetch_add(1, SeqCst);
            }
            if msg == WM_MOUSEWHEEL {
                WHEELS.fetch_add(1, SeqCst);
            }
        }
        if msg == WM_APP {
            DestroyWindow(hwnd);
            return 0;
        }
        if msg == WM_DESTROY && hwnd as usize == TARGET.load(SeqCst) {
            PostQuitMessage(0);
            return 0;
        }
        DefWindowProcW(hwnd, msg, w, l)
    }
    fn wait(name: &str, f: impl Fn() -> bool) {
        let until = Instant::now() + Duration::from_secs(3);
        while !f() && Instant::now() < until {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(f(), "Synthetic {name} was not delivered");
    }
    windows::initialize_dpi();
    let (tx, rx) = mpsc::channel();
    let ui = thread::spawn(move || unsafe {
        let class: Vec<u16> = "KokodaInputProbe\0".encode_utf16().collect();
        let instance = GetModuleHandleW(ptr::null());
        let mut wc: WNDCLASSW = mem::zeroed();
        wc.lpfnWndProc = Some(proc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        assert_ne!(RegisterClassW(&wc), 0);
        let target = CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            40,
            80,
            320,
            240,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );
        assert!(!target.is_null());
        TARGET.store(target as usize, SeqCst);
        let cover = CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            40,
            80,
            320,
            240,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );
        assert!(!cover.is_null());
        tx.send((target as usize, cover as usize)).unwrap();
        let mut msg = mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
    let (id, cover) = rx.recv().unwrap();
    struct Cleanup(usize, usize);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            unsafe {
                PostMessageW(self.1 as HWND, WM_APP, 0, 0);
                PostMessageW(self.0 as HWND, WM_APP, 0, 0);
            }
        }
    }
    let cleanup = Cleanup(id, cover);
    let mut target = Target::new(id as u64).unwrap();
    let blocker = Target::new(cover as u64).unwrap();
    blocker.activate().unwrap();
    let mut w = windows::inspect(id as u64).unwrap();
    windows::capture_bounds(&mut w);
    let width = (w.bounds.right - w.bounds.left) as u32;
    let height = (w.bounds.bottom - w.bounds.top) as u32;
    RELOCATE.store(true, SeqCst);
    target
        .apply(InputEvent::Button {
            x: 0.5,
            y: 0.5,
            width,
            height,
            button: 0,
            down: true,
        })
        .unwrap();
    target.apply(InputEvent::Release).unwrap();
    wait("click", || CLICKS.load(SeqCst) == 1);
    println!("PASS: activation moves independent popup; click uses fresh physical bounds");
    // Change caller awareness deliberately; DWM bounds and pointer APIs stay physical.
    unsafe {
        SetThreadDpiAwarenessContext(-1isize as _);
        SetPhysicalCursorPos(5, 5);
    }
    target
        .apply(InputEvent::WheelAt {
            x: 0.5,
            y: 0.5,
            width,
            height,
            delta: -120,
        })
        .unwrap();
    assert!(
        unsafe { AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(), -1isize as _) } != 0,
        "Caller DPI context leaked"
    );
    wait("wheel", || WHEELS.load(SeqCst) == 1);
    println!("PASS: positioned wheel needs no previous hover, including DPI-unaware caller");
    unsafe {
        SetThreadDpiAwarenessContext(-4isize as _);
        SetWindowPos(
            cover as HWND,
            HWND_TOPMOST,
            420,
            80,
            320,
            240,
            SWP_NOACTIVATE,
        );
    }
    let error = target
        .apply(InputEvent::WheelAt {
            x: 0.5,
            y: 0.5,
            width,
            height,
            delta: 120,
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("reason=same_process_surface"));
    assert!(error.contains("inside_window=true"), "{error}");
    assert!(error.contains("hit_surface=normal"), "{error}");
    assert!(error.contains("hit_topmost=true"), "{error}");
    assert!(error.contains("target_hit=client"), "{error}");
    assert!(error.contains("surface_hit=client"), "{error}");
    assert_eq!(WHEELS.load(SeqCst), 1);
    target.release().unwrap();
    println!("PASS: a real covering window is rejected without leaking its identity or injecting through it");
    unsafe {
        SetWindowLongW(
            cover as HWND,
            GWL_EXSTYLE,
            (GetWindowLongW(cover as HWND, GWL_EXSTYLE) as u32 | WS_EX_LAYERED | WS_EX_TRANSPARENT)
                as i32,
        );
        assert_ne!(
            SetLayeredWindowAttributes(cover as HWND, 0, 180, LWA_ALPHA),
            0
        );
    }
    target
        .apply(InputEvent::WheelAt {
            x: 0.5,
            y: 0.5,
            width,
            height,
            delta: 120,
        })
        .unwrap();
    wait("wheel through click-through overlay", || {
        WHEELS.load(SeqCst) == 2
    });
    println!(
        "PASS: Windows routes through a layered transparent overlay without relaxing target checks"
    );
    drop(cleanup);
    ui.join().unwrap();
}
