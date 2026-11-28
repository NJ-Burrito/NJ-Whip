//! The NJ-Whip window: a header (ON/OFF, language) and a tab ribbon that stay in place, one
//! tab's content scrolling under them (whip, message, hotkeys, options, about), and a footer.
//! Every change is saved at once and the engine thread is told to apply it.

use crate::app::{self, post, QUITTING, WM_RELOAD, WM_RESET, WM_SOUND, WM_TEST};
use crate::lang::{Lang, Words, PIECE};
use crate::rage::CRACKS_TO_FULL;
use crate::brand::{self, Palette, GLACIER, POLAR_NIGHT};
use crate::config::{self, Config, MacroStep, Theme};
use crate::messages::{self, Pack, Profile};
use crate::whips::{self, WhipId, ALL};
use crate::sound::{self, Clip, Sound};
use crate::{autostart, glyphs, keys, tipimg, update, whipdraw};
use eframe::egui::text::ByteIndex;
use eframe::egui::{self, Color32, CornerRadius, FontFamily, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

mod chrome;
mod whip_tab;
mod message_tab;
mod options_tab;
mod widgets;
#[cfg(test)]
mod sheets;
pub const TITLE: &str = "NJ-Whip";
const MOTION: f32 = 0.2; // seconds, ease-out (egui's animate_* easing)

fn c(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

fn c_a(rgb: u32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, (a * 255.0) as u8)
}

/// Hotkey text for a key press, e.g. `Ctrl+Alt+W`. Names are the ones `keys::parse_hotkey` reads.
pub fn key_event_name(key: egui::Key, mods: egui::Modifiers) -> String {
    let mut s = String::new();
    for (on, name) in [(mods.ctrl, "Ctrl+"), (mods.alt, "Alt+"), (mods.shift, "Shift+")] {
        if on {
            s.push_str(name);
        }
    }
    s + &key_label(key)
}

fn key_label(key: egui::Key) -> String {
    match key {
        egui::Key::Escape => "Esc".into(),
        k => k.name().to_string(),
    }
}

fn macro_key_name(key: egui::Key) -> Option<String> {
    let n = key_label(key);
    keys::key_vk(&n).map(|_| n)
}

/// True for a hotkey whose only modifier is Shift on a key that is not F1-F24 (typing, not a hotkey).
fn shift_only_plain_key(key: egui::Key, mods: egui::Modifiers) -> bool {
    let name = key.name();
    let f_key = name.len() > 1 && name.starts_with('F') && name[1..].bytes().all(|b| b.is_ascii_digit());
    mods.shift && !mods.ctrl && !mods.alt && !f_key
}

const SITE: &str = "https://nj-burrito.github.io/NJ-Whip/";
const SOURCE: &str = "https://github.com/NJ-Burrito/NJ-Whip";
const CREDITS: &str = "https://github.com/NJ-Burrito/NJ-Whip/blob/main/CREDITS.md";
const MORE_TOOLS: &str = "https://nj-burrito.github.io/";
const RED: u32 = 0xE0524A;
/// A hover text is broken into rows this wide.
const TIP_WIDTH: f32 = 300.0;

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Whip,
    Message,
    Hotkeys,
    Options,
    About,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Whip, Tab::Message, Tab::Hotkeys, Tab::Options, Tab::About];

    fn name(self) -> &'static str {
        match self {
            Tab::Whip => "Whip",
            Tab::Message => "Message",
            Tab::Hotkeys => "Hotkeys",
            Tab::Options => "Options",
            Tab::About => "About",
        }
    }

    fn fade_id(self) -> egui::Id {
        egui::Id::unique("tab-in").with(self as u8)
    }
}

