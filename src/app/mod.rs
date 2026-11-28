//! The whip engine: tray, hotkeys, overlay, crack, effects. Runs on its own thread with its own
//! Win32 message loop; the window (ui.rs) talks to it through the config file and thread messages.

use crate::config::{self, Config};
use crate::focus;
use crate::keys::parse_hotkey;
use crate::macro_send::{build_actions, queued, reset_actions, send, wait_modifiers_released_real, KeyAction};
use crate::overlay::{self, Overlay};
use crate::physics::Rope;
use crate::glyphs::Glyphs;
use crate::rage::{Combo, Heat};
use crate::render::{Canvas, ComboView, Scene};
use crate::sound::{self, Sound};
use crate::swing::{Event, Swing};
use crate::{brand, effects, messages, whips};
use eframe::egui;
use std::sync::atomic::{AtomicBool, AtomicU32};
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::*;

/// One physics step = one 1/60 s frame (the physics constants are tuned per frame).
mod whip_state;
mod typing;
mod frame;
const STEP: f32 = 1.0 / 60.0;
/// The drawn whip follows the rope once a point has moved this far (px) from where it is drawn.
const SHOW_MOVE: f32 = 0.3;
/// The tip flashes this long (s) after a crack.
const FLASH_FOR: f32 = 0.15;
/// No swing crack this soon after the whip appears.
const FIRST_CRACK_GRACE: Duration = Duration::from_millis(350);
const CONFIG_POLL_MS: u32 = 1000;
const MODIFIER_WAIT: Duration = Duration::from_secs(2);

/// Thread messages the window posts to the engine thread.
pub const WM_RELOAD: u32 = WM_APP + 1; // config.toml changed: apply now
pub const WM_RESET: u32 = WM_APP + 2; // type the Reset message into the last window
pub const WM_TEST: u32 = WM_APP + 3; // play the current whip's crack (sound + effect), no typing
pub const WM_QUIT_ENGINE: u32 = WM_APP + 4;
pub const WM_SOUND: u32 = WM_APP + 5; // play the current whip's sound only (a whip was picked)

/// The engine thread's id (0 until it runs) and the app-wide "really quit" flag.
pub static ENGINE_TID: AtomicU32 = AtomicU32::new(0);
pub static QUITTING: AtomicBool = AtomicBool::new(false);
/// A crack's message is being typed (on its own thread); a crack that lands meanwhile types nothing.
static TYPING: AtomicBool = AtomicBool::new(false);

/// Ask the engine thread to do something; ignored when it is not running.
pub fn post(msg: u32) {
    let tid = ENGINE_TID.load(Ordering::Relaxed);
    if tid != 0 {
        unsafe {
            let _ = PostThreadMessageW(tid, msg, windows::Win32::Foundation::WPARAM(0), windows::Win32::Foundation::LPARAM(0));
        }
    }
}

/// Really quit: closing the window otherwise only hides it to the tray (ui.rs).
pub fn request_quit(ctx: &egui::Context) {
    QUITTING.store(true, Ordering::Relaxed);
    post(WM_QUIT_ENGINE);
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}

pub fn errors_path() -> PathBuf {
    config::config_path().with_file_name("errors.txt")
}

/// Current hotkey problems (rewritten on every registration; Settings shows it with `errors.txt`).
pub fn hotkeys_path() -> PathBuf {
    config::config_path().with_file_name("hotkeys.txt")
}

/// Append a line to `%APPDATA%\NJ-Whip\errors.txt` (Settings shows it). Failures to log are ignored.
fn log_error(msg: &str) {
    let path = errors_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[unix {secs}] {msg}");
    }
}

/// Is the Windows taskbar light? (HKCU ... Themes\Personalize\SystemUsesLightTheme)
pub fn taskbar_is_light() -> bool {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut v: u32 = 0;
    let mut n = 4u32;
    let r = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            windows::core::w!("SystemUsesLightTheme"), RRF_RT_REG_DWORD, None, Some(&mut v as *mut u32 as *mut _), Some(&mut n))
    };
    r.is_ok() && v == 1
}

