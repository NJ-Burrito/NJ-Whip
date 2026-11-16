//! The NJ-Whip window: ON/OFF, whip picker, messages, custom tip, hotkeys, options.
//! Every change is saved at once and the engine thread is told to apply it.

use crate::app::{self, post, QUITTING, WM_RELOAD, WM_RESET, WM_TEST};
use crate::rage::CRACKS_TO_FULL;
use crate::brand::{self, Palette, GLACIER, POLAR_NIGHT};
use crate::config::{self, Config, MacroStep, Theme};
use crate::messages::{self, Pack, Profile};
use crate::whips::{self, WhipId, ALL};
use crate::{autostart, glyphs, keys, tipimg, whipdraw};
use eframe::egui::{self, Color32, CornerRadius, FontFamily, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime};

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
    rec: Option<Rec>,
    hint: String,
    status: String,
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
        let ctx = &cc.egui_ctx;
        load_fonts(ctx);
        let thumbs = ALL.iter().map(|&id| texture(ctx, &format!("whip-{id:?}"), &whip_thumb(id))).collect();
        let logos = [
            texture(ctx, "logo-light", &brand::monogram(96, &GLACIER, false)),
            texture(ctx, "logo-dark", &brand::monogram(96, &POLAR_NIGHT, false)),
        ];
        let (cfg, cfg_stamp) = config::load();
        Window {
            cfg: cfg.unwrap_or_default(),
            thumbs,
            logos,
            rec: None,
            hint: String::new(),
            status: String::new(),
            new_line: String::new(),
            errors: read_errors(),
            errors_stamp: error_stamp(),
            cfg_stamp,
            dark: true,
        }
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
            Err(e) => self.status = format!("Couldn't save settings: {e}"),
        }
    }

    fn validate(&self) -> Result<(), String> {
        let a = keys::parse_hotkey(&self.cfg.crack_hotkey).map_err(|e| format!("Crack hotkey: {e}"))?;
        let b = keys::parse_hotkey(&self.cfg.toggle_hotkey).map_err(|e| format!("Show/hide hotkey: {e}"))?;
        if a == b {
            return Err("Crack and show/hide need different hotkeys.".into());
        }
        if self.cfg.macro_steps.is_empty() {
            return Err("The keys a crack sends need at least one step.".into());
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
            w.corner_radius = CornerRadius::same(8);
        }
        v.widgets.inactive.weak_bg_fill = c(p.selected);
        v.widgets.inactive.bg_fill = c(p.selected);
        v.widgets.hovered.weak_bg_fill = c_a(p.accent, 0.25);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, c(p.accent));
        v.widgets.active.weak_bg_fill = c_a(p.accent, 0.4);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c(p.line));
        v.slider_trailing_fill = true;
        ctx.set_visuals(v);
        ctx.all_styles_mut(|s| {
            s.spacing.item_spacing = Vec2::new(8.0, 8.0);
            s.spacing.button_padding = Vec2::new(12.0, 6.0);
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
            .corner_radius(CornerRadius::same(14))
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(title).size(16.0).family(FontFamily::Name("bold".into())));
                ui.add_space(4.0);
                body(ui)
            })
            .inner
}

