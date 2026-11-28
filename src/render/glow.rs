//! Screen-edge glow and the white flash at the whip tip.

use super::*;

pub(super) const GLOW_DEPTH: f32 = 150.0;

/// The glow's strength and each edge's share are drawn in this many steps: a breathing glow
/// repaints the edge strips on a step, not on every frame.
pub(super) const GLOW_STEPS: f32 = 48.0;

pub(super) const FLASH_RADIUS: f32 = 40.0;

/// Where the edge glow sits: `area` shifted by `off` in whole pixels, and how deep it reaches.
pub(super) fn glow_frame(area: Rect, scale: f32, off: V) -> (R, i32) {
    let (dx, dy) = (off.0.round() as i32, off.1.round() as i32);
    let (l, t, r, b) = (area.left().round() as i32 + dx, area.top().round() as i32 + dy, area.right().round() as i32 + dx, area.bottom().round() as i32 + dy);
    ((l, t, r, b), ((GLOW_DEPTH * scale).round() as i32).min((r - l) / 2).min((b - t) / 2))
}

/// The box of one edge's glow (0 top, 1 bottom, 2 left, 3 right). Top and bottom run the full
/// width and hold the corners; no two boxes overlap, so one edge can be repainted alone.
pub(super) fn edge_box(((l, t, r, b), d): (R, i32), e: usize) -> R {
    match e {
        0 => (l, t, r, t + d),
        1 => (l, b - d, r, b),
        2 => (l, t + d, l + d, b - d),
        _ => (r - d, t + d, r, b - d),
    }
}

/// Source-over of one premultiplied pixel.
pub(super) fn over(dst: &mut [u8], src: [u8; 4]) {
    let inv = 255 - src[3] as u32;
    for (d, s) in dst.iter_mut().zip(src) {
        *d = (s as u32 + (*d as u32 * inv + 127) / 255) as u8;
    }
}

/// Ordered-dither thresholds (in 16ths): the glow's alpha is rounded up or down by them, so
/// its 8-bit steps do not draw contour lines on a light window.
pub(super) const DITHER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Paints the glow inside one edge's box, each pixel once. The glow fades inward as
/// closeness^2.5 (closeness 1 at the frame, 0 at full depth: about 1, 0.45, 0.12, 0 at depths
/// 0, 0.25, 0.6, 1, with no step where it ends). In a corner the two edges join in a smooth
/// curve (the 5th powers of their closeness add up), never brighter than an edge on its own
/// and with no crease along the mitre, and their strengths blend, so a strong edge meets a
/// weak one without a seam.
pub(super) fn draw_edge(pm: &mut Pixmap, frame: (R, i32), e: usize, rgb: u32, alpha: f32, edges: [f32; 4]) {
    let ((l, t, r, b), d) = frame;
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let bx = edge_box(frame, e);
    let (x0, y0, x1, y1) = (bx.0.clamp(0, w), bx.1.clamp(0, h), bx.2.clamp(0, w), bx.3.clamp(0, h));
    if d < 1 || x0 >= x1 || y0 >= y1 {
        return;
    }
    let (cr, cg, cb) = ((rgb >> 16) as u8 as f32, (rgb >> 8) as u8 as f32, rgb as u8 as f32);
    // the colour of glow strength `g` (0..1) at pixel (x, y)
    let shade = |g: f32, x: i32, y: i32| -> [u8; 4] {
        let step = (DITHER[(y & 3) as usize][(x & 3) as usize] as f32 + 0.5) / 16.0;
        let a = ((alpha * g).clamp(0.0, 1.0) * 255.0 + step).floor() / 255.0;
        [(cr * a).round() as u8, (cg * a).round() as u8, (cb * a).round() as u8, (255.0 * a).round() as u8]
    };
    // closeness^5 of a pixel `k` px from an edge; the glow is its square root
    let q: Vec<f32> = (0..d).map(|k| (1.0 - (k as f32 + 0.5) / d as f32).powi(5)).collect();
    // away from the corners: `k` px from this box's own edge
    let own = |k: i32| q[k as usize].sqrt() * edges[e];
    // `kx` px from the left or right edge (`sx` its strength), `ky` from this box's own
    let corner = |kx: i32, sx: f32, ky: i32| {
        let (qx, qy) = (q[kx as usize], q[ky as usize]);
        (qx + qy).min(1.0).sqrt() * (qx * sx + qy * edges[e]) / (qx + qy)
    };
    if e >= 2 {
        // a side box has no corner: every row repeats one of four (the dither's rows)
        let rows: [Vec<[u8; 4]>; 4] = std::array::from_fn(|j| (x0..x1).map(|x| shade(own((x - l).min(r - 1 - x)), x, j as i32)).collect());
        for y in y0..y1 {
            for (p, src) in row(pm, y, x0, x1).chunks_exact_mut(4).zip(&rows[(y & 3) as usize]) {
                over(p, *src);
            }
        }
        return;
    }
    // the top or bottom box: a corner at each end, in between one strength (4 dithered pixels, repeated)
    let (a, z) = ((l + d).clamp(x0, x1), (r - d).clamp(x0, x1));
    for y in y0..y1 {
        let ky = if e == 0 { y - t } else { b - 1 - y };
        let between: [[u8; 4]; 4] = std::array::from_fn(|i| shade(own(ky), i as i32, y));
        let (left, rest) = row(pm, y, x0, x1).split_at_mut((a - x0) as usize * 4);
        let (mid, right) = rest.split_at_mut((z - a) as usize * 4);
        for (x, p) in (x0..a).zip(left.chunks_exact_mut(4)) {
            over(p, shade(corner(x - l, edges[2], ky), x, y));
        }
        for (x, p) in (a..z).zip(mid.chunks_exact_mut(4)) {
            over(p, between[(x & 3) as usize]);
        }
        for (x, p) in (z..x1).zip(right.chunks_exact_mut(4)) {
            over(p, shade(corner(r - 1 - x, edges[3], ky), x, y));
        }
    }
}

/// The flash at the tip: white at the centre with a rim in the whip's colour, so it also
/// shows on a light window.
pub(super) fn draw_flash(pm: &mut Pixmap, tip: V, flash: f32, hue: u32) {
    let c = Point::from_xy(tip.0, tip.1);
    let stops = vec![GradientStop::new(0.0, col(0xFFFFFF, 0.8 * flash)), GradientStop::new(0.45, col(hue, 0.45 * flash)), GradientStop::new(1.0, col(hue, 0.0))];
    if let Some(shader) = RadialGradient::new(c, 0.0, c, FLASH_RADIUS, stops, SpreadMode::Pad, Transform::identity()) {
        let mut p = Paint::default();
        p.shader = shader;
        if let Some(path) = PathBuilder::from_circle(tip.0, tip.1, FLASH_RADIUS) {
            pm.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
        }
    }
}

