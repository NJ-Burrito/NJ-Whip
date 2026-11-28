//! The window's own small widgets: switch rows, choice rows, a segmented control, key caps, a
//! slider row, dividers and chips. Each reads right to left when the window's words do.

use super::*;
use egui::{Align, Layout, Rect, UiBuilder};

/// A row's layout: items start on the reading side.
pub(super) fn row_layout(rtl: bool) -> Layout {
    if rtl { Layout::right_to_left(Align::Center) } else { Layout::left_to_right(Align::Center) }
}

/// A column whose lines sit on the reading side.
pub(super) fn col_layout(rtl: bool) -> Layout {
    Layout::top_down(if rtl { Align::Max } else { Align::Min })
}

/// Content that wraps onto more rows, starting on the reading side.
pub(super) fn wrap_layout(rtl: bool) -> Layout {
    (if rtl { Layout::right_to_left(Align::Min) } else { Layout::left_to_right(Align::Min) }).with_main_wrap(true)
}

/// One row: `start` on the reading side, `end` pushed to the far side.
pub(super) fn split_row(ui: &mut egui::Ui, rtl: bool, start: impl FnOnce(&mut egui::Ui), end: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.with_layout(row_layout(rtl), |ui| {
            start(ui);
            ui.with_layout(row_layout(!rtl), end);
        });
    });
}

/// A hairline between groups inside a card.
pub(super) fn rule(ui: &mut egui::Ui, p: &Palette) {
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c_a(p.line, 0.9));
    ui.add_space(6.0);
}

/// A small group title above a block of controls.
pub(super) fn caption(ui: &mut egui::Ui, p: &Palette, line: &str) {
    ui.label(job_text(line, 13.0, "bold", c(p.muted)));
}

/// A line as widget text in the named face.
pub(super) fn job_text(line: &str, size: f32, family: &str, color: Color32) -> egui::WidgetText {
    job(line, egui::FontId::new(size, FontFamily::Name(family.into())), color).into()
}

/// A small pill with a word in it.
pub(super) fn chip(ui: &mut egui::Ui, line: &str, fg: Color32, bg: Color32) {
    egui::Frame::new().fill(bg).corner_radius(CornerRadius::same(11)).inner_margin(egui::Margin::symmetric(10, 3)).show(ui, |ui| {
        ui.label(job_text(line, 12.0, "bold", fg));
    });
}

/// The accent button: filled, one per area.
pub(super) fn primary(ui: &mut egui::Ui, p: &Palette, dark: bool, line: &str) -> egui::Response {
    let on = if dark { p.bg } else { 0xFFFFFF };
    let button = egui::Button::new(job_text(line, 14.0, "bold", c(on))).fill(c(p.accent)).stroke(Stroke::NONE).corner_radius(CornerRadius::same(10)).min_size(Vec2::new(0.0, 32.0));
    ui.add(button)
}

/// A tick row: the words on the reading side, a switch on the far one; the whole row is the target.
pub(super) fn toggle(ui: &mut egui::Ui, w: &Words, p: &Palette, rtl: bool, on: &mut bool, line: &str) -> egui::Response {
    let sw = Vec2::new(40.0, 22.0);
    let width = ui.available_width();
    let size = body_size(ui);
    let text = rows(w, ui, line, size, width - sw.x - 28.0);
    let measure = ui.painter().layout_job(job(&text, egui::FontId::proportional(size), Color32::WHITE));
    let h = measure.size().y.max(sw.y) + 14.0;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(width, h), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, MOTION);
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), MOTION);
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(10), c_a(p.accent, 0.08 * hover));
    let cx = if rtl { rect.left() + 10.0 + sw.x / 2.0 } else { rect.right() - 10.0 - sw.x / 2.0 };
    let track = Rect::from_center_size(egui::pos2(cx, rect.center().y), sw);
    let fill = c(p.line).lerp_to_gamma(c(p.accent), t);
    painter.rect_filled(track, CornerRadius::same(11), fill);
    let kx = track.left() + 11.0 + (sw.x - 22.0) * t;
    painter.circle_filled(egui::pos2(kx, track.center().y + 1.0), 8.5, Color32::from_black_alpha(45));
    painter.circle_filled(egui::pos2(kx, track.center().y), 8.5, c(if t > 0.5 { p.bg } else { p.panel }));
    let text_rect = if rtl {
        Rect::from_min_max(egui::pos2(rect.left() + sw.x + 22.0, rect.top()), egui::pos2(rect.right() - 10.0, rect.bottom()))
    } else {
        Rect::from_min_max(egui::pos2(rect.left() + 10.0, rect.top()), egui::pos2(rect.right() - sw.x - 22.0, rect.bottom()))
    };
    ui.scope_builder(UiBuilder::new().max_rect(text_rect).layout(col_layout(rtl)), |ui| {
        ui.add_space(((text_rect.height() - measure.size().y) / 2.0).max(0.0));
        ui.label(txs(&text, size, c(p.ink)));
    });
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp
}

/// A slider on a row of its own, its name on the reading side and its value on the far side.
pub(super) fn slider_row(ui: &mut egui::Ui, p: &Palette, rtl: bool, value: &mut f32, line: &str) -> egui::Response {
    split_row(
        ui,
        rtl,
        |ui| {
            ui.label(txs(line, body_size(ui), c(p.ink)));
        },
        |ui| {
            ui.label(job_text(&format!("{}%", (*value * 100.0).round() as i32), 13.0, "bold", c(p.accent)));
        },
    );
    ui.spacing_mut().slider_width = ui.available_width() - 6.0;
    let r = ui.add(egui::Slider::new(value, 0.0..=1.0).show_value(false));
    ui.add_space(6.0);
    r
}

