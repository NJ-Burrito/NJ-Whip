//! Colours, paints and small paths shared by everything that draws with tiny-skia.

use tiny_skia::{Color, Paint, Path, PathBuilder};

/// `rgb` is 0xRRGGBB, `a` is 0..1.
pub fn col(rgb: u32, a: f32) -> Color {
    Color::from_rgba8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn paint(rgb: u32, a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(col(rgb, a));
    p.anti_alias = true;
    p
}

/// Build a path from a list of ops: M x y, L x y, Q x1 y1 x y, C x1 y1 x2 y2 x y, Z.
pub fn shape(ops: &[(char, &[f32])]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for (op, a) in ops {
        match op {
            'M' => pb.move_to(a[0], a[1]),
            'L' => pb.line_to(a[0], a[1]),
            'Q' => pb.quad_to(a[0], a[1], a[2], a[3]),
            'C' => pb.cubic_to(a[0], a[1], a[2], a[3], a[4], a[5]),
            _ => pb.close(),
        }
    }
    pb.finish()
}