impl Window {
    /// The big ON/OFF switch: the knob slides and the track fills, one ease-out motion.
    fn switch(&mut self, ui: &mut egui::Ui) {
        let p = self.pal();
        let size = Vec2::new(64.0, 34.0);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let t = ui.ctx().animate_bool_with_time(resp.id, self.cfg.enabled, MOTION);
        let off = c(p.line);
        let on = c(p.accent);
        let track = Color32::from_rgb(
            (off.r() as f32 + (on.r() as f32 - off.r() as f32) * t) as u8,
            (off.g() as f32 + (on.g() as f32 - off.g() as f32) * t) as u8,
            (off.b() as f32 + (on.b() as f32 - off.b() as f32) * t) as u8,
        );
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(17), track);
        let x = rect.left() + 17.0 + (rect.width() - 34.0) * t;
        painter.circle_filled(egui::pos2(x, rect.center().y + 1.0), 13.0, Color32::from_black_alpha(40));
        painter.circle_filled(egui::pos2(x, rect.center().y), 13.0, c(p.panel));
        if resp.hovered() {
            painter.rect_stroke(rect, CornerRadius::same(17), Stroke::new(1.5, c(p.accent)), StrokeKind::Outside);
        }
        if resp.clicked() {
            self.cfg.enabled = !self.cfg.enabled;
            self.apply();
        }
        resp.on_hover_text(if self.cfg.enabled { "NJ-Whip is ON: hotkeys and cracks work" } else { "NJ-Whip is OFF: nothing on screen, hotkeys released" });
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let logo = &self.logos[self.dark as usize];
            ui.add(egui::Image::new(logo).fit_to_exact_size(Vec2::splat(44.0)));
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(RichText::new("NJ-Whip").size(24.0).family(FontFamily::Name("display".into())));
                let p = self.pal();
                let state = if self.cfg.enabled { "On · show the whip with the show/hide hotkey" } else { "Off" };
                ui.label(RichText::new(state).size(13.0).color(c(p.muted)));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.switch(ui));
        });
    }

    fn whip_picker(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let cols = 3;
        let gap = 8.0;
        let w = ((ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32).floor();
        let h = w * 0.62 + 24.0;
        let mut pick = None;
        egui::Grid::new("whips").spacing(Vec2::splat(gap)).show(ui, |ui| {
            for (k, &id) in ALL.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
                let sel = self.cfg.whip == id;
                let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), MOTION);
                let painter = ui.painter();
                let fill = if sel { c(p.selected) } else { c(p.bg) };
                painter.rect_filled(rect, CornerRadius::same(12), fill);
                let stroke = if sel { Stroke::new(2.0, c(p.accent)) } else { Stroke::new(1.0, c_a(p.accent, 0.15 + 0.6 * hover)) };
                painter.rect_stroke(rect, CornerRadius::same(12), stroke, StrokeKind::Inside);
                let img_rect = egui::Rect::from_min_size(rect.min + Vec2::new(4.0, 4.0 - 3.0 * hover), Vec2::new(w - 8.0, w * 0.62 - 4.0));
                egui::Image::new(&self.thumbs[k]).paint_at(ui, img_rect);
                painter.text(
                    egui::pos2(rect.center().x, rect.bottom() - 14.0),
                    egui::Align2::CENTER_CENTER,
                    whips::def(id).name,
                    egui::FontId::proportional(13.0),
                    c(p.ink),
                );
                if resp.clicked() {
                    pick = Some(id);
                }
                if (k + 1) % cols == 0 {
                    ui.end_row();
                }
            }
        });
        if let Some(id) = pick {
            if id != self.cfg.whip {
                self.cfg.whip = id;
                self.apply();
            }
        }
        let def = whips::def(self.cfg.whip);
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(def.name).family(FontFamily::Name("bold".into())));
            ui.label(RichText::new(format!("· {}", def.intent.name())).color(c(p.accent)));
        });
        ui.label(RichText::new(def.blurb).color(c(p.muted)));
        ui.horizontal(|ui| {
            if ui.button("Test crack").on_hover_text("Plays the crack and its effect; types nothing").clicked() {
                post(WM_TEST);
            }
            let has_tip = self.cfg.custom_tips.contains_key(&self.cfg.whip);
            if ui.button(if has_tip { "Change tip picture…" } else { "Use my picture as the tip…" }).clicked() {
                self.pick_tip();
            }
            if has_tip && ui.button("Remove picture").clicked() {
                self.cfg.custom_tips.remove(&self.cfg.whip);
                self.apply();
            }
        });
    }

    fn pick_tip(&mut self) {
        let Some(src) = rfd::FileDialog::new().add_filter("PNG picture", &["png"]).pick_file() else { return };
        let name = format!("{:?}", self.cfg.whip).to_lowercase();
        match tipimg::import(&src, &name) {
            Ok(dest) => {
                self.cfg.custom_tips.insert(self.cfg.whip, dest);
                self.apply();
            }
            Err(e) => self.status = e,
        }
    }

    fn messages_card(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let def = whips::def(self.cfg.whip);
        let small = |s: &str| RichText::new(s).size(12.0).color(c(p.muted));
        ui.label("Instruction pack");
        let (mut pack, mut profile) = (self.cfg.pack, self.cfg.profile);
        for k in Pack::ALL {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut pack, k, k.name());
                ui.label(small(k.note()));
            });
        }
        ui.add_enabled_ui(pack.uses_profile(), |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Session");
                for f in [Profile::Light, Profile::Heavy] {
                    ui.selectable_value(&mut profile, f, f.label());
                }
            });
        })
        .response
        .on_disabled_hover_text("This pack words its messages the same for every session");
        if (pack, profile) != (self.cfg.pack, self.cfg.profile) {
            (self.cfg.pack, self.cfg.profile) = (pack, profile);
            self.apply();
        }
        let style = self.cfg.style();
        let line = self.cfg.user_lines().map_or(def.lines[0], |l| l[0].as_str());
        let sample = messages::compose(line, def.intent, style);
        egui::Frame::new().fill(c(p.bg)).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(sample).italics().color(c(p.muted)));
            ui.label(small(if messages::after_esc(def.intent, style) { "Sent after Esc" } else { "Sent queued, no Esc" }));
        });
        ui.add_space(4.0);
        ui.label(RichText::new(format!("{} lines", def.name)).family(FontFamily::Name("bold".into())));
        let custom = self.cfg.user_lines().is_some();
        let mut remove = None;
        let mut edited = false;
        if custom {
            let lines = self.cfg.custom_lines.get_mut(&self.cfg.whip).expect("custom");
            for (i, l) in lines.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    edited |= ui.add(egui::TextEdit::singleline(l).desired_width(ui.available_width() - 80.0)).lost_focus();
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
        } else {
            for l in def.lines {
                ui.label(RichText::new(format!("• {l}")).color(c(p.muted)));
            }
        }
        if let Some(i) = remove {
            let whip = self.cfg.whip;
            let all = &mut self.cfg.custom_lines;
            // the last line removed: the whip goes back to its own lines
            if all.get_mut(&whip).is_some_and(|lines| {
                lines.remove(i);
                lines.is_empty()
            }) {
                all.remove(&whip);
            }
            edited = true;
        }
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.new_line).hint_text("Add your own line").desired_width(ui.available_width() - 80.0));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.button("Add").clicked() || enter) && !self.new_line.trim().is_empty() {
                let lines = self.cfg.custom_lines.entry(self.cfg.whip).or_insert_with(|| def.lines.iter().map(|s| s.to_string()).collect());
                lines.push(self.new_line.trim().to_string());
                self.new_line.clear();
                edited = true;
            }
        });
        ui.horizontal(|ui| {
            if custom && ui.button("Back to the whip's own lines").clicked() {
                self.cfg.custom_lines.remove(&self.cfg.whip);
                edited = true;
            }
            if ui.button("Reset Claude").on_hover_text(format!("Types “{}”, then “{}”, into the window you were in (ends sticky modes)", messages::RESET, messages::RESUME)).clicked() {
                post(WM_RESET);
            }
            ui.label(small("also switches off always-on modes such as ponytail"));
        });
        if edited {
            self.apply();
        }
    }

    fn hotkey_row(&mut self, ui: &mut egui::Ui, label: &str, which: Rec) {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let r = ui.button(if self.rec == Some(which) { "Cancel" } else { "Record" });
                if r.clicked() {
                    self.rec = if self.rec == Some(which) { None } else { Some(which) };
                    self.hint.clear();
                    r.surrender_focus();
                }
                let text = if self.rec == Some(which) {
                    "Press keys…".to_string()
                } else if which == Rec::Crack {
                    self.cfg.crack_hotkey.clone()
                } else {
                    self.cfg.toggle_hotkey.clone()
                };
                ui.monospace(text);
            });
        });
    }

    fn options_card(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        for (v, label) in [(&mut self.cfg.volume, "Crack volume"), (&mut self.cfg.sfx_volume, "Whip sound volume (trumpet, yeehaw, gong…)")] {
            let r = ui.add(egui::Slider::new(v, 0.0..=1.0).text(label).show_value(false));
            changed |= r.drag_stopped() || (r.changed() && !r.dragged());
        }
        changed |= ui.checkbox(&mut self.cfg.rage, format!("Heat meter and full screen effect (fills in {CRACKS_TO_FULL} cracks)")).changed();
        changed |= ui.checkbox(&mut self.cfg.swing_crack, "Crack when I snap the whip back (wind up one way, snap the other)").changed();
        changed |= ui
            .checkbox(&mut self.cfg.always_esc, "Always press Esc first")
            .on_hover_text("Off: a whip that only nudges Claude is queued without Esc, so nothing Claude is doing is cancelled (the sample under Message says how the current whip is sent). Turn on if queued messages get lost.")
            .changed();
        changed |= ui.checkbox(&mut self.cfg.harder_ultrathink, "HARDER adds ultrathink (one turn at extra-high effort; costs more)").changed();
        if ui.checkbox(&mut self.cfg.autostart, "Start with Windows").changed() {
            match autostart::set(self.cfg.autostart) {
                Ok(()) => changed = true,
                Err(e) => {
                    // Windows keeps the real setting: the box goes back to what will happen.
                    self.cfg.autostart = !self.cfg.autostart;
                    self.status = format!("Couldn't change start-with-Windows: {e}");
                }
            }
        }
        ui.horizontal(|ui| {
            ui.label("Theme");
            let mut t = self.cfg.theme;
            ui.selectable_value(&mut t, Theme::System, "System");
            ui.selectable_value(&mut t, Theme::Light, "Glacier");
            ui.selectable_value(&mut t, Theme::Dark, "Polar Night");
            if t != self.cfg.theme {
                self.cfg.theme = t;
                changed = true;
            }
        });
        egui::CollapsingHeader::new("Keys a crack sends").show(ui, |ui| self.macro_editor(ui));
        if changed {
            self.apply();
        }
    }

    fn macro_editor(&mut self, ui: &mut egui::Ui) {
        let mut act: Option<(usize, i8)> = None;
        for (i, step) in self.cfg.macro_steps.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.monospace(match step {
                    MacroStep::Key(k) => format!("{}. {k}", i + 1),
                    MacroStep::Phrase => format!("{}. {{message}}", i + 1),
                });
                if ui.small_button("↑").clicked() { act = Some((i, -1)); }
                if ui.small_button("↓").clicked() { act = Some((i, 1)); }
                if ui.small_button("Remove").clicked() { act = Some((i, 0)); }
            });
        }
        let mut changed = false;
        if let Some((i, d)) = act {
            let steps = &mut self.cfg.macro_steps;
            match d {
                0 => { steps.remove(i); }
                -1 if i > 0 => steps.swap(i, i - 1),
                1 if i + 1 < steps.len() => steps.swap(i, i + 1),
                _ => {}
            }
            changed = true;
        }
        ui.horizontal(|ui| {
            let recording = self.rec == Some(Rec::Macro);
            let r = ui.button(if recording { "Done" } else { "Record keys" });
            if r.clicked() {
                self.rec = if recording { None } else { Some(Rec::Macro) };
                r.surrender_focus();
            }
            if ui.button("Add {message}").clicked() {
                self.cfg.macro_steps.push(MacroStep::Phrase);
                changed = true;
            }
        });
        if changed {
            self.apply();
        }
    }
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
        let ctx = ui.ctx().clone();
        self.set_visuals(&ctx);
        self.record(&ctx);
        ctx.request_repaint_after(Duration::from_secs(1));
        if config::mtime() != self.cfg_stamp && self.rec.is_none() {
            let (cfg, at) = config::load();
            self.cfg_stamp = at;
            // An unreadable file leaves the window as it is; the engine reports it under Problems.
            if let Ok(cfg) = cfg {
                self.cfg = cfg;
            }
        }
        let stamp = error_stamp();
        if stamp != self.errors_stamp {
            self.errors_stamp = stamp;
            self.errors = read_errors();
        }
        let p = *self.pal();
        egui::Frame::new().fill(c(p.bg)).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                self.header(ui);
                ui.add_space(8.0);
                card(&p, ui, "Whip", |ui| self.whip_picker(ui));
                ui.add_space(8.0);
                card(&p, ui, "Message", |ui| self.messages_card(ui));
                ui.add_space(8.0);
                card(&p, ui, "Hotkeys", |ui| {
                    self.hotkey_row(ui, "Crack", Rec::Crack);
                    self.hotkey_row(ui, "Show / hide the whip", Rec::Toggle);
                    if !self.hint.is_empty() {
                        ui.colored_label(c(0xE0524A), &self.hint);
                    }
                });
                ui.add_space(8.0);
                card(&p, ui, "Options", |ui| self.options_card(ui));
                if !self.status.is_empty() || !self.errors.trim().is_empty() {
                    ui.add_space(8.0);
                    card(&p, ui, "Problems", |ui| {
                        if !self.status.is_empty() {
                            ui.colored_label(c(0xE0524A), &self.status);
                        }
                        if !self.errors.trim().is_empty() {
                            ui.colored_label(c(0xE0524A), self.errors.trim_end());
                            if ui.button("Clear").clicked() {
                                let _ = std::fs::write(app::errors_path(), "");
                                self.errors_stamp = error_stamp();
                                self.errors = read_errors();
                            }
                        }
                    });
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(concat!("NJ-Whip v", env!("CARGO_PKG_VERSION"), " · © 2026 NJ-Burrito · MIT")).size(12.0).color(c(p.muted)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Quit NJ-Whip").clicked() {
                            app::request_quit(ui.ctx());
                        }
                    });
                });
            });
        });
    }
}