/// The NJ monogram for the tray, coloured for the taskbar's theme and drawn from the vector at
/// exactly the pixel size the tray slot has (16 at 100%, 24 at 150%, 32 at 200% ...), so Windows
/// never has to rescale a bitmap.
fn tray_icon() -> Option<Icon> {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};
    let pal = if taskbar_is_light() { brand::GLACIER } else { brand::POLAR_NIGHT };
    let size = (unsafe { GetSystemMetrics(SM_CXSMICON) }).clamp(16, 128) as u32;
    let pm = brand::monogram(size, &pal, false);
    Icon::from_rgba(brand::straight_rgba(&pm), size, size).ok()
}

struct Whip {
    rope: Rope,
    dropping: bool,
    spawned: Instant,
}

fn tip_image(cfg: &Config) -> Option<crate::tipimg::Tip> {
    cfg.custom_tips.get(&cfg.whip).and_then(|p| crate::tipimg::load(p))
}

/// What the last crack left on the screen.
struct Mark {
    when: Instant,
    /// Where it happened and on which side (-1 left, 1 right, 0 none).
    at: (f32, f32),
    dir: i8,
    /// A random direction (rad) for the Jolt screen effect.
    angle: f32,
    /// The whip was on the mouse: its tip flashes.
    lit: bool,
    /// The combo label as a path + its width (from the 2nd crack of a streak).
    label: Option<(tiny_skia::Path, f32)>,
}

struct App {
    cfg: Config,
    cfg_mtime: Option<SystemTime>,
    overlay: Overlay,
    sound: Sound,
    heat: Heat,
    combo: Combo,
    mark: Option<Mark>,
    swing: Swing,
    glyphs: Option<Glyphs>,
    tip_img: Option<crate::tipimg::Tip>,
    fx: effects::Fx,
    ctx: egui::Context,
    tray_light: bool,
    hotkeys: GlobalHotKeyManager,
    /// The registered hotkeys (None while OFF, or when one could not be registered).
    crack_key: Option<HotKey>,
    toggle_key: Option<HotKey>,
    whip: Option<Whip>,
    /// The overlay's frame buffer (None while hidden).
    canvas: Option<Canvas>,
    /// The overlay is up, click-through, with no whip (effects, meter, combo still playing).
    ghost: bool,
    /// The lash points last handed to the renderer: they follow the rope only when a point has
    /// moved more than `SHOW_MOVE`, so a whip at rest is the same scene frame after frame.
    shown: Vec<(f32, f32)>,
    /// The mouse at the previous frame (the physics steps in between follow it from there).
    mouse_prev: (f32, f32),
    epoch: Instant,
    last_frame: Instant,
    acc: f32,
    quit: bool,
    /// The tray menu's two entries (their words follow the window's language).
    menu: [MenuItem; 2],
    open_id: MenuId,
    quit_id: MenuId,
    tray: TrayIcon,
}

impl App {
    fn new(ctx: egui::Context) -> Result<App, String> {
        let (cfg, cfg_mtime) = config::load();
        let cfg = cfg.unwrap_or_else(|e| {
            log_error(&format!("config.toml could not be read, defaults are in use (a copy is kept as config.broken.toml): {e}"));
            Config::default()
        });
        let overlay = Overlay::new()?;
        let hotkeys = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;

        let open = MenuItem::new("Open NJ-Whip", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let menu = Menu::new();
        menu.append_items(&[&open, &quit]).map_err(|e| e.to_string())?;
        let icon = tray_icon().ok_or("tray icon")?;
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("NJ-Whip")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| e.to_string())?;

        let mut app = App {
            sound: Sound::new(cfg.volume, cfg.sfx_volume),
            tip_img: tip_image(&cfg),
            cfg,
            cfg_mtime,
            overlay,
            heat: Heat::default(),
            combo: Combo::default(),
            mark: None,
            swing: Swing::default(),
            glyphs: Glyphs::load(),
            fx: effects::Fx::default(),
            ctx,
            tray_light: taskbar_is_light(),
            hotkeys,
            crack_key: None,
            toggle_key: None,
            whip: None,
            canvas: None,
            ghost: false,
            shown: Vec::new(),
            mouse_prev: (0.0, 0.0),
            epoch: Instant::now(),
            last_frame: Instant::now(),
            acc: 0.0,
            quit: false,
            open_id: open.id().clone(),
            quit_id: quit.id().clone(),
            menu: [open, quit],
            tray,
        };
        // ON = the tool is live (hotkeys, cracks); the whip only appears with the show/hide hotkey.
        app.sound.set_loadout(sound::loadout(&app.cfg));
        app.sync_hotkeys();
        app.sync_menu();
        Ok(app)
    }

