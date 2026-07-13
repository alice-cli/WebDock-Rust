//! Raise / close / bounds for target windows.

#[cfg(target_os = "macos")]
use tracing::info;
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
        if pid <= 0 {
            return Err(WindowError::NotFound);
        }
        raise_pid(pid)?;
        debug!(pid, id = w.id.as_i64(), "raised window");
        Ok(())
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
        let title = window_id_from_route(w.id)
            .and_then(find_window)
            .and_then(|win| win.title().ok())
            .unwrap_or_default();

        #[cfg(target_os = "macos")]
        {
            // 1) Accessibility close button (single window, keeps multi-window apps alive)
            if close_via_ax(pid, &title) {
                info!(pid, %title, "window closed via Accessibility");
                return Ok(());
            }
            // 2) Raise + Cmd+W
            raise_pid(pid)?;
            std::thread::sleep(std::time::Duration::from_millis(40));
            if close_via_cmd_w() {
                info!(pid, %title, "window closed via Cmd+W");
                return Ok(());
            }
            return Err(WindowError::Other(
                "close failed — grant Accessibility to WebRust".into(),
            ));
        }

        #[cfg(not(target_os = "macos"))]
        {
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

fn raise_pid(pid: i32) -> Result<(), WindowError> {
    #[cfg(target_os = "macos")]
    {
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
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        return raise_windows(pid);
    }

    #[cfg(target_os = "linux")]
    {
        let status = std::process::Command::new("xdotool")
            .args([
                "search",
                "--pid",
                &pid.to_string(),
                "windowactivate",
                "--sync",
            ])
            .status();
        match status {
            Ok(s) if s.success() => Ok(()),
            _ => {
                warn!(
                    pid,
                    "xdotool raise failed — install xdotool for window focus"
                );
                Ok(())
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = pid;
        Err(WindowError::Other("unsupported OS".into()))
    }
}

/// Close the matching window via AX close button (System Events).
#[cfg(target_os = "macos")]
fn close_via_ax(pid: i32, title: &str) -> bool {
    let title_esc = title.replace('\\', "\\\\").replace('"', "\\\"");
    // Prefer window whose name matches title; else first window of the process.
    let script = if title.is_empty() {
        format!(
            r#"tell application "System Events"
  set procs to every process whose unix id is {pid}
  if (count of procs) is 0 then return false
  tell first item of procs
    try
      set frontmost to true
      delay 0.05
      if (count of windows) is 0 then return false
      try
        click (first button of window 1 whose subrole is "AXCloseButton")
        return true
      end try
      try
        perform action "AXPress" of (first button of window 1 whose description is "close button")
        return true
      end try
      -- last resort: button 1 is often the close button on standard windows
      click button 1 of window 1
      return true
    end try
  end tell
  return false
end tell"#
        )
    } else {
        format!(
            r#"tell application "System Events"
  set procs to every process whose unix id is {pid}
  if (count of procs) is 0 then return false
  tell first item of procs
    try
      set frontmost to true
      delay 0.05
      set wlist to every window
      repeat with win in wlist
        try
          if name of win is "{title_esc}" or name of win contains "{title_esc}" then
            try
              click (first button of win whose subrole is "AXCloseButton")
              return true
            end try
            try
              click button 1 of win
              return true
            end try
          end if
        end try
      end repeat
      if (count of windows) > 0 then
        try
          click (first button of window 1 whose subrole is "AXCloseButton")
          return true
        end try
        click button 1 of window 1
        return true
      end if
    end try
  end tell
  return false
end tell"#
        )
    };
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output();
    match output {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            o.status.success() && s.contains("true")
        }
        Err(_) => false,
    }
}

#[cfg(target_os = "macos")]
fn close_via_cmd_w() -> bool {
    let status = std::process::Command::new("osascript")
        .args([
            "-e",
            r#"tell application "System Events" to keystroke "w" using command down"#,
        ])
        .status();
    matches!(status, Ok(s) if s.success())
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
fn raise_windows(pid: i32) -> Result<(), WindowError> {
    // windows crate 0.61: BOOL lives in windows_core; HWND is a thin wrapper.
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow,
        SW_RESTORE,
    };

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
        if ctx.hwnd.0.is_null() {
            return Err(WindowError::NotFound);
        }
        let _ = ShowWindow(ctx.hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(ctx.hwnd);
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
