//! The overlay window: one borderless, transparent, topmost layered window over the cursor's
//! monitor. It never takes focus; frames are pushed with `UpdateLayeredWindow` (premultiplied BGRA).

use std::sync::atomic::{AtomicBool, Ordering};
use tiny_skia::{Pixmap, Rect};
use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Set by the window procedure on `WM_LBUTTONDOWN`; the app loop takes it.
pub static CLICKED: AtomicBool = AtomicBool::new(false);

extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_LBUTTONDOWN => {
            CLICKED.store(true, Ordering::Relaxed);
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        // The whip is the cursor while shown .
        WM_SETCURSOR => {
            unsafe { SetCursor(None) };
            LRESULT(1)
        }
        _ => unsafe { DefWindowProcW(h, m, w, l) },
    }
}

/// A 32-bit top-down DIB section selected into a memory DC.
struct Dib {
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    size: (u32, u32),
}

impl Dib {
    fn new(w: u32, h: u32) -> Option<Dib> {
        unsafe {
            let dc = CreateCompatibleDC(None);
            let mut info = BITMAPINFO::default();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let Ok(bmp) = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
                let _ = DeleteDC(dc);
                return None;
            };
            let old = SelectObject(dc, bmp);
            Some(Dib { dc, bmp, old, bits: bits as *mut u8, size: (w, h) })
        }
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bmp);
            let _ = DeleteDC(self.dc);
        }
    }
}

pub struct Overlay {
    hwnd: HWND,
    mon: HMONITOR,
    origin: (i32, i32),
    size: (u32, u32),
    /// The monitor's work area (no taskbar) in overlay coordinates: where the whip catches clicks.
    work: Rect,
    /// The monitor's display scaling (DPI / 96): 1.0 at 100%.
    scale: f32,
    dib: Option<Dib>,
}

/// Copies one box of a `w` px wide frame: premultiplied RGBA `src` -> premultiplied BGRA `dst`.
pub fn to_bgra(dst: &mut [u8], src: &[u8], w: i32, (l, t, r, b): (i32, i32, i32, i32)) {
    for y in t..b {
        let (i, j) = ((y * w + l) as usize * 4, (y * w + r) as usize * 4);
        for (d, s) in dst[i..j].chunks_exact_mut(4).zip(src[i..j].chunks_exact(4)) {
            d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
        }
    }
}

fn cursor_pos() -> POINT {
    let mut pt = POINT::default();
    let _ = unsafe { GetCursorPos(&mut pt) };
    pt
}

impl Overlay {
    pub fn new() -> Result<Overlay, String> {
        unsafe {
            let hinst = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let class = w!("NJWhipOverlay");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinst.into(),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                class,
                // Not the Settings window's title: a second launch finds that window by title (main.rs).
                w!("NJ-Whip overlay"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                hinst,
                None,
            )
            .map_err(|e| e.to_string())?;
            Ok(Overlay { hwnd, mon: HMONITOR::default(), origin: (0, 0), size: (0, 0), work: Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap(), scale: 1.0, dib: None })
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn work_area(&self) -> Rect {
        self.work
    }

    /// Display scaling of the overlay's monitor (1.0 = 100%): the meter, glow and combo grow by it.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Cursor position relative to the overlay's top-left.
    pub fn cursor_local(&self) -> (f32, f32) {
        let pt = cursor_pos();
        ((pt.x - self.origin.0) as f32, (pt.y - self.origin.1) as f32)
    }

    /// Re-aim at the monitor under the cursor. True when the monitor, its size, its work area
    /// (taskbar moved, resolution changed) or its scaling is not what the overlay was laid out for.
    pub fn move_to_cursor_monitor(&mut self) -> bool {
        unsafe {
            let mon = MonitorFromPoint(cursor_pos(), MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if !GetMonitorInfoW(mon, &mut mi).as_bool() {
                return false;
            }
            let r = mi.rcMonitor;
            let origin = (r.left, r.top);
            let size = ((r.right - r.left) as u32, (r.bottom - r.top) as u32);
            let wa = mi.rcWork;
            let Some(work) = Rect::from_ltrb(
                (wa.left - r.left) as f32,
                (wa.top - r.top) as f32,
                (wa.right - r.left) as f32,
                (wa.bottom - r.top) as f32,
            )
            .or_else(|| Rect::from_xywh(0.0, 0.0, size.0 as f32, size.1 as f32)) else {
                return false;
            };
            // 96 (100%) when Windows cannot say
            let (mut dpi, mut dpi_y) = (96u32, 96u32);
            let _ = GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
            let scale = dpi.max(96) as f32 / 96.0;
            if mon == self.mon && origin == self.origin && size == self.size && work == self.work && scale == self.scale {
                return false;
            }
            self.scale = scale;
            self.mon = mon;
            self.origin = origin;
            self.size = size;
            self.work = work;
            self.dib = None;
            true
        }
    }

    /// Become visible and clickable (without activating).
    pub fn show(&mut self) {
        self.show_with(false);
    }

    /// Become visible but click-through (rage effects with no whip).
    pub fn show_passive(&mut self) {
        self.show_with(true);
    }

    fn show_with(&mut self, click_through: bool) {
        self.move_to_cursor_monitor();
        self.set_click_through(click_through);
        unsafe {
            let _ = SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    pub fn hide(&mut self) {
        self.set_click_through(true);
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
        self.dib = None;
        self.mon = HMONITOR::default();
    }

    fn set_click_through(&self, on: bool) {
        unsafe {
            let ex = GetWindowLongW(self.hwnd, GWL_EXSTYLE) as u32;
            let flag = WS_EX_TRANSPARENT.0;
            let new = if on { ex | flag } else { ex & !flag };
            SetWindowLongW(self.hwnd, GWL_EXSTYLE, new as i32);
        }
    }

    /// Push the changed boxes of a frame (tiny-skia premultiplied RGBA -> premultiplied BGRA).
    /// `pm` must match `size()`. A fresh frame buffer (first frame after show or a monitor
    /// change) is always pushed whole.
    pub fn present(&mut self, pm: &Pixmap, dirty: &[(i32, i32, i32, i32)]) {
        if (pm.width(), pm.height()) != self.size {
            return;
        }
        let (w, h) = (self.size.0 as i32, self.size.1 as i32);
        let whole = [(0, 0, w, h)];
        let dirty = if self.dib.as_ref().is_none_or(|d| d.size != self.size) {
            self.dib = Dib::new(self.size.0, self.size.1);
            &whole[..]
        } else {
            dirty
        };
        let Some(dib) = &self.dib else { return };
        let dst = unsafe { std::slice::from_raw_parts_mut(dib.bits, pm.data().len()) };
        let src = pm.data();
        let pos = POINT { x: self.origin.0, y: self.origin.1 };
        let size = SIZE { cx: w, cy: h };
        let zero = POINT::default();
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        for &(l, t, r, b) in dirty {
            if l < 0 || t < 0 || r > w || b > h || l >= r || t >= b {
                continue;
            }
            to_bgra(dst, src, w, (l, t, r, b));
            let rect = RECT { left: l, top: t, right: r, bottom: b };
            let info = UPDATELAYEREDWINDOWINFO {
                cbSize: std::mem::size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
                hdcDst: HDC::default(),
                pptDst: &pos,
                psize: &size,
                hdcSrc: dib.dc,
                pptSrc: &zero,
                crKey: COLORREF(0),
                pblend: &blend,
                dwFlags: ULW_ALPHA,
                prcDirty: &rect,
            };
            unsafe {
                let _ = UpdateLayeredWindowIndirect(self.hwnd, &info);
            }
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