/// Thmanyah is the house face: read from the user's fonts folder at runtime (licensed files, never
/// embedded). Missing files fall back to egui's defaults. egui does not shape Arabic.
fn load_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let defaults = fonts.families[&FontFamily::Proportional].clone();
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
    let icon = brand::monogram(64, &POLAR_NIGHT, true);
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([560.0, 760.0])
            .with_min_inner_size([480.0, 520.0])
            .with_icon(egui::IconData { rgba: brand::straight_rgba(&icon), width: 64, height: 64 }),
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
mod tests {
    use super::*;

    #[test]
    fn key_event_names_parse() {
        let m = egui::Modifiers { ctrl: true, alt: true, ..Default::default() };
        assert_eq!(key_event_name(egui::Key::W, m), "Ctrl+Alt+W");
        for k in egui::Key::ALL {
            if macro_key_name(*k).is_some() {
                assert!(keys::parse_hotkey(&key_event_name(*k, m)).is_ok(), "{k:?}");
            }
        }
    }

    #[test]
    fn shift_alone_only_with_f_keys() {
        let s = egui::Modifiers { shift: true, ..Default::default() };
        assert!(shift_only_plain_key(egui::Key::A, s));
        assert!(!shift_only_plain_key(egui::Key::F9, s));
    }

    #[test]
    fn every_whip_has_a_thumbnail() {
        for id in ALL {
            assert!(whip_thumb(id).pixels().iter().any(|p| p.alpha() > 200), "{id:?}");
        }
    }
}
