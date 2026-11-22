use kokoda_app_model::input::{DragAnchor, InputEvent};
use kokoda_media::BackendResult as Result;
use std::{collections::BTreeSet, io, mem};
use winapi::{
    shared::{
        minwindef::FALSE,
        ntdef::HANDLE,
        windef::{HWND, POINT, RECT},
    },
    um::{
        dwmapi::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
        handleapi::CloseHandle,
        processthreadsapi::{GetCurrentThreadId, OpenProcess},
        synchapi::WaitForSingleObject,
        winnt::SYNCHRONIZE,
        winuser::*,
    },
};

pub struct Target {
    window: HWND,
    process: HANDLE,
    process_id: u32,
    thread_id: u32,
    keys: BTreeSet<u16>,
    buttons: BTreeSet<u8>,
    drag: Option<DragAnchor>,
    last_gesture: u64,
}

// Input always uses the same physical coordinate space as DWM capture bounds,
// even if a caller or a native callback changed this thread's DPI context.
struct PhysicalCoordinates(winapi::shared::windef::DPI_AWARENESS_CONTEXT);
impl PhysicalCoordinates {
    fn enter() -> Result<Self> {
        let previous = unsafe {
            SetThreadDpiAwarenessContext(
                winapi::shared::windef::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            )
        };
        if previous.is_null() {
            return Err(io::Error::last_os_error().into());
        }
        Ok(Self(previous))
    }
}
impl Drop for PhysicalCoordinates {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}

fn inject(input: &mut INPUT) -> Result<()> {
    if unsafe { SendInput(1, input, mem::size_of::<INPUT>() as i32) } != 1 {
        return Err(io::Error::other(
            "Windows rejected input; check foreground and process permissions",
        )
        .into());
    }
    Ok(())
}

fn key(vk: u16, down: bool) -> Result<()> {
    let mut input: INPUT = unsafe { mem::zeroed() };
    input.type_ = INPUT_KEYBOARD;
    unsafe {
        input.u.ki_mut().wVk = vk;
        let extended = matches!(vk, 0x21..=0x28 | 0x2D | 0x2E | 0xA3 | 0xA5 | 0x5B | 0x5C);
        input.u.ki_mut().dwFlags = if down { 0 } else { KEYEVENTF_KEYUP }
            | if extended { KEYEVENTF_EXTENDEDKEY } else { 0 };
    }
    inject(&mut input)
}

fn button(button: u8, down: bool) -> Result<()> {
    let flags = match (button, down) {
        (0, true) => MOUSEEVENTF_LEFTDOWN,
        (0, false) => MOUSEEVENTF_LEFTUP,
        (1, true) => MOUSEEVENTF_RIGHTDOWN,
        (1, false) => MOUSEEVENTF_RIGHTUP,
        (2, true) => MOUSEEVENTF_MIDDLEDOWN,
        (2, false) => MOUSEEVENTF_MIDDLEUP,
        _ => return Err("Unsupported mouse button".into()),
    };
    let mut input: INPUT = unsafe { mem::zeroed() };
    input.type_ = INPUT_MOUSE;
    unsafe {
        input.u.mi_mut().dwFlags = flags;
    }
    inject(&mut input)
}

impl Target {
    pub fn new(id: u64) -> Result<Self> {
        let window = id as usize as HWND;
        let mut process_id = 0;
        let thread_id = unsafe { GetWindowThreadProcessId(window, &mut process_id) };
        if thread_id == 0 {
            return Err("Target window is unavailable".into());
        }
        let process = unsafe { OpenProcess(SYNCHRONIZE, FALSE, process_id) };
        if process.is_null() {
            return Err(io::Error::last_os_error().into());
        }
        Ok(Self {
            window,
            process,
            process_id,
            thread_id,
            keys: BTreeSet::new(),
            buttons: BTreeSet::new(),
            drag: None,
            last_gesture: 0,
        })
    }