/// Where an update stands (only ever moves when the user presses a button in About).
#[derive(Clone, PartialEq)]
enum Update {
    Idle,
    Checking,
    Latest,
    Available(update::Version),
    Installing,
    /// The line that says what failed (English, with a `{}` for the reason) and the reason.
    Failed(&'static str, String),
    Restarting,
}

/// A line from `Words` laid out for a widget: each of its pieces is a section of its own, so
/// egui joins and directs each Arabic word by itself (see lang.rs).
fn job(line: &str, font: egui::FontId, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let format = egui::TextFormat { font_id: font, color, ..Default::default() };
    // Not `append`: it merges a piece into the one before it when their formats are the same.
    for piece in line.split(PIECE).filter(|p| !p.is_empty()) {
        let start = job.text.len();
        job.text.push_str(piece);
        job.sections.push(egui::text::LayoutSection { leading_space: 0.0, byte_range: ByteIndex(start)..ByteIndex(job.text.len()), format: format.clone() });
    }
    job
}

fn body_size(ui: &egui::Ui) -> f32 {
    egui::TextStyle::Body.resolve(ui.style()).size
}

/// A line as widget text in the body face and the widget's own colour.
fn tx(ui: &egui::Ui, line: &str) -> egui::WidgetText {
    job(line, egui::FontId::proportional(body_size(ui)), Color32::PLACEHOLDER).into()
}

/// A line as widget text at `size` in `color`.
fn txs(line: &str, size: f32, color: Color32) -> egui::WidgetText {
    job(line, egui::FontId::proportional(size), color).into()
}

/// A line as widget text in the bold face.
fn txb(line: &str, size: f32) -> egui::WidgetText {
    job(line, egui::FontId::new(size, FontFamily::Name("bold".into())), Color32::PLACEHOLDER).into()
}

/// A line that may need several rows, at `size` points in a space `width` wide (see
/// `Words::wrap`). The width of a joined Arabic word is not the sum of its letters', so the
/// rows are kept a little short of the space.
fn rows(w: &Words, ui: &egui::Ui, line: &str, size: f32, width: f32) -> String {
    let font = egui::FontId::proportional(size);
    w.wrap(line, width * 0.9, |s| ui.ctx().fonts_mut(|f| s.chars().map(|c| f.glyph_width(&font, c)).sum()))
}

/// A body line across the space that is left, at `size` in `color`.
fn para_c(w: &Words, ui: &egui::Ui, line: &str, size: f32, color: Color32) -> egui::WidgetText {
    txs(&rows(w, ui, line, size, ui.available_width()), size, color)
}

/// A body line across the space that is left.
fn para(w: &Words, ui: &egui::Ui, line: &str) -> egui::WidgetText {
    tx(ui, &rows(w, ui, line, body_size(ui), ui.available_width()))
}

/// A hover text.
fn tip(w: &Words, ui: &egui::Ui, line: &str) -> egui::WidgetText {
    tx(ui, &rows(w, ui, line, body_size(ui), TIP_WIDTH))
}

#[derive(PartialEq, Clone, Copy)]
enum Rec {
    Crack,
    Toggle,
    Macro,
}

fn error_stamp() -> [Option<SystemTime>; 2] {
    [app::hotkeys_path(), app::errors_path()].map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

fn read_errors() -> String {
    let read = |p| std::fs::read_to_string(p).unwrap_or_default();
    let (hk, er) = (read(app::hotkeys_path()), read(app::errors_path()));
    [hk.trim(), er.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("\n")
}

struct Window {
    cfg: Config,
    thumbs: Vec<egui::TextureHandle>,
    logos: [egui::TextureHandle; 2],
    /// The window's words in `cfg.lang`.
    words: Rc<Words>,
    /// The word "Arabic" in Arabic, for the language choice (shown in both languages).
    ar_name: String,
    tab: Tab,
    update: Arc<Mutex<Update>>,
    rec: Option<Rec>,
    /// A line in reading order (shown through `para`).
    hint: String,
    /// A line in reading order (shown through `para`).
    status: String,
    /// Plays the crack and sound previews of the window's own buttons.
    sound: Sound,
    new_line: String,
    errors: String,
    errors_stamp: [Option<SystemTime>; 2],
    /// config.toml's mtime as this window last read or wrote it (an outside edit is re-read).
    cfg_stamp: Option<SystemTime>,
    dark: bool,
}

fn texture(ctx: &egui::Context, name: &str, pm: &tiny_skia::Pixmap) -> egui::TextureHandle {
    let img = egui::ColorImage::from_rgba_unmultiplied([pm.width() as usize, pm.height() as usize], &brand::straight_rgba(pm));
    ctx.load_texture(name, img, egui::TextureOptions::LINEAR)
}

/// Each whip drawn in the mockup's resting arc on a transparent 300x170 card image.
fn whip_thumb(id: WhipId) -> tiny_skia::Pixmap {
    let def = whips::def(id);
    let mut pm = tiny_skia::Pixmap::new(300, 170).expect("size");
    let pts = whipdraw::preview_points(def.physics.points, (46.0, 140.0), 0.42);
    whipdraw::draw_whip(&mut pm, &pts, def, None);
    pm
}

impl Window {
    fn new(cc: &eframe::CreationContext) -> Self {
        Self::build(&cc.egui_ctx)
    }

    fn build(ctx: &egui::Context) -> Self {
        load_fonts(ctx);
        let thumbs = ALL.iter().map(|&id| texture(ctx, &format!("whip-{id:?}"), &whip_thumb(id))).collect();
        let logos = [
            texture(ctx, "logo-light", &brand::monogram(96, &GLACIER, false)),
            texture(ctx, "logo-dark", &brand::monogram(96, &POLAR_NIGHT, false)),
        ];
        let (cfg, cfg_stamp) = config::load();
        let cfg = cfg.unwrap_or_default();
        let sound = Sound::new(cfg.volume, cfg.sfx_volume);
        Window {
            words: Rc::new(Words::new(cfg.lang)),
            ar_name: Words::new(Lang::Ar).get("Arabic").to_string(),
            // NJ_WHIP_TAB=about opens on that tab (used to take the window's pictures).
            tab: std::env::var("NJ_WHIP_TAB").ok().and_then(|v| Tab::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(&v))).unwrap_or(Tab::Whip),
            update: Arc::new(Mutex::new(Update::Idle)),
            cfg,
            thumbs,
            logos,
            rec: None,
            hint: String::new(),
            status: String::new(),
            sound,
            new_line: String::new(),
            errors: read_errors(),
            errors_stamp: error_stamp(),
            cfg_stamp,
            dark: true,
        }
    }

    /// Arabic reads from the right: content rows start there.
    fn rtl(&self) -> bool {
        self.cfg.lang == Lang::Ar
    }

    fn pal(&self) -> &'static Palette {
        if self.dark { &POLAR_NIGHT } else { &GLACIER }
    }

    fn apply(&mut self) {
        if let Err(e) = self.validate() {
            self.status = e;
            return;
        }
        match config::save_to(&config::config_path(), &self.cfg) {
            Ok(()) => {
                self.status.clear();
                self.cfg_stamp = config::mtime();
                post(WM_RELOAD);
            }
            Err(e) => self.status = self.words.text("Couldn't save settings: {}", &[&e.to_string()]),
        }
    }

    fn validate(&self) -> Result<(), String> {
        let w = &self.words;
        let a = keys::parse_hotkey(&self.cfg.crack_hotkey).map_err(|e| w.text("Crack hotkey: {}", &[&e]))?;
        let b = keys::parse_hotkey(&self.cfg.toggle_hotkey).map_err(|e| w.text("Show/hide hotkey: {}", &[&e]))?;
        if a == b {
            return Err(w.text("Crack and show/hide need different hotkeys.", &[]));
        }
        if self.cfg.macro_steps.is_empty() {
            return Err(w.text("The keys a crack sends need at least one step.", &[]));
        }
        Ok(())
    }

    fn set_visuals(&mut self, ctx: &egui::Context) {
        let dark = match self.cfg.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => ctx.system_theme().is_none_or(|t| t == egui::Theme::Dark),
        };
        self.dark = dark;
        let p = self.pal();
        let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.panel_fill = c(p.bg);
        v.window_fill = c(p.panel);
        v.extreme_bg_color = c(p.selected);
        v.faint_bg_color = c(p.panel);
        v.override_text_color = Some(c(p.ink));
        v.hyperlink_color = c(p.accent);
        v.selection.bg_fill = c_a(p.accent, 0.35);
        v.selection.stroke = Stroke::new(1.0, c(p.ink));
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.noninteractive] {
            w.corner_radius = CornerRadius::same(10);
        }
        v.widgets.inactive.weak_bg_fill = c(p.selected);
        v.widgets.inactive.bg_fill = c(p.selected);
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, c_a(p.accent, 0.18));
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, c(p.ink));
        v.widgets.hovered.fg_stroke = Stroke::new(1.5, c(p.ink));
        // the slider's rail and the check box read as parts of the same family
        v.widgets.inactive.expansion = 0.0;
        v.widgets.hovered.expansion = 0.0;
        v.widgets.active.expansion = 0.0;
        v.widgets.hovered.weak_bg_fill = c_a(p.accent, 0.25);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, c(p.accent));
        v.widgets.active.weak_bg_fill = c_a(p.accent, 0.4);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c(p.line));
        v.slider_trailing_fill = true;
        ctx.set_visuals(v);
        ctx.all_styles_mut(|s| {
            s.spacing.item_spacing = Vec2::new(8.0, 8.0);
            s.spacing.button_padding = Vec2::new(14.0, 7.0);
            s.spacing.slider_rail_height = 6.0;
            s.spacing.interact_size.y = 30.0;
            s.animation_time = MOTION;
        });
    }

    fn record(&mut self, ctx: &egui::Context) {
        let Some(rec) = self.rec else { return };
        let events = ctx.input(|i| i.events.clone());
        let mut changed = false;
        for e in events {
            let egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } = e else { continue };
            match rec {
                Rec::Macro => {
                    if let Some(n) = macro_key_name(key) {
                        self.cfg.macro_steps.push(MacroStep::Key(n));
                        changed = true;
                    }
                }
                _ if key == egui::Key::Escape && !modifiers.any() => self.rec = None,
                _ if !(modifiers.ctrl || modifiers.alt || modifiers.shift) => {
                    self.hint = "Hold Ctrl, Alt or Shift together with the key.".into();
                }
                _ if shift_only_plain_key(key, modifiers) => {
                    self.hint = "Shift alone isn't enough: add Ctrl or Alt (Shift works alone only with F1–F24).".into();
                }
                _ => {
                    let name = key_event_name(key, modifiers);
                    match keys::parse_hotkey(&name) {
                        Ok(_) => {
                            if rec == Rec::Crack { self.cfg.crack_hotkey = name } else { self.cfg.toggle_hotkey = name }
                            self.hint.clear();
                            self.rec = None;
                            changed = true;
                        }
                        Err(e) => self.hint = format!("{name}: {e}"),
                    }
                }
            }
        }
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Key { .. })));
        if changed {
            self.apply();
        }
    }

}

