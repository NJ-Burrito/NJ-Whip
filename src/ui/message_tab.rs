//! The message tab: how messages are worded and the whip's own lines; the hotkey rows and the
//! macro editor of the hotkeys tab.

use super::*;

impl Window {
    /// The message tab: how messages are worded (pack, session), then the whip's own lines.
    pub(super) fn message_tab(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        card(&p, ui, "", |ui| self.wording_card(ui));
        ui.add_space(10.0);
        card(&p, ui, "", |ui| self.lines_card(ui));
    }

    fn wording_card(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let def = whips::def(self.cfg.whip);
        let w = self.words.clone();
        let rtl = self.rtl();
        let small = |s: &str| txs(s, 12.0, c(p.muted));
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.with_layout(widgets::col_layout(rtl), |ui| widgets::caption(ui, &p, w.get("Instruction pack")));
        let (mut pack, mut profile) = (self.cfg.pack, self.cfg.profile);
        for k in Pack::ALL {
            if widgets::choice(ui, &p, rtl, pack == k, k.name(), Some(w.get(k.note()))).clicked() {
                pack = k;
            }
        }
        ui.add_space(6.0);
        ui.with_layout(widgets::col_layout(rtl), |ui| widgets::caption(ui, &p, w.get("Session")));
        let same = tip(&w, ui, "This pack words its messages the same for every session");
        ui.add_enabled_ui(pack.uses_profile(), |ui| {
            for f in [Profile::Light, Profile::Heavy] {
                if widgets::choice(ui, &p, rtl, profile == f, w.get(f.label()), None).clicked() {
                    profile = f;
                }
            }
        })
        .response
        .on_disabled_hover_text(same);
        if (pack, profile) != (self.cfg.pack, self.cfg.profile) {
            (self.cfg.pack, self.cfg.profile) = (pack, profile);
            self.apply();
        }
        let style = self.cfg.style();
        let line = self.cfg.user_lines().map_or(def.lines[0], |l| l[0].as_str());
        let sample = messages::compose(line, def.intent, style);
        ui.add_space(6.0);
        // the sample: what a crack of this whip would type, a bar of the accent beside it
        let sample_rows = rows(&w, ui, &sample, 13.0, ui.available_width() - 44.0);
        egui::Frame::new().fill(c(p.bg)).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(widgets::col_layout(rtl), |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.label(RichText::new(sample_rows).italics().size(13.0).color(c(p.ink)));
                ui.label(small(w.get(if messages::after_esc(def.intent, style) { "Sent after Esc" } else { "Sent queued, no Esc" })));
            });
            let edge = ui.min_rect();
            let x = if rtl { edge.right() + 6.0 } else { edge.left() - 6.0 };
            ui.painter().line_segment([egui::pos2(x, edge.top()), egui::pos2(x, edge.bottom())], Stroke::new(3.0, c(p.accent)));
        });
    }

    fn lines_card(&mut self, ui: &mut egui::Ui) {
        let p = *self.pal();
        let def = whips::def(self.cfg.whip);
        let w = self.words.clone();
        let rtl = self.rtl();
        let align = if rtl { egui::Align2::RIGHT_CENTER } else { egui::Align2::LEFT_CENTER };
        ui.with_layout(widgets::col_layout(rtl), |ui| {
            ui.label(widgets::job_text(&w.fill("{} lines", &[def.name]), 16.0, "bold", c(p.ink)));
        });
        ui.add_space(2.0);
        let custom = self.cfg.user_lines().is_some();
        let mut remove = None;
        let mut edited = false;
        if custom {
            let lines = self.cfg.custom_lines.get_mut(&self.cfg.whip).expect("custom");
            for (i, l) in lines.iter_mut().enumerate() {
                widgets::split_row(
                    ui,
                    rtl,
                    |ui| {
                        let width = (ui.available_width() - 90.0).max(60.0);
                        edited |= ui.add(egui::TextEdit::singleline(l).desired_width(width).align(align)).lost_focus();
                    },
                    |ui| {
                        if ui.small_button(tx(ui, w.get("Remove"))).clicked() {
                            remove = Some(i);
                        }
                    },
                );
            }
        } else {
            ui.scope(|ui| {
                ui.spacing_mut().interact_size.y = 20.0;
                ui.spacing_mut().item_spacing.y = 5.0;
                for l in def.lines {
                    ui.horizontal(|ui| {
                        ui.with_layout(widgets::row_layout(rtl), |ui| {
                            let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                            ui.painter().circle_filled(dot.center(), 2.5, c_a(p.accent, 0.8));
                            ui.label(txs(l, 13.0, c(p.ink)));
                        });
                    });
                }
            });
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
        ui.add_space(4.0);
        let (mut entered, mut clicked) = (false, false);
        let new_line = &mut self.new_line;
        widgets::split_row(
            ui,
            rtl,
            |ui| {
                let width = (ui.available_width() - 90.0).max(60.0);
                let r = ui.add(egui::TextEdit::singleline(new_line).hint_text(tx(ui, w.get("Add your own line"))).desired_width(width).align(align));
                entered = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            },
            |ui| clicked = ui.button(tx(ui, w.get("Add"))).clicked(),
        );
        if (entered || clicked) && !self.new_line.trim().is_empty() {
            let lines = self.cfg.custom_lines.entry(self.cfg.whip).or_insert_with(|| def.lines.iter().map(|s| s.to_string()).collect());
            lines.push(self.new_line.trim().to_string());
            self.new_line.clear();
            edited = true;
        }
        widgets::rule(ui, &p);
        ui.with_layout(widgets::wrap_layout(rtl), |ui| {
            if custom && ui.button(tx(ui, w.get("Back to the whip's own lines"))).clicked() {
                self.cfg.custom_lines.remove(&self.cfg.whip);
                edited = true;
            }
            let what = w.text("Types “{}”, then “{}”, into the window you were in (ends sticky modes)", &[messages::RESET, messages::RESUME]);
            let hover = tip(&w, ui, &what);
            if ui.button(tx(ui, w.get("Reset Claude"))).on_hover_text(hover).clicked() {
                post(WM_RESET);
            }
        });
        ui.with_layout(widgets::col_layout(rtl), |ui| {
            let note = rows(&w, ui, w.get("also switches off always-on modes such as ponytail"), 12.0, ui.available_width());
            ui.label(txs(&note, 12.0, c(p.muted)));
        });
        if edited {
            self.apply();
        }
    }

    pub(super) fn hotkey_row(&mut self, ui: &mut egui::Ui, label: &str, which: Rec) {
        let w = self.words.clone();
        let p = *self.pal();
        let rtl = self.rtl();
        let recording = self.rec == Some(which);
        let keys = if which == Rec::Crack { self.cfg.crack_hotkey.clone() } else { self.cfg.toggle_hotkey.clone() };
        let mut click = false;
        widgets::split_row(
            ui,
            rtl,
            |ui| {
                ui.label(widgets::job_text(w.get(label), 14.0, "bold", c(p.ink)));
            },
            |ui| {
                let r = ui.button(tx(ui, w.get(if recording { "Cancel" } else { "Record" })));
                if r.clicked() {
                    click = true;
                    r.surrender_focus();
                }
                ui.add_space(4.0);
                if recording {
                    ui.label(txs(w.get("Press keys…"), 13.0, c(p.accent)));
                } else {
                    widgets::keycaps(ui, &p, &keys);
                }
            },
        );
        if click {
            self.rec = if recording { None } else { Some(which) };
            self.hint.clear();
        }
    }

    pub(super) fn macro_editor(&mut self, ui: &mut egui::Ui) {
        let w = self.words.clone();
        let p = *self.pal();
        let rtl = self.rtl();
        let mut act: Option<(usize, i8)> = None;
        for (i, step) in self.cfg.macro_steps.iter().enumerate() {
            widgets::split_row(
                ui,
                rtl,
                |ui| {
                    ui.set_min_height(34.0);
                    ui.label(RichText::new(format!("{}", i + 1)).monospace().size(12.0).color(c(p.muted)));
                    ui.add_space(2.0);
                    match step {
                        MacroStep::Key(k) => widgets::keycaps(ui, &p, k),
                        MacroStep::Phrase => widgets::chip(ui, "{message}", c(p.accent), c_a(p.accent, 0.16)),
                    }
                },
                |ui| {
                    if widgets::mini(ui, tx(ui, w.get("Remove"))).clicked() {
                        act = Some((i, 0));
                    }
                    if widgets::mini(ui, "↓").clicked() {
                        act = Some((i, 1));
                    }
                    if widgets::mini(ui, "↑").clicked() {
                        act = Some((i, -1));
                    }
                },
            );
        }
        let mut changed = false;
        if let Some((i, d)) = act {
            let steps = &mut self.cfg.macro_steps;
            match d {
                0 => {
                    steps.remove(i);
                }
                -1 if i > 0 => steps.swap(i, i - 1),
                1 if i + 1 < steps.len() => steps.swap(i, i + 1),
                _ => {}
            }
            changed = true;
        }
        widgets::rule(ui, &p);
        ui.with_layout(widgets::wrap_layout(rtl), |ui| {
            let recording = self.rec == Some(Rec::Macro);
            let r = ui.button(tx(ui, w.get(if recording { "Done" } else { "Record keys" })));
            if r.clicked() {
                self.rec = if recording { None } else { Some(Rec::Macro) };
                r.surrender_focus();
            }
            if ui.button(tx(ui, w.get("Add {message}"))).clicked() {
                self.cfg.macro_steps.push(MacroStep::Phrase);
                changed = true;
            }
        });
        if changed {
            self.apply();
        }
    }
}