    /// Legacy Win32 caret, available in standard controls. Modern editors may not expose it.
    pub fn caret(&self) -> Option<(f64, f64)> {
        unsafe {
            if GetForegroundWindow() != self.window {
                return None;
            }
            let mut info: GUITHREADINFO = mem::zeroed();
            info.cbSize = mem::size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(self.thread_id, &mut info) == 0
                || info.hwndCaret.is_null()
                || (info.hwndCaret != self.window && IsChild(self.window, info.hwndCaret) == 0)
            {
                return None;
            }
            let mut point = POINT {
                x: info.rcCaret.left,
                y: info.rcCaret.bottom,
            };
            let mut bounds: RECT = mem::zeroed();
            // rcCaret belongs to the target control's logical coordinate space.
            // Match that control while converting to screen coordinates, then explicitly
            // convert to physical pixels to match Windows Graphics Capture and DWM bounds.
            let context = GetWindowDpiAwarenessContext(info.hwndCaret);
            let previous = SetThreadDpiAwarenessContext(context);
            if previous.is_null() {
                return None;
            }
            let mapped = ClientToScreen(info.hwndCaret, &mut point) != 0
                && LogicalToPhysicalPointForPerMonitorDPI(info.hwndCaret, &mut point) != 0;
            SetThreadDpiAwarenessContext(previous);
            if !mapped
                || DwmGetWindowAttribute(
                    self.window,
                    DWMWA_EXTENDED_FRAME_BOUNDS,
                    &mut bounds as *mut _ as *mut _,
                    mem::size_of::<RECT>() as u32,
                ) < 0
            {
                return None;
            }
            let width = bounds.right - bounds.left;
            let height = bounds.bottom - bounds.top;
            if width <= 0 || height <= 0 {
                return None;
            }
            let x = f64::from(point.x - bounds.left) / f64::from(width);
            let y = f64::from(point.y - bounds.top) / f64::from(height);
            if (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) {
                Some((x, y))
            } else {
                None
            }
        }
    }