/// A pill of choices, the chosen one carried by a thumb that slides to it.
pub(super) fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, p: &Palette, rtl: bool, cur: &mut T, items: &[(T, String)]) -> bool {
    let font = egui::FontId::new(13.0, FontFamily::Name("bold".into()));
    let galleys: Vec<_> = items.iter().map(|(_, s)| ui.painter().layout_job(job(s, font.clone(), c(p.ink)))).collect();
    let cell = Vec2::new(galleys.iter().map(|g| g.size().x).fold(56.0, f32::max) + 26.0, 32.0);
    let n = items.len();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(cell.x * n as f32, cell.y), Sense::click());
    let slot = |k: usize| if rtl { n - 1 - k } else { k };
    let at = items.iter().position(|(v, _)| v == cur).unwrap_or(0);
    let t = ui.ctx().animate_value_with_time(resp.id, slot(at) as f32, MOTION);
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(16), c(p.selected));
    painter.rect_stroke(rect, CornerRadius::same(16), Stroke::new(1.0, c_a(p.accent, 0.25)), StrokeKind::Inside);
    let thumb = Rect::from_min_size(rect.min + Vec2::new(cell.x * t, 0.0), cell).shrink(3.0);
    painter.rect_filled(thumb, CornerRadius::same(13), c_a(p.accent, 0.38));
    let mut changed = false;
    for (k, g) in galleys.into_iter().enumerate() {
        let centre = rect.min + Vec2::new(cell.x * (slot(k) as f32 + 0.5), cell.y / 2.0);
        painter.galley(centre - g.size() / 2.0, g, c(p.ink));
    }
    if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
        let col = (((pos.x - rect.left()) / cell.x) as usize).min(n - 1);
        let k = if rtl { n - 1 - col } else { col };
        if items[k].0 != *cur {
            *cur = items[k].0;
            changed = true;
        }
    }
    changed
}

/// One choice among several, as a row: a dot, a title and (if any) a note under it.
pub(super) fn choice(ui: &mut egui::Ui, p: &Palette, rtl: bool, selected: bool, title: &str, note: Option<&str>) -> egui::Response {
    let on = ui.is_enabled();
    let dim = if on { 1.0 } else { 0.4 };
    let h = if note.is_some() { 48.0 } else { 38.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered() && on, MOTION);
    let sel = ui.ctx().animate_bool_with_time(resp.id.with("s"), selected, MOTION);
    let painter = ui.painter();
    let fill = c_a(p.accent, 0.07 * hover + 0.0).lerp_to_gamma(c(p.selected), sel);
    painter.rect_filled(rect, CornerRadius::same(10), fill.gamma_multiply(if sel > 0.0 || hover > 0.0 { dim } else { 0.0 }));
    painter.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0 + 0.5 * sel, c_a(p.accent, (0.12 + 0.6 * sel) * dim)), StrokeKind::Inside);
    let dot = egui::pos2(if rtl { rect.right() - 20.0 } else { rect.left() + 20.0 }, rect.center().y);
    painter.circle_stroke(dot, 8.0, Stroke::new(1.5, c_a(if selected { p.accent } else { p.muted }, dim * if selected { 1.0 } else { 0.7 })));
    if sel > 0.0 {
        painter.circle_filled(dot, 4.5 * sel, c_a(p.accent, dim));
    }
    let tg = painter.layout_job(job(title, egui::FontId::new(14.0, FontFamily::Name("bold".into())), c_a(p.ink, dim)));
    let ng = note.map(|n| painter.layout_job(job(n, egui::FontId::proportional(12.0), c_a(p.muted, dim))));
    let total = tg.size().y + ng.as_ref().map_or(0.0, |g| g.size().y + 1.0);
    let top = rect.center().y - total / 2.0;
    let x = |wd: f32| if rtl { rect.right() - 40.0 - wd } else { rect.left() + 40.0 };
    let tw = tg.size().x;
    let th = tg.size().y;
    painter.galley(egui::pos2(x(tw), top), tg, c(p.ink));
    if let Some(g) = ng {
        let gw = g.size().x;
        painter.galley(egui::pos2(x(gw), top + th + 1.0), g, c(p.muted));
    }
    resp
}

/// A hotkey as key caps joined by plus signs.
pub(super) fn keycaps(ui: &mut egui::Ui, p: &Palette, text: &str) {
    let spacing = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = 4.0;
    // a hotkey reads left to right, whichever way the row it sits in runs
    let mut parts: Vec<&str> = text.split('+').collect();
    if ui.layout().prefer_right_to_left() {
        parts.reverse();
    }
    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            ui.label(txs("+", 12.0, c(p.muted)));
        }
        egui::Frame::new()
            .fill(c(p.selected))
            .stroke(Stroke::new(1.0, c(p.line)))
            .corner_radius(CornerRadius::same(7))
            .inner_margin(egui::Margin::symmetric(9, 4))
            .show(ui, |ui| {
                ui.label(RichText::new(part).monospace().size(13.0).color(c(p.ink)));
            });
    }
    ui.spacing_mut().item_spacing.x = spacing;
}

/// A small square-ish button for an arrow or a short word, all of one height.
pub(super) fn mini(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Button::new(text).min_size(Vec2::new(30.0, 26.0)))
}
