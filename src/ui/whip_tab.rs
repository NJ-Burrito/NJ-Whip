//! The whip tab: whip picker, crack sounds, own sound, tip picture.

use super::*;

impl Window {
    /// The whip tab: the chosen whip up front, the whole range under it, then the tip and sounds.
    pub(super) fn whip_tab(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        card(&p, ui, "", |ui| self.whip_hero(ui));
        ui.add_space(10.0);
        card(&p, ui, "", |ui| self.whip_picker(ui));
        ui.add_space(10.0);
        card(&p, ui, "", |ui| {
            self.tip_picture(ui);
            widgets::rule(ui, &p);
            self.crack_sounds(ui);
            widgets::rule(ui, &p);
            self.own_sound(ui);
        });
    }

    /// The chosen whip large on a lit stage, its name, what it does and the test button.
    fn whip_hero(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let words = self.words.clone();
        let rtl = self.rtl();
        let def = whips::def(self.cfg.whip);
        let k = ALL.iter().position(|&id| id == self.cfg.whip).unwrap_or(0);
        let dark = self.dark;
        ui.horizontal(|ui| {
            ui.with_layout(widgets::row_layout(rtl), |ui| {
                let size = Vec2::new(208.0, 118.0);
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                ui.painter().rect_filled(rect, CornerRadius::same(12), c(p.bg));
                stage(ui, &p, rect, 1.0);
                let img = egui::Rect::from_center_size(rect.center() + Vec2::new(0.0, 4.0), Vec2::new(size.x - 16.0, (size.x - 16.0) * 170.0 / 300.0));
                egui::Image::new(&self.thumbs[k]).paint_at(ui, img);
                ui.add_space(8.0);
                ui.with_layout(widgets::col_layout(rtl), |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.label(RichText::new(def.name).size(24.0).family(FontFamily::Name("display".into())));
                    widgets::chip(ui, words.get(def.intent.name()), c(p.accent), c_a(p.accent, 0.16));
                    ui.label(para_c(&words, ui, def.blurb, body_size(ui), c(p.muted)));
                    ui.add_space(2.0);
                    let hover = tip(&words, ui, "Plays the crack and its effect; types nothing");
                    if widgets::primary(ui, &p, dark, words.get("Test crack")).on_hover_text(hover).clicked() {
                        post(WM_TEST);
                    }
                    ui.label(para_c(&words, ui, "Pick a whip to hear its sound.", 12.0, c(p.muted)));
                });
            });
        });
    }

    pub(super) fn whip_picker(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let cols = 3;
        let gap = 10.0;
        let w = ((ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32).floor();
        let h = (w - 12.0) * 170.0 / 300.0 + 40.0;
        let mut pick = None;
        egui::Grid::new("whips").spacing(Vec2::splat(gap)).show(ui, |ui| {
            for (k, &id) in ALL.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
                let sel = self.cfg.whip == id;
                let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), MOTION);
                let on = ui.ctx().animate_bool_with_time(resp.id.with("s"), sel, MOTION);
                let painter = ui.painter().clone();
                let radius = CornerRadius::same(12);
                painter.rect_filled(rect, radius, c(p.bg).lerp_to_gamma(c(p.selected), on));
                stage(ui, &p, rect, 0.6 + 0.8 * on + 0.4 * hover);
                let stroke = Stroke::new(1.0 + on, c_a(p.accent, 0.14 + 0.6 * hover.max(on)));
                painter.rect_stroke(rect, radius, stroke, StrokeKind::Inside);
                let iw = w - 12.0;
                let img_rect = egui::Rect::from_min_size(rect.min + Vec2::new(6.0, 6.0 - 3.0 * hover), Vec2::new(iw, iw * 170.0 / 300.0));
                egui::Image::new(&self.thumbs[k]).paint_at(ui, img_rect);
                let font = egui::FontId::new(13.0, FontFamily::Name("bold".into()));
                painter.text(egui::pos2(rect.center().x, rect.bottom() - 15.0), egui::Align2::CENTER_CENTER, whips::def(id).name, font, if sel { c(p.accent) } else { c(p.ink) });
                if on > 0.0 {
                    // a small tick in a disc, top corner
                    let at = egui::pos2(rect.right() - 14.0, rect.top() + 14.0);
                    painter.circle_filled(at, 8.0 * on, c(p.accent));
                    let tick = c(if self.dark { p.bg } else { 0xFFFFFF });
                    let s = Stroke::new(1.8, tick.gamma_multiply(on));
                    painter.line_segment([at + Vec2::new(-3.5, 0.2), at + Vec2::new(-1.0, 2.8)], s);
                    painter.line_segment([at + Vec2::new(-1.0, 2.8), at + Vec2::new(3.6, -2.6)], s);
                }
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
            // Picking a whip plays its sound (the engine has applied the pick by then).
            post(WM_SOUND);
        }
    }

    /// The tip picture: the user's own picture or GIF in place of the whip's tip.
    fn tip_picture(&mut self, ui: &mut egui::Ui) {
        let words = self.words.clone();
        let rtl = self.rtl();
        ui.with_layout(widgets::wrap_layout(rtl), |ui| {
            let has_tip = self.cfg.custom_tips.contains_key(&self.cfg.whip);
            if ui.button(tx(ui, words.get(if has_tip { "Change tip picture or GIF…" } else { "Use my picture or GIF as the tip…" }))).clicked() {
                self.pick_tip();
            }
            if has_tip && ui.button(tx(ui, words.get("Remove picture"))).clicked() {
                self.cfg.custom_tips.remove(&self.cfg.whip);
                self.apply();
            }
        });
    }

    /// A preview of one clip by hand, at the volumes now set.
    pub(super) fn play_clip(&self, clip: Clip, extra: bool) {
        self.sound.set_volume(self.cfg.volume, self.cfg.sfx_volume);
        self.sound.play(clip, extra);
    }

    /// Every crack sound we have, to tick for the current whip (one of the ticked is played at
    /// random per crack; none ticked = the whip's own).
    pub(super) fn crack_sounds(&mut self, ui: &mut egui::Ui) {
        let w = self.words.clone();
        let p = *self.pal();
        let def = whips::def(self.cfg.whip);
        let picked = self.cfg.custom_cracks.get(&def.id).map(Vec::as_slice);
        let mut names: Vec<&'static str> = sound::chosen_cracks(def, picked).iter().map(|s| s.name).collect();
        let custom = self.cfg.custom_cracks.contains_key(&def.id);
        let (mut changed, mut reset) = (false, false);
        let rtl = self.rtl();
        egui::CollapsingHeader::new(widgets::job_text(w.get("Crack sounds"), 15.0, "bold", Color32::PLACEHOLDER)).id_salt("crack-sounds").default_open(cfg!(test)).show(ui, |ui| {
            ui.with_layout(widgets::col_layout(rtl), |ui| {
                ui.label(para_c(&w, ui, "Tick the cracks this whip plays. One of them is chosen at random per crack.", 12.0, c(p.muted)));
            });
            ui.spacing_mut().interact_size.y = 24.0;
            ui.spacing_mut().item_spacing.y = 4.0;
            for s in sound::CRACK_SOUNDS {
                let mut play = false;
                widgets::split_row(
                    ui,
                    rtl,
                    |ui| {
                        let mut on = names.contains(&s.name);
                        // the last tick stays: no tick at all would mean the whip's own cracks
                        let last = on && names.len() == 1;
                        if ui.add_enabled(!last, egui::Checkbox::new(&mut on, s.label)).changed() {
                            if on { names.push(s.name) } else { names.retain(|n| *n != s.name) }
                            changed = true;
                        }
                    },
                    |ui| play = ui.small_button(tx(ui, w.get("Play"))).clicked(),
                );
                if play {
                    self.play_clip(Clip::Static(s.bytes), false);
                }
            }
            ui.with_layout(widgets::col_layout(rtl), |ui| {
                reset = ui.add_enabled(custom, egui::Button::new(tx(ui, w.get("Reset to default")))).clicked();
            });
        });
        if reset {
            self.cfg.custom_cracks.remove(&def.id);
            self.apply();
        } else if changed {
            let mut own: Vec<_> = sound::default_cracks(def).iter().map(|s| s.name).collect();
            let mut now = names.clone();
            (own.sort(), now.sort());
            if own == now {
                self.cfg.custom_cracks.remove(&def.id);
            } else {
                self.cfg.custom_cracks.insert(def.id, names.iter().map(|n| n.to_string()).collect());
            }
            self.apply();
        }
    }

    /// The user's own sound for the current whip (at most 3 s), played in place of the whip's
    /// built-in extra sound.
    pub(super) fn own_sound(&mut self, ui: &mut egui::Ui) {
        let w = self.words.clone();
        let p = *self.pal();
        let id = self.cfg.whip;
        let path = self.cfg.custom_sfx.get(&id).cloned();
        let rtl = self.rtl();
        ui.with_layout(widgets::col_layout(rtl), |ui| {
            ui.label(widgets::job_text(w.get("Special sound"), 15.0, "bold", Color32::PLACEHOLDER));
            ui.label(para_c(&w, ui, "Plays on top of the crack, in place of this whip's own sound. Up to 3 seconds: mp3, wav, ogg or flac.", 12.0, c(p.muted)));
        });
        ui.with_layout(widgets::wrap_layout(rtl), |ui| {
            if ui.button(tx(ui, w.get(if path.is_some() { "Change my sound…" } else { "Use my sound…" }))).clicked() {
                self.pick_sfx();
            }
            if let Some(path) = &path {
                if ui.button(tx(ui, w.get("Play"))).clicked() {
                    if let Some(clip) = sound::load_sfx(path) {
                        self.play_clip(clip, true);
                    }
                }
                if ui.button(tx(ui, w.get("Remove sound"))).clicked() {
                    self.cfg.custom_sfx.remove(&id);
                    sound::remove_sfx(&sound::sfx_dir(), &format!("{id:?}").to_lowercase());
                    self.apply();
                }
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    ui.label(txs(name, 12.0, c(p.muted)));
                }
            }
        });
    }

    pub(super) fn pick_sfx(&mut self) {
        let Some(src) = rfd::FileDialog::new().add_filter("Sound", &sound::SFX_EXTENSIONS).pick_file() else { return };
        let name = format!("{:?}", self.cfg.whip).to_lowercase();
        match sound::import_sfx(&src, &sound::sfx_dir(), &name) {
            Ok(dest) => {
                self.cfg.custom_sfx.insert(self.cfg.whip, dest);
                self.apply();
            }
            Err(e) => self.status = self.words.text(e.line(), &[]),
        }
    }

    pub(super) fn pick_tip(&mut self) {
        let Some(src) = rfd::FileDialog::new().add_filter("Pictures and GIFs", &["png", "jpg", "jpeg", "gif", "webp", "bmp"]).pick_file() else { return };
        let name = format!("{:?}", self.cfg.whip).to_lowercase();
        match tipimg::import(&src, &name) {
            Ok(dest) => {
                self.cfg.custom_tips.insert(self.cfg.whip, dest);
                self.apply();
            }
            Err(e) => self.status = e,
        }
    }
}

/// A soft light behind a whip: a few wide ellipses of the accent, kept inside `rect`.
fn stage(ui: &egui::Ui, p: &Palette, rect: egui::Rect, strength: f32) {
    let painter = ui.painter().with_clip_rect(rect);
    let centre = rect.center() + Vec2::new(0.0, rect.height() * 0.04);
    for i in 0..8 {
        let f = 1.0 - i as f32 / 8.0;
        let radius = Vec2::new(rect.width() * 0.48 * f, rect.height() * 0.46 * f);
        painter.add(egui::Shape::ellipse_filled(centre, radius, c_a(p.accent, 0.011 * strength)));
    }
}