fn card<R>(p: &Palette, ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
        egui::Frame::new()
            .fill(c(p.panel))
            .stroke(Stroke::new(1.0, c(p.line)))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(egui::Margin::same(18))
            .shadow(egui::epaint::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(if p.panel == POLAR_NIGHT.panel { 70 } else { 16 }) })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if !title.is_empty() {
                    ui.label(txb(title, 16.0));
                    ui.add_space(6.0);
                }
                body(ui)
            })
            .inner
}

impl Window {
}

impl eframe::App for Window {
    /// Runs every pass, also when the window is minimised or covered and `ui` is skipped, so a
    /// close request is always answered.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Closing the window keeps NJ-Whip in the tray; Quit (here or in the tray) really exits.
        if ctx.input(|i| i.viewport().close_requested()) && !QUITTING.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

impl Window {
    /// One pass of the window: the whole of what `eframe::App::ui` draws.
    fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.set_visuals(&ctx);
        self.record(&ctx);
        ctx.request_repaint_after(Duration::from_secs(1));
        if config::mtime() != self.cfg_stamp && self.rec.is_none() {
            let (cfg, at) = config::load();
            self.cfg_stamp = at;
            // An unreadable file leaves the window as it is; the engine reports it under Problems.
            if let Ok(cfg) = cfg {
                if cfg.lang != self.cfg.lang {
                    self.words = Rc::new(Words::new(cfg.lang));
                }
                self.cfg = cfg;
            }
        }
        let stamp = error_stamp();
        if stamp != self.errors_stamp {
            self.errors_stamp = stamp;
            self.errors = read_errors();
        }
        let p = *self.pal();
        let w = self.words.clone();
        let edge = |top: i8, bottom: i8| egui::Frame::new().fill(c(p.bg)).inner_margin(egui::Margin { left: 16, right: 16, top, bottom });
        // The header (with the tab ribbon) and the footer stay; only the tab's content scrolls.
        egui::Panel::top("header").resizable(false).show_separator_line(true).frame(edge(14, 0)).show(ui, |ui| {
            self.header(ui);
            ui.add_space(6.0);
            self.tabs(ui);
        });
        egui::Panel::bottom("footer").resizable(false).show_separator_line(true).frame(edge(8, 10)).show(ui, |ui| self.footer(ui));
        edge(10, 0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt(self.tab as u8).auto_shrink(false).show(ui, |ui| {
                let fade = ui.ctx().animate_bool_with_time(self.tab.fade_id(), true, MOTION);
                ui.multiply_opacity(fade);
                ui.add_space(6.0 * (1.0 - fade));
                self.problems(ui);
                match self.tab {
                    Tab::Whip => self.whip_tab(ui),
                    Tab::Message => self.message_tab(ui),
                    Tab::Hotkeys => {
                        card(&p, ui, "", |ui| {
                            self.hotkey_row(ui, "Crack", Rec::Crack);
                            widgets::rule(ui, &p);
                            self.hotkey_row(ui, "Show / hide the whip", Rec::Toggle);
                            if !self.hint.is_empty() {
                                widgets::rule(ui, &p);
                                ui.with_layout(widgets::col_layout(self.cfg.lang == Lang::Ar), |ui| ui.label(para_c(&w, ui, &self.hint, body_size(ui), c(RED))));
                            }
                        });
                        ui.add_space(10.0);
                        card(&p, ui, w.get("Keys a crack sends"), |ui| self.macro_editor(ui));
                    }
                    Tab::Options => self.options_tab(ui),
                    Tab::About => {
                        card(&p, ui, "", |ui| self.about_card(ui));
                        ui.add_space(10.0);
                        card(&p, ui, w.get("Updates"), |ui| self.updates_card(ui));
                    }
                }
                ui.add_space(10.0);
            });
        });
    }
}

