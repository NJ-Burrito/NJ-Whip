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
use crate::sound::Sound;
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

fn tip_image(cfg: &Config) -> Option<tiny_skia::Pixmap> {
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
    tip_img: Option<tiny_skia::Pixmap>,
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

    fn show_whip(&mut self) {
        self.ghost = false;
        self.overlay.show();
        self.spawn_whip();
    }

    fn spawn_whip(&mut self) {
        self.mouse_prev = self.overlay.cursor_local();
        let rope = Rope::spawn(&whips::def(self.cfg.whip).physics, self.mouse_prev);
        self.shown.clone_from(&rope.pts);
        self.whip = Some(Whip { rope, dropping: false, spawned: Instant::now() });
        self.swing = Swing::default();
        self.last_frame = Instant::now();
        self.acc = 0.0;
    }

    fn hide_whip(&mut self) {
        self.whip = None;
        if self.lingering() {
            self.start_ghost();
        } else {
            self.hide_overlay();
        }
    }

    fn hide_overlay(&mut self) {
        self.ghost = false;
        self.canvas = None;
        self.overlay.hide();
    }

    /// Something is still playing that outlives the whip: effects, meter, glow, combo label.
    fn lingering(&self) -> bool {
        self.fx.alive() || self.combo.visible(Instant::now()).is_some() || !self.heat.cold()
    }

    /// No whip on the mouse: keep the overlay up, click-through, for what is still playing.
    fn start_ghost(&mut self) {
        self.overlay.show_passive();
        self.ghost = true;
    }

    /// The loop animates (instead of blocking) while the whip or the ghost overlay is up.
    fn active(&self) -> bool {
        self.whip.is_some() || self.ghost
    }

    /// The whip is on the mouse: shown and not being dropped.
    fn held(&self) -> bool {
        self.whip.as_ref().is_some_and(|w| !w.dropping)
    }

    fn burst_point(&self) -> (f32, f32) {
        self.whip.as_ref().and_then(|w| w.rope.pts.last().copied()).unwrap_or_else(|| self.overlay.cursor_local())
    }

    fn toggle(&mut self) {
        match &mut self.whip {
            Some(w) if !w.dropping => w.dropping = true,
            _ if self.cfg.enabled => self.show_whip(),
            _ => {}
        }
        focus::restore();
    }

    /// One crack: sound, heat, combo, the lash thrown toward `dir` (-1 left, 1 right, 0 = as it
    /// lies), the message typed into the last window, the whip's effect at the tip.
    fn crack(&mut self, from_hotkey: bool, dir: i8) {
        if !self.cfg.enabled {
            return;
        }
        let def = whips::def(self.cfg.whip);
        self.sound.crack(def.cracks, def.sounds);
        if self.cfg.rage {
            self.heat.crack();
        }
        let when = Instant::now();
        let count = self.combo.crack(when);
        let label = if self.combo.labelled() { self.glyphs.as_ref().and_then(|g| g.line(&format!("{} ×{count}", def.vibe.shout))) } else { None };
        let lit = self.held();
        if let Some(w) = self.whip.as_mut().filter(|_| lit) {
            w.rope.kick(dir);
        }
        self.type_message(from_hotkey);
        let at = self.start_burst();
        self.mark = Some(Mark { when, at, dir, angle: fastrand::f32() * std::f32::consts::TAU, lit, label });
    }

    /// Type the crack's message: after Esc (the macro as set) or queued (its Esc steps left out),
    /// by the whip's intent, the pack and the "Always press Esc first" switch.
    fn type_message(&self, from_hotkey: bool) {
        let steps = if messages::after_esc(whips::def(self.cfg.whip).intent, self.cfg.style()) {
            self.cfg.macro_steps.clone()
        } else {
            queued(&self.cfg.macro_steps)
        };
        self.type_actions(from_hotkey, build_actions(&steps, &self.message()));
    }

    /// Send key actions to the last window on their own thread, so the whip never freezes while
    /// keys go out. Nothing is sent while an earlier message is still being typed.
    fn type_actions(&self, from_hotkey: bool, actions: Vec<KeyAction>) {
        if TYPING.swap(true, Ordering::AcqRel) {
            return;
        }
        focus::restore();
        std::thread::spawn(move || {
            // Typing with Ctrl/Alt still held would send chords, so wait for them to go up.
            if from_hotkey && !wait_modifiers_released_real(MODIFIER_WAIT) {
                log_error(&format!("modifier keys were still held after {} s, macro not sent", MODIFIER_WAIT.as_secs()));
            } else {
                let failed = send(&actions);
                if failed > 0 {
                    log_error(&format!("some keystrokes were refused by Windows ({failed} key events)"));
                }
            }
            TYPING.store(false, Ordering::Release);
        });
    }

    fn message(&self) -> String {
        let def = whips::def(self.cfg.whip);
        let line = match self.cfg.user_lines() {
            Some(l) => l[fastrand::usize(..l.len())].clone(),
            None => def.lines[fastrand::usize(..def.lines.len())].to_string(),
        };
        messages::compose(&line, def.intent, self.cfg.style())
    }

    fn start_burst(&mut self) -> (f32, f32) {
        if self.whip.is_none() {
            // also re-aims a hidden overlay at the cursor's monitor, so the point below is on it
            self.start_ghost();
        }
        let (w, h) = self.overlay.size();
        let at = self.burst_point();
        self.fx.burst(whips::def(self.cfg.whip).effect, at, (w as f32, h as f32));
        self.last_frame = Instant::now();
        at
    }

    /// Reset: two messages into the last window. "normal mode" alone ends sticky and always-on
    /// modes (nj-terse, ponytail); "Continue." then sends Claude back to the work.
    fn type_reset(&self) {
        self.type_actions(false, reset_actions(&self.cfg.macro_steps, messages::RESET, messages::RESUME));
    }

    fn on_thread_msg(&mut self, msg: u32) {
        match msg {
            WM_RELOAD => self.reload_config(),
            WM_RESET => self.type_reset(),
            WM_TEST => {
                let def = whips::def(self.cfg.whip);
                self.sound.preview(def.cracks, def.sounds);
                self.start_burst();
            }
            WM_SOUND => {
                let def = whips::def(self.cfg.whip);
                self.sound.preview(def.cracks, def.sounds);
            }
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

    fn frame(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        if self.overlay.move_to_cursor_monitor() {
            // The frame buffer was drawn for the old monitor (its work area may differ).
            self.canvas = None;
            if self.whip.is_none() {
                self.start_ghost();
            } else if self.whip.as_ref().is_some_and(|w| w.dropping) {
                // The cursor changed monitor. A whip already being dropped is hidden at once
                // (it was leaving anyway); otherwise restart the whip on the new monitor.
                self.hide_whip();
                return;
            } else {
                self.spawn_whip();
            }
        }
        let (w, h) = self.overlay.size();
        let def = whips::def(self.cfg.whip);
        let params = &def.physics;

        let mouse = self.overlay.cursor_local();
        let (mut whoosh, mut cracked) = (None, None);
        if let Some(whip) = &mut self.whip {
            let live = self.cfg.swing_crack
                && !whip.dropping
                && now.duration_since(whip.spawned) >= FIRST_CRACK_GRACE;
            let swing = &mut self.swing;
            each_step(&mut self.acc, dt, self.mouse_prev, mouse, |hand| {
                whip.rope.step(params, hand, (0.0, 0.0, w as f32, h as f32), whip.dropping);
                if !live {
                    return;
                }
                match swing.step(whip.rope.hand_vel().0) {
                    Some(Event::Whoosh { power, .. }) => whoosh = Some(power),
                    Some(Event::Crack { dir, .. }) => cracked = Some(dir),
                    None => {}
                }
            });
            let pts = &whip.rope.pts;
            if self.shown.len() != pts.len() || pts.iter().zip(&self.shown).any(|(a, b)| (a.0 - b.0).hypot(a.1 - b.1) > SHOW_MOVE) {
                self.shown.clone_from(pts);
            }
        }
        match (def.trail, &self.whip) {
            (Some(rgb), Some(whip)) if !whip.dropping => self.fx.trail(*whip.rope.pts.last().expect("a rope has points"), rgb),
            _ => self.fx.trail_end(),
        }
        let mouse_still = mouse == self.mouse_prev;
        self.mouse_prev = mouse;
        if let Some(dir) = cracked {
            self.crack(false, dir);
        } else if let Some(power) = whoosh {
            self.sound.whoosh(power, params.weight);
        }

        self.fx.step(dt);
        self.heat.step(dt);
        let since = |m: &Mark| now.saturating_duration_since(m.when).as_secs_f32();
        let flash = self.mark.as_ref().filter(|m| m.lit).map_or(0.0, |m| (1.0 - since(m) / FLASH_FOR).max(0.0));
        let vibe = def.vibe;
        let t = now.duration_since(self.epoch).as_secs_f32();
        let (level, offset, edges) = vibe.screen.look(t, self.mark.as_ref().map(|m| (since(m), m.angle)));
        let heat_on = !self.heat.cold();

        if self.whip.is_none() && !self.lingering() {
            self.hide_overlay();
            return;
        }
        if self.canvas.as_ref().is_none_or(|c| c.size() != (w, h)) {
            self.canvas = Canvas::new(w, h);
        }
        // The canvas repaints only what differs from the last frame: a still scene costs nothing.
        let scene = Scene {
            whip: self.whip.as_ref().map(|_| (&self.shown[..], def, self.tip_img.as_ref())),
            flash,
            glow: self.heat.glow() * level,
            edges,
            hue: vibe.hue,
            offset,
            scale: self.overlay.scale(),
            meter: (self.cfg.rage && (self.whip.is_some() || heat_on)).then(|| (self.heat.shown(), self.heat.full())),
            clock: t,
            combo: self.combo.visible(now).and_then(|age| {
                let m = self.mark.as_ref()?;
                let (path, width) = m.label.as_ref()?;
                Some(ComboView { path, width: *width, at: m.at, age, count: self.combo.count(), dir: m.dir })
            }),
            burst: Some(&self.fx),
        };
        let mut drew = true;
        if let Some(c) = &mut self.canvas {
            let dirty = c.draw(&scene, self.overlay.work_area());
            drew = !dirty.is_empty();
            self.overlay.present(&c.pm, &dirty);
        }

        if self.whip.as_ref().is_some_and(|w| w.dropping && w.rope.fallen_off(h as f32)) {
            self.hide_whip();
        }
        // Idle frame (nothing drawn, mouse still, whip hanging on the mouse): sleep until input
        // instead of waking for every compositor frame.
        if !drew && mouse_still && self.held() {
            unsafe { MsgWaitForMultipleObjects(None, false, IDLE_WAIT_MS, QS_ALLINPUT) };
        } else {
            vsync(now);
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
mod tests {
    use super::*;
    use crate::physics::Params;

    #[test]
    fn pace_sleeps_only_when_flush_returned_early() {
        let ms = Duration::from_millis;
        assert_eq!(pace_sleep(ms(10), ms(11)), Duration::ZERO, "a real wait is left alone");
        assert_eq!(pace_sleep(ms(0), ms(3)), ms(13), "instant flush: pad to 16 ms since frame start");
        assert_eq!(pace_sleep(ms(2), ms(5)), Duration::ZERO, "a short wait on a fast screen is real too");
        assert_eq!(pace_sleep(Duration::from_micros(300), ms(20)), Duration::ZERO, "frame already took longer");
    }

    /// The whole chain as the app runs it (frames of any length -> fixed steps -> weighted
    /// handle -> swing) reads a swing the same at every refresh rate, with uneven frames:
    /// a lazy sweep never cracks, a snap cracks exactly once.
    #[test]
    fn swing_reads_the_same_at_every_frame_rate() {
        const B: (f32, f32, f32, f32) = (0.0, 0.0, 1920.0, 1080.0);
        // The mouse's x at time t (s), the motions of swing.rs's own whole-chain test.
        // lazy: 300 px each way, once a second
        let lazy = |t: f32| 960.0 + 300.0 * (t * std::f32::consts::TAU).cos();
        // snap: 250 px to the left in 10 steps, then hard right at 3000 px/s for 12 steps
        let snap = |t: f32| {
            let s = t * 60.0;
            if s <= 10.0 { 1000.0 - 25.0 * s } else { 750.0 + 50.0 * (s - 10.0).min(12.0) }
        };
        let mut rng = fastrand::Rng::with_seed(2026);
        for weight in [0.6, 1.0, 1.8] {
            let p = Params { weight, ..Params::default() };
            for hz in [60.0, 75.0, 144.0, 240.0] {
                let mut cracks = |mouse: &dyn Fn(f32) -> f32, secs: f32| {
                    let mut prev = (mouse(0.0), 600.0);
                    let mut rope = Rope::spawn(&p, prev);
                    let (mut swing, mut acc, mut t, mut n) = (Swing::default(), 0.0, 0.0, 0);
                    while t < secs {
                        let dt = 1.0 / hz + (rng.f32() - 0.5) * 0.001; // +-0.5 ms of frame jitter
                        t += dt;
                        let at = (mouse(t), 600.0);
                        each_step(&mut acc, dt, prev, at, |hand| {
                            rope.step(&p, hand, B, false);
                            if let Some(Event::Crack { .. }) = swing.step(rope.hand_vel().0) {
                                n += 1;
                            }
                        });
                        prev = at;
                    }
                    n
                };
                assert_eq!(cracks(&lazy, 8.0), 0, "weight {weight} at {hz} Hz: lazy sweep");
                assert_eq!(cracks(&snap, 1.0), 1, "weight {weight} at {hz} Hz: snap");
            }
        }
    }

}