    pub fn validate_identity(&self) -> Result<()> {
        let mut pid = 0;
        let thread = unsafe { GetWindowThreadProcessId(self.window, &mut pid) };
        if thread != self.thread_id
            || pid != self.process_id
            || unsafe { WaitForSingleObject(self.process, 0) } != 258
        {
            return Err("Target window or process has closed".into());
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_identity()?;
        if unsafe { IsWindowVisible(self.window) } == 0 {
            return Err("Target window is hidden".into());
        }
        if unsafe { IsIconic(self.window) } != 0 {
            return Err("Target window is minimized".into());
        }
        Ok(())
    }

    pub fn activate(&self) -> Result<()> {
        self.validate()?;
        unsafe {
            if IsWindowEnabled(self.window) == 0 {
                return Err("Target is disabled by a modal dialog; use the dialog window".into());
            }
            if GetForegroundWindow() == self.window
                || crate::window_management::transient_in_foreground(self.window as usize as u64)
            {
                return Ok(());
            }
            let source_thread = GetCurrentThreadId();
            let foreground_thread =
                GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
            let mut message: MSG = mem::zeroed();
            PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
            // Join both queues for activation, then detach before waiting or injecting.
            // The foreground queue can belong to another app (including Parallels tools).
            let foreground_attached = foreground_thread != 0
                && foreground_thread != source_thread
                && AttachThreadInput(source_thread, foreground_thread, 1) != 0;
            let target_attached = self.thread_id != source_thread
                && self.thread_id != foreground_thread
                && AttachThreadInput(source_thread, self.thread_id, 1) != 0;
            SetForegroundWindow(self.window);
            BringWindowToTop(self.window);
            if target_attached {
                AttachThreadInput(source_thread, self.thread_id, 0);
            }
            if foreground_attached {
                AttachThreadInput(source_thread, foreground_thread, 0);
            }
            // SetForegroundWindow may notify another input queue asynchronously.
            // Wait for that queue to handle the notification, bounded for hung apps.
            let mut result = 0;
            SendMessageTimeoutW(
                self.window,
                WM_NULL,
                0,
                0,
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                200,
                &mut result,
            );
            if GetForegroundWindow() != self.window {
                return Err("Windows did not allow activating the target window; close blocking dialogs or switch to the target on Windows".into());
            }
        }
        Ok(())
    }

    fn resolve_point(&self, x: f64, y: f64, width: u32, height: u32) -> Result<POINT> {
        if !x.is_finite() || !y.is_finite() || !(0.0..1.0).contains(&x) || !(0.0..1.0).contains(&y)
        {
            return Err("Pointer is outside the image".into());
        }
        let mut bounds: RECT = unsafe { mem::zeroed() };
        let result = unsafe {
            DwmGetWindowAttribute(
                self.window,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut bounds as *mut _ as *mut _,
                mem::size_of::<RECT>() as u32,
            )
        };
        if result < 0 {
            return Err("Cannot read target bounds".into());
        }
        if bounds.right - bounds.left != width as i32 || bounds.bottom - bounds.top != height as i32
        {
            return Err("Window resized; wait for the next frame".into());
        }
        Ok(POINT {
            x: bounds.left + (x * width as f64) as i32,
            y: bounds.top + (y * height as f64) as i32,
        })
    }

    fn point(&self, x: f64, y: f64, width: u32, height: u32, activate: bool) -> Result<()> {
        let mut point = self.resolve_point(x, y, width, height)?;
        if !self.hit(point) {
            // Hover over a covered/disabled owner must not steal focus from its
            // dialog or block the connection in foreground activation retries.
            if !activate {
                return Ok(());
            }
            self.activate()?;
            // Activation can move a popup or change its layout. Never reuse the
            // pre-activation screen location; a changed size requires a fresh frame.
            self.validate()?;
            point = self.resolve_point(x, y, width, height)?;
            if !self.hit(point) {
                return Err(self.pointer_error(point).into());
            }
        }
        // Do not force activation for already-visible popups: hover cards may hide on focus change.
        if unsafe { SetPhysicalCursorPos(point.x, point.y) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }

    fn hit(&self, point: POINT) -> bool {
        let hit = unsafe { WindowFromPhysicalPoint(point) };
        hit == self.window || unsafe { IsChild(self.window, hit) } != 0
    }

    fn pointer_error(&self, point: POINT) -> String {
        unsafe {
            let hit = WindowFromPhysicalPoint(point);
            let mut pid = 0;
            if !hit.is_null() {
                GetWindowThreadProcessId(hit, &mut pid);
            }
            let reason = if IsWindowEnabled(self.window) == 0 {
                "modal_disabled"
            } else if MonitorFromPoint(point, MONITOR_DEFAULTTONULL).is_null() {
                "off_screen"
            } else if hit.is_null() {
                "no_hit"
            } else if pid == self.process_id {
                "same_process_surface"
            } else {
                "other_process_surface"
            };
            // No title, path, class or identity of an unauthorized window is exposed.
            format!("Target is obscured at pointer location [reason={reason}; dpi={}; foreground={}; layered={}]",
                GetDpiForWindow(self.window), GetForegroundWindow()==self.window,
                GetWindowLongW(self.window,GWL_EXSTYLE) as u32 & WS_EX_LAYERED != 0)
        }
    }

    fn wheel(&self, delta: i32) -> Result<()> {
        if !(-1200..=1200).contains(&delta) {
            return Err("Wheel delta exceeds limit".into());
        }
        let mut point: POINT = unsafe { mem::zeroed() };
        if unsafe { GetPhysicalCursorPos(&mut point) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        if !self.hit(point) {
            return Err(self.pointer_error(point).into());
        }
        let mut input: INPUT = unsafe { mem::zeroed() };
        input.type_ = INPUT_MOUSE;
        unsafe {
            input.u.mi_mut().dwFlags = MOUSEEVENTF_WHEEL;
            input.u.mi_mut().mouseData = delta as u32;
        }
        inject(&mut input)
    }

    fn drag_point(&self, gesture: u64, dx: f64, dy: f64) -> Result<()> {
        let anchor = self.drag.ok_or("No active pointer gesture")?;
        let (x, y) = anchor
            .point(gesture, dx, dy)
            .ok_or("Invalid pointer gesture")?;
        let point = POINT { x, y };
        let mut info: GUITHREADINFO = unsafe { mem::zeroed() };
        info.cbSize = mem::size_of::<GUITHREADINFO>() as u32;
        let captured = unsafe {
            GetGUIThreadInfo(self.thread_id, &mut info) != 0
                && ((!info.hwndCapture.is_null()
                    && (info.hwndCapture == self.window
                        || IsChild(self.window, info.hwndCapture) != 0))
                    || (info.flags & GUI_INMOVESIZE != 0 && info.hwndMoveSize == self.window))
        };
        // Outside-target motion is only legal while this target actually owns
        // native capture/move-size. Never reactivate another window mid-gesture.
        if !captured && !self.hit(point) {
            return Err("Pointer gesture lost target capture".into());
        }
        if unsafe { SetPhysicalCursorPos(x, y) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }

    pub fn apply(&mut self, event: InputEvent) -> Result<()> {
        if matches!(event, InputEvent::Release) {
            return self.release();
        }
        // A key-down (Escape/Alt+F4) can close a transient window. Release only
        // keys injected by this target without trying to reactivate that HWND.
        if let InputEvent::Key { vk, down: false } = event {
            if self.keys.contains(&vk) {
                key(vk, false)?;
                self.keys.remove(&vk);
                return Ok(());
            }
        }
        let _coordinates = PhysicalCoordinates::enter()?;
        self.validate()?;
        if matches!(event, InputEvent::Key { .. } | InputEvent::Text { .. }) {
            self.activate()?;
        }
        match event {
            InputEvent::DragBegin {
                gesture,
                x,
                y,
                width,
                height,
            } => {
                if gesture == 0
                    || gesture <= self.last_gesture
                    || self.drag.is_some()
                    || !self.buttons.is_empty()
                {
                    return Err("Invalid pointer gesture start".into());
                }
                self.point(x, y, width, height, true)?;
                let mut point: POINT = unsafe { mem::zeroed() };
                if unsafe { GetPhysicalCursorPos(&mut point) } == 0 {
                    return Err(io::Error::last_os_error().into());
                }
                button(0, true)?;
                self.buttons.insert(0);
                self.last_gesture = gesture;
                self.drag = Some(DragAnchor {
                    gesture,
                    x: point.x,
                    y: point.y,
                });
            }
            InputEvent::DragMove { gesture, dx, dy } => self.drag_point(gesture, dx, dy)?,
            InputEvent::DragEnd { gesture, dx, dy } => {
                // Release even if the window resized/moved/disappeared from the
                // hit point; Group also releases on any input error.
                let moved = self.drag_point(gesture, dx, dy);
                let released = button(0, false);
                self.buttons.remove(&0);
                self.drag = None;
                released?;
                moved?;
            }
            InputEvent::Move {
                x,
                y,
                width,
                height,
            } => self.point(x, y, width, height, false)?,
            InputEvent::Button {
                x,
                y,
                width,
                height,
                button: which,
                down,
            } => {
                self.point(x, y, width, height, down)?;
                button(which, down)?;
                if down {
                    self.buttons.insert(which);
                } else {
                    self.buttons.remove(&which);
                }
            }
            InputEvent::WheelAt {
                x,
                y,
                width,
                height,
                delta,
            } => {
                if !(-1200..=1200).contains(&delta) {
                    return Err("Wheel delta exceeds limit".into());
                }
                self.point(x, y, width, height, true)?;
                self.wheel(delta)?;
            }
            InputEvent::Wheel { delta } => self.wheel(delta)?,
            InputEvent::Key { vk, down } => {
                if !(8..=254).contains(&vk) {
                    return Err("Unsupported virtual key".into());
                }
                key(vk, down)?;
                if down {
                    self.keys.insert(vk);
                } else {
                    self.keys.remove(&vk);
                }
            }
            InputEvent::Text { text } => {
                if text.len() > 4096 {
                    return Err("Text exceeds limit".into());
                }
                for unit in text.encode_utf16() {
                    let mut input: INPUT = unsafe { mem::zeroed() };
                    input.type_ = INPUT_KEYBOARD;
                    unsafe {
                        input.u.ki_mut().wScan = unit;
                        input.u.ki_mut().dwFlags = KEYEVENTF_UNICODE;
                    }
                    inject(&mut input)?;
                    unsafe {
                        input.u.ki_mut().dwFlags |= KEYEVENTF_KEYUP;
                    }
                    inject(&mut input)?;
                }
            }
            InputEvent::Release => {}
        }
        Ok(())
    }

    pub fn release(&mut self) -> Result<()> {
        self.drag = None;
        let mut first_error = None;
        for vk in std::mem::take(&mut self.keys) {
            if let Err(error) = key(vk, false) {
                first_error.get_or_insert(error);
            }
        }
        for which in std::mem::take(&mut self.buttons) {
            if let Err(error) = button(which, false) {
                first_error.get_or_insert(error);
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        if let Err(error) = self.release() {
            eprintln!("Input release failed: {error}");
        }
        unsafe {
            CloseHandle(self.process);
        }
    }
}
