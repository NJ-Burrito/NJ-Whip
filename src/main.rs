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
mod lang;
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
mod update;
mod whipdraw;
mod whips;

use windows::core::{w, HSTRING};
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW};

fn main() {
    unsafe {
        // Started by an update: the copy that started us is still closing, so wait for it.
        let mut tries = if std::env::args().any(|a| a == update::AFTER_UPDATE) { 50 } else { 1 };
        // A copy started to take the window's pictures (NJ_WHIP_TAB, see ui.rs) runs beside the real one.
        let name = if std::env::var_os("NJ_WHIP_TAB").is_some() { w!("Local\\NJ-Whip-pictures") } else { w!("Local\\NJ-Whip") };
        let _guard = loop {
            let guard = CreateMutexW(None, true, name);
            if GetLastError() != ERROR_ALREADY_EXISTS {
                break guard;
            }
            tries -= 1;
            if tries == 0 {
                // Already running: bring its window forward instead of starting a second copy.
                if let Ok(h) = FindWindowW(None, &HSTRING::from(ui::TITLE)) {
                    let _ = ShowWindow(h, SW_SHOW);
                    let _ = ShowWindow(h, SW_RESTORE);
                    let _ = SetForegroundWindow(h);
                }
                return;
            }
            if let Ok(h) = guard {
                let _ = CloseHandle(h);
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        update::tidy();
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        ui::run();
    }
}