    /// The tray menu in the window's language (Windows joins and orders the Arabic itself).
    fn sync_menu(&self) {
        let words = crate::lang::Words::new(self.cfg.lang);
        for (item, en) in self.menu.iter().zip(["Open NJ-Whip", "Quit"]) {
            item.set_text(words.text(en, &[]));
        }
    }

    /// The hotkeys follow ON/OFF, and hotkeys.txt always holds the current problems (none while OFF).
    fn sync_hotkeys(&mut self) {
        for hk in [self.crack_key.take(), self.toggle_key.take()].into_iter().flatten() {
            let _ = self.hotkeys.unregister(hk);
        }
        let mut problems = Vec::new();
        if self.cfg.enabled {
            let mut register = |name: &str, spec: &str| {
                let hk = match parse_hotkey(spec) {
                    Ok(hk) => hk,
                    Err(e) => {
                        problems.push(format!("{name} hotkey '{spec}' is not valid: {e}"));
                        return None;
                    }
                };
                match self.hotkeys.register(hk) {
                    Ok(()) => Some(hk),
                    Err(e) => {
                        problems.push(format!("{name} hotkey '{spec}' could not be registered: {e}"));
                        None
                    }
                }
            };
            self.crack_key = register("crack", &self.cfg.crack_hotkey);
            self.toggle_key = register("toggle", &self.cfg.toggle_hotkey);
        }
        // Rewritten, never appended: the file always shows the current state.
        let path = hotkeys_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, problems.join("\n"));
    }

    /// The 1 s timer: follow the taskbar's theme, pick up config edits.
    fn on_timer(&mut self) {
        let light = taskbar_is_light();
        if light != self.tray_light {
            self.tray_light = light;
            let _ = self.tray.set_icon(tray_icon());
        }
        if config::mtime() != self.cfg_mtime {
            self.reload_config();
        }
    }

    fn reload_config(&mut self) {
        let (cfg, at) = config::load();
        self.cfg_mtime = at;
        let cfg = match cfg {
            Ok(cfg) => cfg,
            Err(e) => {
                log_error(&format!("config.toml could not be read, the last settings stay in use (a copy is kept as config.broken.toml): {e}"));
                return;
            }
        };
        let old = std::mem::replace(&mut self.cfg, cfg);
        self.sync_menu();
        self.sound.set_volume(self.cfg.volume, self.cfg.sfx_volume);
        if (old.whip, &old.custom_cracks, &old.custom_sfx) != (self.cfg.whip, &self.cfg.custom_cracks, &self.cfg.custom_sfx) {
            self.sound.set_loadout(sound::loadout(&self.cfg));
        }
        self.sync_hotkeys();
        if !self.cfg.rage {
            // The heat meter is switched off: nothing of it stays on the screen.
            self.heat = Heat::default();
        }
        if !self.cfg.enabled {
            self.whip = None;
            self.fx = effects::Fx::default();
            self.heat = Heat::default();
            self.combo = Combo::default();
            self.hide_overlay();
            return;
        }
        // Anything may look different now (the tip picture can change under the same name).
        self.canvas = None;
        self.tip_img = tip_image(&self.cfg);
        if self.held() && old.whip != self.cfg.whip {
            self.show_whip();
        }
    }

    fn on_thread_msg(&mut self, msg: u32) {
        match msg {
            WM_RELOAD => self.reload_config(),
            WM_RESET => self.type_reset(),
            WM_TEST => {
                self.sound.preview();
                self.start_burst();
            }
            WM_SOUND => self.sound.preview(),
            WM_QUIT_ENGINE => self.quit = true,
            _ => {}
        }
    }

    /// One message from the engine thread's queue: the window's requests (thread messages have
    /// no window), the 1 s timer, and everything else on to its window.
    fn dispatch(&mut self, msg: &MSG) {
        if msg.hwnd.is_invalid() && msg.message >= WM_APP {
            self.on_thread_msg(msg.message);
            return;
        }
        unsafe {
            let _ = TranslateMessage(msg);
            DispatchMessageW(msg);
        }
        if msg.message == WM_TIMER {
            self.on_timer();
        }
    }

    fn show_window(&self) {
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.ctx.request_repaint();
    }

    fn pump(&mut self) {
        if overlay::CLICKED.swap(false, Ordering::Relaxed) && self.held() {
            self.crack(false, 0);
        }
        while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
            if ev.state() != HotKeyState::Pressed {
                continue;
            }
            let is = |key: Option<HotKey>| key.is_some_and(|k| k.id() == ev.id());
            if is(self.crack_key) {
                self.crack(true, 0);
            } else if is(self.toggle_key) {
                self.toggle();
            }
        }
        while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                self.show_window();
            }
        }
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == self.quit_id {
                request_quit(&self.ctx);
            } else if ev.id == self.open_id {
                self.show_window();
            }
        }
    }

}

