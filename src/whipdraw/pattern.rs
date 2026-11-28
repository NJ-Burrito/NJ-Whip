//! The lash pattern: braid, rings, ribbon, chunks.

use super::*;

pub(super) fn draw_pattern(pm: &mut Pixmap, pen: &mut Pen, pts: &[V], l: &Look) {
    let n = pts.len() - 1;
    let id = Transform::identity();
    // Braid marks a pixel wide or less are hairlines, which tiny-skia draws segment by segment:
    // every run of them at one width goes down in one call instead of one call each.
    let braid = paint(l.mark, 0.8);
    let mut thin = (0.0, PathBuilder::new());
    let flush = |pm: &mut Pixmap, pen: &mut Pen, thin: &mut (f32, PathBuilder)| {
        if let Some(path) = std::mem::replace(&mut thin.1, PathBuilder::new()).finish() {
            pen.stroke_path(pm, &path, &braid, &stroke(thin.0), id);
            thin.1 = path.clear();
        }
    };
    for i in 0..n {
        let ww = width_at(l, i as f32 / n as f32);
        let (a, b) = (pts[i], pts[i + 1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx.hypot(dy).max(1e-3);
        let (nx, ny) = (-dy / len, dx / len);
        let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        match l.pattern {
            Pattern::Braid if ww > 2.6 => {
                let e = ww * 0.4;
                let sw = (ww * 0.11).max(0.9);
                let hair = is_hairline(sw, id);
                if !hair || sw != thin.0 {
                    flush(pm, pen, &mut thin);
                    thin.0 = sw;
                }
                for from in [(a.0 + nx * e, a.1 + ny * e), (a.0 - nx * e, a.1 - ny * e)] {
                    if hair {
                        thin.1.move_to(from.0, from.1);
                        thin.1.line_to(m.0, m.1);
                    } else {
                        line(pm, pen, from, m, l.mark, 0.8, sw, id);
                    }
                }
            }
            Pattern::Rings if i % 2 == 0 && ww > 4.0 => {
                line(pm, pen, (m.0 - nx * ww * 0.45, m.1 - ny * ww * 0.45), (m.0 + nx * ww * 0.45, m.1 + ny * ww * 0.45), l.mark, 1.0, 1.5, id);
            }
            Pattern::Ribbon if ww > 3.0 => {
                let c = if i % 2 == 1 { l.mark } else { l.mark2 };
                line(pm, pen, (m.0 - nx * ww * 0.4, m.1 - ny * ww * 0.4), (m.0 + nx * ww * 0.4 + dx * 0.5, m.1 + ny * ww * 0.4 + dy * 0.5), c, 1.0, ww * 0.42, id);
            }
            Pattern::Chunks if i % 3 == 1 && ww > 3.0 => {
                let c = if i % 6 == 1 { l.mark } else { l.mark2 };
                let tf = Transform::from_translate(m.0, m.1).pre_rotate(i as f32 * 37.0);
                if let Some(r) = tiny_skia::Rect::from_xywh(-ww * 0.18, -ww * 0.15, ww * 0.36, ww * 0.3) {
                    tick();
                    pm.fill_rect(r, &paint(c, 1.0), tf, None);
                }
            }
            _ => {}
        }
    }
    flush(pm, pen, &mut thin);
}

