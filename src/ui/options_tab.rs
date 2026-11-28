//! The options and about tabs: options, about, updates.

use super::*;

impl Window {
    /// The options tab: sound, behaviour, look; each its own card.
    pub(super) fn options_tab(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let w = self.words.clone();
        let rtl = self.rtl();
        let mut changed = false;
        card(&p, ui, "", |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for (v, label) in [(&mut self.cfg.volume, "Crack volume"), (&mut self.cfg.sfx_volume, "Whip sound volume (trumpet, yeehaw, gong…)")] {
                let r = widgets::slider_row(ui, &p, rtl, v, w.get(label));
                changed |= r.drag_stopped() || (r.changed() && !r.dragged());
            }
        });
        ui.add_space(10.0);
        card(&p, ui, "", |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let meter = w.text("Heat meter and full screen effect (fills in {} cracks)", &[&CRACKS_TO_FULL.to_string()]);
            changed |= widgets::toggle(ui, &w, &p, rtl, &mut self.cfg.rage, &meter).changed();
            changed |= widgets::toggle(ui, &w, &p, rtl, &mut self.cfg.swing_crack, w.get("Crack when I snap the whip back (wind up one way, snap the other)")).changed();
            let esc = tip(&w, ui, "Off: a whip that only nudges Claude is queued without Esc, so nothing Claude is doing is cancelled (the sample under Message says how the current whip is sent). Turn on if queued messages get lost.");
            changed |= widgets::toggle(ui, &w, &p, rtl, &mut self.cfg.always_esc, w.get("Always press Esc first")).on_hover_text(esc).changed();
            changed |= widgets::toggle(ui, &w, &p, rtl, &mut self.cfg.harder_ultrathink, w.get("HARDER adds ultrathink (one turn at extra-high effort; costs more)")).changed();
            if widgets::toggle(ui, &w, &p, rtl, &mut self.cfg.autostart, w.get("Start with Windows")).changed() {
                match autostart::set(self.cfg.autostart) {
                    Ok(()) => changed = true,
                    Err(e) => {
                        // Windows keeps the real setting: the switch goes back to what will happen.
                        self.cfg.autostart = !self.cfg.autostart;
                        self.status = w.text("Couldn't change start-with-Windows: {}", &[&e.to_string()]);
                    }
                }
            }
        });
        ui.add_space(10.0);
        card(&p, ui, "", |ui| {
            let mut t = self.cfg.theme;
            let items = [(Theme::System, w.get("System").to_string()), (Theme::Light, "Glacier".to_string()), (Theme::Dark, "Polar Night".to_string())];
            widgets::split_row(
                ui,
                rtl,
                |ui| {
                    ui.label(widgets::job_text(w.get("Theme"), 14.0, "bold", c(p.ink)));
                },
                |ui| {
                    widgets::segmented(ui, &p, rtl, &mut t, &items);
                },
            );
            if t != self.cfg.theme {
                self.cfg.theme = t;
                changed = true;
            }
        });
        if changed {
            self.apply();
        }
    }

    pub(super) fn about_card(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let w = self.words.clone();
        let rtl = self.rtl();
        ui.horizontal(|ui| {
            ui.with_layout(widgets::row_layout(rtl), |ui| {
                ui.add(egui::Image::new(&self.logos[self.dark as usize]).fit_to_exact_size(Vec2::splat(60.0)));
                ui.with_layout(widgets::col_layout(rtl), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new("NJ-Whip").size(26.0).family(FontFamily::Name("display".into())));
                    ui.label(txs(&w.fill("Version {}", &[env!("CARGO_PKG_VERSION")]), body_size(ui), c(p.accent)));
                });
            });
        });
        ui.add_space(6.0);
        ui.with_layout(widgets::col_layout(rtl), |ui| {
            ui.label(para(&w, ui, "An on-screen whip for Claude Code. Swing it, it cracks, and Claude gets one short instruction."));
            ui.label(para_c(&w, ui, "Made by Nader Alzghoul (NJ-Burrito).", body_size(ui), c(p.muted)));
            ui.label(para_c(&w, ui, "Free and open source under the MIT licence.", body_size(ui), c(p.muted)));
        });
        widgets::rule(ui, &p);
        ui.horizontal(|ui| {
            ui.with_layout(widgets::row_layout(rtl), |ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                for (name, url) in [("Website", SITE), ("Source code", SOURCE), ("Sound credits", CREDITS), ("More tools", MORE_TOOLS)] {
                    ui.hyperlink_to(tx(ui, w.get(name)), url);
                }
            });
        });
        widgets::rule(ui, &p);
        let note = "An independent project, not affiliated with Anthropic. “Claude” and “Claude Code” are trademarks of Anthropic.";
        ui.with_layout(widgets::col_layout(rtl), |ui| {
            ui.label(para_c(&w, ui, note, 12.0, c(p.muted)));
        });
    }

    pub(super) fn updates_card(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let w = self.words.clone();
        let state = self.update.lock().expect("update").clone();
        let busy = |ui: &mut egui::Ui, line: &str| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tx(ui, w.get(line)));
            });
        };
        match &state {
            Update::Checking => busy(ui, "Checking…"),
            Update::Installing => busy(ui, "Downloading…"),
            Update::Restarting => {
                // The new copy is started and waits for this one to close.
                busy(ui, "Restarting…");
                app::request_quit(ui.ctx());
            }
            _ => {
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tx(ui, w.get("Check for updates"))).clicked() {
                        self.run_update(ui.ctx(), None);
                    }
                    if let Update::Available(v) = state {
                        if ui.button(tx(ui, w.get("Install and restart"))).clicked() {
                            self.run_update(ui.ctx(), Some(v));
                        }
                    }
                });
                match &state {
                    Update::Latest => drop(ui.label(tx(ui, w.get("You have the latest version.")))),
                    Update::Available(v) => drop(ui.label(txs(&w.fill("Version {} is available.", &[&update::dotted(*v)]), body_size(ui), c(p.accent)))),
                    Update::Failed(what, why) => drop(ui.label(para_c(&w, ui, &w.text(what, &[why]), body_size(ui), c(RED)))),
                    _ => {}
                }
            }
        }
        ui.label(para_c(&w, ui, "NJ-Whip goes online only when you press this button.", 12.0, c(p.muted)));
    }

    /// Checks for a newer release, or (given its version) installs it and starts it. Runs on its
    /// own thread: the window keeps drawing and is repainted when the answer is in.
    pub(super) fn run_update(&self, ctx: &egui::Context, install: Option<update::Version>) {
        let (slot, ctx) = (self.update.clone(), ctx.clone());
        *slot.lock().expect("update") = if install.is_some() { Update::Installing } else { Update::Checking };
        std::thread::spawn(move || {
            let next = match install {
                None => match update::check() {
                    Ok(Some(v)) => Update::Available(v),
                    Ok(None) => Update::Latest,
                    Err(e) => Update::Failed("Couldn't check: {}", e),
                },
                Some(v) => {
                    let done = std::env::current_exe().map_err(|e| e.to_string()).and_then(|exe| {
                        update::install(v, &exe)?;
                        update::restart(&exe)
                    });
                    match done {
                        Ok(()) => Update::Restarting,
                        Err(e) => Update::Failed("Couldn't install: {}", e),
                    }
                }
            };
            *slot.lock().expect("update") = next;
            ctx.request_repaint();
        });
    }
}