/// Longest wait after an idle frame, in ms. The overlay window receives mouse messages while the
/// whip is shown, so a mouse move ends the wait at once; the timeout is only a net for moves
/// over the taskbar, where the overlay is click-through.
const IDLE_WAIT_MS: u32 = 50;

/// Runs the fixed physics steps that fit in a frame `dt` seconds long (`acc` carries the rest
/// over). The mouse was at `from` at the last frame and is at `to` now: each step gets the
/// point it had reached at that step's own time, so the hand's speed per step does not depend
/// on how many steps a frame happens to hold (0, 1 or 2 at any refresh rate but an exact 60 Hz).
fn each_step(acc: &mut f32, dt: f32, from: (f32, f32), to: (f32, f32), mut step: impl FnMut((f32, f32))) {
    *acc += dt;
    while *acc >= STEP {
        *acc -= STEP;
        let u = (1.0 - *acc / dt.max(1e-6)).clamp(0.0, 1.0);
        step((from.0 + (to.0 - from.0) * u, from.1 + (to.1 - from.1) * u));
    }
}

/// Frame time to pace to when `DwmFlush` does not block.
const FRAME: Duration = Duration::from_millis(16);
/// A `DwmFlush` that returns faster than this was not really waiting for a frame (1 ms: on a
/// 144 Hz screen a real wait after a busy frame is only a few ms long).
const MIN_FLUSH: Duration = Duration::from_millis(1);

/// How long to sleep after a frame: nothing when the flush really waited, otherwise whatever
/// is left of ~16 ms since the frame started.
fn pace_sleep(flush_took: Duration, since_frame_start: Duration) -> Duration {
    if flush_took >= MIN_FLUSH { Duration::ZERO } else { FRAME.saturating_sub(since_frame_start) }
}

/// Wait for the next compositor frame. `DwmFlush` returns at once when DWM is not composing
/// (display off, session locked), so a too-fast return is paced by hand instead of spinning.
fn vsync(frame_start: Instant) {
    let t = Instant::now();
    let _ = unsafe { DwmFlush() };
    std::thread::sleep(pace_sleep(t.elapsed(), frame_start.elapsed()));
}

/// The engine thread body. `ctx` is the window's egui context (to show the window from the tray).
pub fn run_engine(ctx: egui::Context) {
    ENGINE_TID.store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    // At Windows start-up the taskbar may not exist yet, so the tray icon can fail: retry for ~20 s.
    let mut tries = 0;
    let mut app = loop {
        match App::new(ctx.clone()) {
            Ok(a) => break a,
            Err(e) if tries < 20 => {
                tries += 1;
                if tries == 1 {
                    log_error(&format!("start failed, retrying: {e}"));
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                // No engine: let the window's X really quit, and show the window (it lists errors.txt).
                log_error(&format!("start failed: {e}"));
                ENGINE_TID.store(0, Ordering::Relaxed);
                QUITTING.store(true, Ordering::Relaxed);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.request_repaint();
                return;
            }
        }
    };
    focus::install();
    unsafe { SetTimer(None, 0, CONFIG_POLL_MS, None) };

    let mut msg = MSG::default();
    while !app.quit {
        unsafe {
            if app.active() {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_QUIT {
                        app.quit = true;
                    }
                    app.dispatch(&msg);
                }
            } else {
                // Idle: block (0 CPU) until a message, hotkey, tray event or the config timer.
                if GetMessageW(&mut msg, None, 0, 0).0 <= 0 {
                    break;
                }
                app.dispatch(&msg);
            }
        }
        app.pump();
        if app.active() {
            app.frame();
        }
    }
    ENGINE_TID.store(0, Ordering::Relaxed);
    // The engine is gone (Quit, or its message loop ended): the window goes too.
    request_quit(&ctx);
}

#[cfg(test)]
mod tests;
