#![windows_subsystem = "windows"]

mod app;
mod autostart;
mod brand;
mod config;
mod effects;
#[cfg(test)]
mod feel;
mod focus;
mod glyphs;
mod ink;
mod keys;
mod macro_send;
mod messages;
mod overlay;
mod physics;
mod rage;
mod render;
mod sound;
mod swing;
mod tipimg;
mod ui;
mod whipdraw;
mod whips;

use windows::core::{w, HSTRING};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW};

fn main() {
    unsafe {
        let _guard = CreateMutexW(None, true, w!("Local\\NJ-Whip"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            // Already running: bring its window forward instead of starting a second copy.
            if let Ok(h) = FindWindowW(None, &HSTRING::from(ui::TITLE)) {
                let _ = ShowWindow(h, SW_SHOW);
                let _ = ShowWindow(h, SW_RESTORE);
                let _ = SetForegroundWindow(h);
            }
            return;
        }
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        ui::run();
    }
}
