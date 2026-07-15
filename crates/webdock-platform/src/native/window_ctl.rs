//! Raise / close / bounds for target windows.

use tracing::{debug, warn};

use super::capture::{bounds_for_route, find_window, pid_for_route};
use crate::route::{is_display_route, window_id_from_route};
use crate::traits::*;
use crate::types::*;

pub struct NativeWindowControl;

impl NativeWindowControl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NativeWindowControl {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowControl for NativeWindowControl {
    fn raise(&self, w: &WindowRef) -> Result<(), WindowError> {
        if is_display_route(w.id) {
            return Ok(());
        }
        let pid = resolve_pid(w);

        // Windows/Linux: target the exact window from the route (the id *is*
        // HWND / X11 id) — pid-only raise picks an arbitrary window of the app.
        #[cfg(target_os = "windows")]
        {
            let r = raise_windows(window_id_from_route(w.id), pid);
            if r.is_ok() {
                debug!(pid, id = w.id.as_i64(), "raised window");
            }
            return r;
        }
        #[cfg(target_os = "linux")]
        {
            let r = raise_linux(window_id_from_route(w.id), pid);
            if r.is_ok() {
                debug!(pid, id = w.id.as_i64(), "raised window");
            }
            return r;
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            if pid <= 0 {
                return Err(WindowError::NotFound);
            }
            raise_pid(pid)?;
            debug!(pid, id = w.id.as_i64(), "raised window");
            Ok(())
        }
    }

    fn resize(&self, w: &WindowRef, size: Size) -> Result<(), WindowError> {
        #[cfg(target_os = "macos")]
        {
            return resize_macos(w, size);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (w, size);
            Err(WindowError::Other(
                "resize not implemented on this OS yet".into(),
            ))
        }
    }

    fn close(&self, w: &WindowRef) -> Result<(), WindowError> {
        if is_display_route(w.id) {
            return Ok(());
        }
        let pid = resolve_pid(w);
        if pid <= 0 {
            return Err(WindowError::NotFound);
        }
        let cg_wid = window_id_from_route(w.id).unwrap_or(0);
        let title = w
            .title
            .clone()
            .or_else(|| {
                window_id_from_route(w.id)
                    .and_then(find_window)
                    .and_then(|win| win.title().ok())
            })
            .unwrap_or_default();

        #[cfg(target_os = "macos")]
        {
            // WebDock path: AX close button by CGWindowID, else Cmd+W.
            // Never terminate the whole app (multi-window Terminal-safe).
            if super::ax_close::close_window(
                pid,
                cg_wid,
                Some(title.as_str()).filter(|s| !s.is_empty()),
            ) {
                return Ok(());
            }
            return Err(WindowError::Other(
                "close failed — grant Accessibility to WebRust in System Settings".into(),
            ));
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (cg_wid, title);
            self.raise(w)?;
            close_frontmost()
        }
    }

