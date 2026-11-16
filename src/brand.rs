//! The NJ monogram (N + a J whose top unfurls into a lash) and the two themes' colours.

use crate::ink::{paint, shape};
use tiny_skia::{LineCap, LineJoin, PathBuilder, Pixmap, Stroke, Transform};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub bg: u32,
    pub panel: u32,
    pub selected: u32,
    pub ink: u32,
    pub muted: u32,
    pub accent: u32,
    pub accent2: u32,
    pub line: u32,
}

pub const GLACIER: Palette = Palette {
    bg: 0xEEF6FB, panel: 0xFFFFFF, selected: 0xDDEEF9, ink: 0x0B2540, muted: 0x4A6178,
    accent: 0x1E7FD8, accent2: 0x57D3F2, line: 0xD3E3EF,
};
pub const POLAR_NIGHT: Palette = Palette {
    bg: 0x0A1024, panel: 0x121B38, selected: 0x1A2750, ink: 0xF2F7FF, muted: 0xA9B8D6,
    accent: 0x8FDCFF, accent2: 0xFFFFFF, line: 0x24305A,
};

fn stroke_path(pm: &mut Pixmap, d: &[(char, &[f32])], rgb: u32, w: f32, tf: Transform) {
    if let Some(p) = shape(d) {
        let st = Stroke { width: w, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
        pm.stroke_path(&p, &paint(rgb, 1.0), &st, tf, None);
    }
}

/// The monogram on a rounded tile, `size` px square. Below 20 px only the J-lash is drawn
/// (the N stops reading at that size).
pub fn monogram(size: u32, p: &Palette, tile: bool) -> Pixmap {
    let mut pm = Pixmap::new(size, size).expect("non-zero size");
    let s = size as f32 / 128.0;
    if tile {
        let mut pb = PathBuilder::new();
        let r = 28.0 * s;
        let w = size as f32;
        pb.move_to(r, 0.0);
        pb.line_to(w - r, 0.0);
        pb.quad_to(w, 0.0, w, r);
        pb.line_to(w, w - r);
        pb.quad_to(w, w, w - r, w);
        pb.line_to(r, w);
        pb.quad_to(0.0, w, 0.0, w - r);
        pb.line_to(0.0, r);
        pb.quad_to(0.0, 0.0, r, 0.0);
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(p.selected, 1.0), tiny_skia::FillRule::Winding, Transform::identity(), None);
        }
    }
    let tf = Transform::from_scale(s, s).pre_translate(4.0, 4.0);
    if size >= 20 {
        stroke_path(&mut pm, &[('M', &[22.0, 88.0]), ('L', &[22.0, 34.0]), ('L', &[54.0, 86.0]), ('L', &[54.0, 34.0])], p.ink, 10.0, tf);
        stroke_path(&mut pm, &[('M', &[80.0, 50.0]), ('L', &[80.0, 76.0]), ('Q', &[80.0, 94.0, 62.0, 94.0]), ('Q', &[52.0, 94.0, 47.0, 88.0])], p.accent, 10.0, tf);
        stroke_path(&mut pm, &[('M', &[80.0, 50.0]), ('C', &[80.0, 30.0, 92.0, 20.0, 108.0, 22.0])], p.accent, 7.0, tf);
        stroke_path(&mut pm, &[('M', &[106.0, 22.0]), ('C', &[111.0, 23.0, 114.0, 25.0, 116.0, 28.0])], p.accent2, 3.0, tf);
    } else {
        stroke_path(&mut pm, &[('M', &[76.0, 46.0]), ('L', &[76.0, 74.0]), ('Q', &[76.0, 98.0, 50.0, 98.0])], p.accent, 20.0, tf);
        stroke_path(&mut pm, &[('M', &[76.0, 46.0]), ('C', &[76.0, 24.0, 90.0, 14.0, 108.0, 18.0])], p.accent, 13.0, tf);
    }
    pm
}

/// Straight (not premultiplied) RGBA bytes, as tray icons and egui images want.
pub fn straight_rgba(pm: &Pixmap) -> Vec<u8> {
    pm.pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

/// A Windows .ico holding PNG images at the given sizes (`write_icon` makes assets/icon.ico with it).
#[cfg(test)]
fn ico(sizes: &[u32], p: &Palette) -> Vec<u8> {
    let pngs: Vec<Vec<u8>> = sizes.iter().map(|&s| monogram(s, p, true).encode_png().expect("png")).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, png) in sizes.iter().zip(&pngs) {
        let b = if *s >= 256 { 0 } else { *s as u8 };
        out.extend_from_slice(&[b, b, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for png in pngs {
        out.extend_from_slice(&png);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monogram_draws_at_every_icon_size() {
        for s in [16, 24, 32, 48, 128] {
            let pm = monogram(s, &GLACIER, false);
            assert!(pm.pixels().iter().any(|p| p.alpha() > 200), "{s}");
        }
    }

    #[test]
    fn ico_header_lists_all_images() {
        let b = ico(&[16, 32, 256], &POLAR_NIGHT);
        assert_eq!(&b[0..4], &[0, 0, 1, 0]);
        assert_eq!(u16::from_le_bytes([b[4], b[5]]), 3);
    }

    /// `cargo test write_icon -- --ignored` regenerates assets/icon.ico (Polar Night tile).
    #[test]
    #[ignore]
    fn write_icon() {
        std::fs::write("assets/icon.ico", ico(&[16, 24, 32, 48, 64, 256], &POLAR_NIGHT)).unwrap();
    }
}
