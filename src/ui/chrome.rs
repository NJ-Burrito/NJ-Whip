//! Window chrome: the ON/OFF header, language switch, tab ribbon, footer and problem list.

use super::*;

impl Window {
    /// The big ON/OFF switch: the knob slides and the track fills, one ease-out motion.
    pub(super) fn switch(&mut self, ui: &mut egui::Ui) {
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
        let state = if self.cfg.enabled { "NJ-Whip is ON: hotkeys and cracks work" } else { "NJ-Whip is OFF: nothing on screen, hotkeys released" };
        let text = tip(&self.words, ui, state);
        resp.on_hover_text(text);
    }

    pub(super) fn header(&mut self, ui: &mut egui::Ui) {
        let w = self.words.clone();
        ui.horizontal(|ui| {
            let logo = &self.logos[self.dark as usize];
            ui.add(egui::Image::new(logo).fit_to_exact_size(Vec2::splat(44.0)));
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(RichText::new("NJ-Whip").size(26.0).family(FontFamily::Name("display".into())));
                let p = self.pal();
                let state = if self.cfg.enabled { w.fill("On · {} shows the whip", &[&self.cfg.toggle_hotkey]) } else { w.get("Off").to_string() };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                    let on = ui.ctx().animate_bool_with_time(egui::Id::unique("state-dot"), self.cfg.enabled, MOTION);
                    ui.painter().circle_filled(dot.center(), 3.5, c(p.muted).lerp_to_gamma(c(p.accent), on));
                    ui.label(txs(&state, 13.0, c(p.muted)));
                });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.switch(ui);
                ui.add_space(8.0);
                self.language(ui);
            });
        });
    }

    /// The language of the words (the layout stays as it is): a pill whose thumb slides to the
    /// chosen side, eased, stretching a little on the way; the tab's content fades in again.
    pub(super) fn language(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let font = egui::FontId::new(13.0, FontFamily::Name("bold".into()));
        let names = [ui.painter().layout_job(job("EN", font.clone(), c(p.ink))), ui.painter().layout_job(job(&self.ar_name, font, c(p.ink)))];
        // each side is a comfortable target (44 px wide at least)
        let cell = Vec2::new((names[0].size().x.max(names[1].size().x) + 22.0).max(44.0), 32.0);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(cell.x * 2.0, cell.y), Sense::click());
        let t = ui.ctx().animate_bool_with_time_and_easing(resp.id, self.cfg.lang == Lang::Ar, MOTION, egui::emath::easing::cubic_out);
        let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), MOTION);
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(16), c(p.selected));
        painter.rect_stroke(rect, CornerRadius::same(16), Stroke::new(1.0, c_a(p.accent, 0.2 + 0.5 * hover)), StrokeKind::Inside);
        let stretch = 5.0 * (std::f32::consts::PI * t).sin();
        let thumb = egui::Rect::from_min_size(rect.min + Vec2::new(cell.x * t, 0.0), cell).shrink(3.0).expand2(Vec2::new(stretch, -stretch * 0.25));
        painter.rect_filled(thumb, CornerRadius::same(13), c_a(p.accent, 0.38));
        for (k, name) in names.into_iter().enumerate() {
            let centre = rect.min + Vec2::new(cell.x * (k as f32 + 0.5), cell.y / 2.0);
            painter.galley(centre - name.size() / 2.0, name, c(p.ink));
        }
        if resp.clicked() {
            let lang = if self.cfg.lang == Lang::En { Lang::Ar } else { Lang::En };
            self.cfg.lang = lang;
            self.words = Rc::new(Words::new(lang));
            self.apply();
            ui.ctx().animate_bool_with_time(self.tab.fade_id(), false, 0.0);
        }
    }

    /// The tab ribbon: the chosen tab's underline grows from its middle.
    pub(super) fn tabs(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let w = self.words.clone();
        let rtl = self.rtl();
        ui.horizontal(|ui| {
          ui.with_layout(widgets::row_layout(rtl), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for tab in Tab::ALL {
                let sel = self.tab == tab;
                let font = egui::FontId::new(14.0, FontFamily::Name("bold".into()));
                let galley = ui.painter().layout_job(job(w.get(tab.name()), font, c(if sel { p.ink } else { p.muted })));
                let (rect, resp) = ui.allocate_exact_size(galley.size() + Vec2::new(24.0, 18.0), Sense::click());
                let on = ui.ctx().animate_bool_with_time(resp.id, sel, MOTION);
                let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), MOTION);
                let painter = ui.painter();
                painter.rect_filled(rect.shrink2(Vec2::new(0.0, 3.0)), CornerRadius::same(9), c(p.selected).gamma_multiply(on).lerp_to_gamma(c_a(p.accent, 0.14), (1.0 - on) * hover));
                painter.galley(rect.center() - galley.size() / 2.0, galley, c(p.ink));
                let half = (rect.width() / 2.0 - 8.0) * on;
                painter.hline(rect.center().x - half..=rect.center().x + half, rect.bottom() - 1.0, Stroke::new(2.0, c_a(p.accent, on)));
                if resp.clicked() && !sel {
                    self.tab = tab;
                    // the new tab's content starts clear and fades in
                    ui.ctx().animate_bool_with_time(tab.fade_id(), false, 0.0);
                }
            }
          });
        });
    }

    pub(super) fn footer(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        ui.horizontal(|ui| {
            ui.label(RichText::new(concat!("NJ-Whip v", env!("CARGO_PKG_VERSION"), " · © 2026 NJ-Burrito · MIT")).size(12.0).color(c(p.muted)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(tx(ui, self.words.get("Quit NJ-Whip"))).clicked() {
                    app::request_quit(ui.ctx());
                }
            });
        });
    }

    pub(super) fn problems(&mut self, ui: &mut egui::Ui) {
        if self.status.is_empty() && self.errors.trim().is_empty() {
            return;
        }
        let p = *self.pal();
        let w = self.words.clone();
        card(&p, ui, w.get("Problems"), |ui| {
            if !self.status.is_empty() {
                ui.label(para_c(&w, ui, &self.status, body_size(ui), c(RED)));
            }
            if !self.errors.trim().is_empty() {
                ui.colored_label(c(RED), self.errors.trim_end());
                if ui.button(tx(ui, w.get("Clear"))).clicked() {
                    let _ = std::fs::write(app::errors_path(), "");
                    self.errors_stamp = error_stamp();
                    self.errors = read_errors();
                }
            }
        });
        ui.add_space(8.0);
    }
}