    fn bounds(&self, w: &WindowRef) -> Result<Rect, WindowError> {
        bounds_for_route(w.id).map_err(|e| match e {
            CaptureError::NotFound => WindowError::NotFound,
            other => WindowError::Other(other.to_string()),
        })
    }
}

fn resolve_pid(w: &WindowRef) -> i32 {
    if w.pid > 0 {
        return w.pid;
    }
    pid_for_route(w.id).unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn raise_pid(pid: i32) -> Result<(), WindowError> {
    let script = format!(
        r#"tell application "System Events" to set frontmost of first process whose unix id is {pid} to true"#
    );
    let status = std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()
        .map_err(|e| WindowError::Other(e.to_string()))?;
    if !status.success() {
        warn!(
            pid,
            "osascript raise failed — enable Accessibility for WebRust"
        );
        return Err(WindowError::Other(format!(
            "raise failed for pid {pid} (Accessibility permission?)"
        )));
    }
    Ok(())
}

/// Activate + raise on X11. Prefer the exact window id (route id); pid search
/// can activate the wrong window of a multi-window app.
#[cfg(target_os = "linux")]
fn raise_linux(wid: Option<u32>, pid: i32) -> Result<(), WindowError> {
    fn run(cmd: &str, args: &[&str]) -> bool {
        std::process::Command::new(cmd)
            .args(args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    if let Some(id) = wid {
        let id_s = id.to_string();
        if run("xdotool", &["windowactivate", "--sync", &id_s]) {
            let _ = run("xdotool", &["windowraise", &id_s]);
            return Ok(());
        }
        // wmctrl activates by hex/decimal id with -i.
        if run("wmctrl", &["-i", "-a", &id_s]) {
            return Ok(());
        }
    }
    if pid > 0
        && run(
            "xdotool",
            &[
                "search",
                "--pid",
                &pid.to_string(),
                "windowactivate",
                "--sync",
            ],
        )
    {
        return Ok(());
    }
    warn!(
        pid,
        ?wid,
        "raise failed — install xdotool (or wmctrl) for window focus"
    );
    // Soft-fail like before: input still lands if the window is already up.
    Ok(())
}

#[cfg(target_os = "macos")]
fn resize_macos(w: &WindowRef, size: Size) -> Result<(), WindowError> {
    let pid = resolve_pid(w);
    let title = window_id_from_route(w.id)
        .and_then(find_window)
        .and_then(|win| win.title().ok())
        .unwrap_or_default()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let script = format!(
        r#"tell application "System Events"
  set procs to every process whose unix id is {pid}
  if (count of procs) is 0 then return
  tell first item of procs
    try
      repeat with win in (every window)
        if name of win is "{title}" or "{title}" is "" then
          set size of win to {{{}, {}}}
          exit repeat
        end if
      end repeat
    end try
  end tell
end tell"#,
        size.w.round() as i32,
        size.h.round() as i32
    );
    let _ = std::process::Command::new("osascript")
        .args(["-e", &script])
        .status();
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn close_frontmost() -> Result<(), WindowError> {
    #[cfg(target_os = "windows")]
    {
        return close_windows_foreground();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdotool")
            .args(["getactivewindow", "windowclose"])
            .status();
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Err(WindowError::Other("unsupported".into()))
    }
}

#[cfg(target_os = "windows")]
fn raise_windows(wid: Option<u32>, pid: i32) -> Result<(), WindowError> {
    // windows crate 0.61: BOOL lives in windows_core; HWND is a thin wrapper.
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
        IsWindow, IsWindowVisible, SetForegroundWindow, SetWindowPos, ShowWindow, HWND_NOTOPMOST,
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_RESTORE,
    };

    // Exact HWND from the route (xcap window id *is* the HWND, ≤32 bits).
    let mut hwnd = wid
        .map(|id| HWND(id as usize as *mut core::ffi::c_void))
        .filter(|h| unsafe { IsWindow(Some(*h)).as_bool() });

    // Fallback: first visible top-level window of the pid.
    if hwnd.is_none() && pid > 0 {
        struct Ctx {
            pid: u32,
            hwnd: HWND,
        }
        let mut ctx = Ctx {
            pid: pid as u32,
            hwnd: HWND::default(),
        };
        unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let ctx = &mut *(lparam.0 as *mut Ctx);
            let mut win_pid = 0u32;
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut win_pid));
            if win_pid == ctx.pid && IsWindowVisible(hwnd).as_bool() {
                ctx.hwnd = hwnd;
                return BOOL(0); // FALSE — stop enum
            }
            BOOL(1) // TRUE — continue
        }
        unsafe {
            let _ = EnumWindows(Some(callback), LPARAM(&mut ctx as *mut _ as isize));
        }
        if !ctx.hwnd.0.is_null() {
            hwnd = Some(ctx.hwnd);
        }
    }
    let Some(hwnd) = hwnd else {
        return Err(WindowError::NotFound);
    };

    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd).as_bool() {
            return Ok(());
        }
        // Windows denies SetForegroundWindow to background processes (we run as
        // a tray/server app, so that's the normal case). Attach our input queue
        // to the current foreground thread — then the call is allowed.
        let fg = GetForegroundWindow();
        let cur = GetCurrentThreadId();
        let fg_tid = if fg.0.is_null() {
            0
        } else {
            GetWindowThreadProcessId(fg, None)
        };
        if fg_tid != 0 && fg_tid != cur {
            let attached = AttachThreadInput(cur, fg_tid, true).as_bool();
            let _ = BringWindowToTop(hwnd);
            let ok = SetForegroundWindow(hwnd).as_bool();
            if attached {
                let _ = AttachThreadInput(cur, fg_tid, false);
            }
            if ok {
                return Ok(());
            }
        }
        // Last resort: momentary TOPMOST toggle lifts the window above a
        // full-screen occluder even without keyboard focus, so pointer input
        // hits the right window.
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_NOTOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE,
        );
        let _ = SetForegroundWindow(hwnd);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn close_windows_foreground() -> Result<(), WindowError> {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, PostMessageW, WM_CLOSE};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return Err(WindowError::NotFound);
        }
        // windows 0.61: HWND arg is Option<HWND>
        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
    Ok(())
}
