//! Remembers the last real foreground window so a crack's macro lands there, not in us.

use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
use windows::Win32::UI::WindowsAndMessaging::*;

static LAST: AtomicIsize = AtomicIsize::new(0);

const IGNORED_CLASSES: [&str; 4] = [
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "NotifyIconOverflowWindow",
    "TopLevelWindowForOverflowXamlIsland",
];

/// Windows that must never become a macro's target: the taskbar and any window of ours.
fn is_ignored(class: &str, same_process: bool) -> bool {
    IGNORED_CLASSES.contains(&class) || same_process
}

fn note(hwnd: HWND) {
    if hwnd.0.is_null() {
        return;
    }
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
    let class = String::from_utf16_lossy(&buf[..n]);
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if is_ignored(&class, pid == std::process::id()) {
        return;
    }
    LAST.store(hwnd.0 as isize, Ordering::Relaxed);
}

unsafe extern "system" fn on_foreground(_: HWINEVENTHOOK, _: u32, hwnd: HWND, _: i32, _: i32, _: u32, _: u32) {
    note(hwnd);
}

/// Start tracking. Needs the calling thread's message loop to run. The hook lives until exit.
pub fn install() {
    note(unsafe { GetForegroundWindow() });
    unsafe {
        SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(on_foreground), 0, 0, WINEVENT_OUTOFCONTEXT);
    }
}

pub fn restore() {
    let h = HWND(LAST.load(Ordering::Relaxed) as *mut _);
    unsafe {
        if h.0.is_null() || !IsWindow(h).as_bool() || IsIconic(h).as_bool() || GetForegroundWindow() == h {
            return;
        }
        if !SetForegroundWindow(h).as_bool() {
            // Windows refuses when we have no input right now; a tapped Alt lifts the lock.
            keybd_event(VK_MENU.0 as u8, 0, Default::default(), 0);
            keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
            let _ = SetForegroundWindow(h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_windows() {
        assert!(is_ignored("Shell_TrayWnd", false));
        assert!(is_ignored("NotifyIconOverflowWindow", false));
        assert!(is_ignored("TopLevelWindowForOverflowXamlIsland", false));
        assert!(is_ignored("Notepad", true));
        assert!(!is_ignored("Notepad", false));
    }
}