/// Thmanyah is the house face: read from the user's fonts folder at runtime (licensed files, never
/// embedded). Missing files fall back to egui's defaults, and behind those to Windows' own
/// Segoe UI, which has the Arabic letters egui's faces lack. The window's Arabic is joined and
/// ordered in `lang.rs`.
fn load_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut defaults = fonts.families[&FontFamily::Proportional].clone();
    let segoe = std::env::var_os("WINDIR").map(|d| std::path::PathBuf::from(d).join(r"Fonts\segoeui.ttf"));
    if let Some(bytes) = segoe.and_then(|f| std::fs::read(f).ok()) {
        fonts.font_data.insert("system".into(), egui::FontData::from_owned(bytes).into());
        defaults.push("system".into());
    }
    // A family is its Thmanyah file in front of egui's own faces.
    let mut family = |key: &str, file: &str| {
        fonts.font_data.insert(key.into(), egui::FontData::from_owned(glyphs::user_font(file)?).into());
        Some(std::iter::once(key.to_string()).chain(defaults.iter().cloned()).collect::<Vec<_>>())
    };
    let regular = family("thmanyah", "thmanyahsans-Regular.otf").unwrap_or_else(|| defaults.clone());
    let bold = family("thmanyah-bold", "thmanyahsans-Bold.otf").unwrap_or_else(|| defaults.clone());
    let display = family("thmanyah-display", "thmanyahserifdisplay-Bold.otf").unwrap_or_else(|| bold.clone());
    // The window asks for "bold" and "display" by name, so both exist on every machine.
    fonts.families.insert(FontFamily::Proportional, regular);
    fonts.families.insert(FontFamily::Name("bold".into()), bold);
    fonts.families.insert(FontFamily::Name("display".into()), display);
    ctx.set_fonts(fonts);
}

pub fn run() {
    let icon = brand::monogram(256, &POLAR_NIGHT, true);
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([560.0, 760.0])
            .with_min_inner_size([480.0, 520.0])
            .with_icon(egui::IconData { rgba: brand::straight_rgba(&icon), width: 256, height: 256 }),
        ..Default::default()
    };
    let engine = std::sync::Arc::new(std::sync::Mutex::new(None::<std::thread::JoinHandle<()>>));
    let engine_slot = engine.clone();
    let _ = eframe::run_native(
        TITLE,
        opts,
        Box::new(|cc| {
            let ctx = cc.egui_ctx.clone();
            let handle = std::thread::Builder::new().name("nj-whip-engine".into()).spawn(move || app::run_engine(ctx)).ok();
            *engine_slot.lock().expect("engine slot") = handle;
            Ok(Box::new(Window::new(cc)))
        }),
    );
    // Window gone (Quit): stop the engine and wait for it, so its tray icon and overlay are removed.
    post(app::WM_QUIT_ENGINE);
    if let Some(h) = engine.lock().ok().and_then(|mut g| g.take()) {
        let _ = h.join();
    }
}

#[cfg(test)]
mod tests;
