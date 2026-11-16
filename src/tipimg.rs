//! Custom tip images: import a user picture (background removed, cropped) and load it for drawing.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use tiny_skia::Pixmap;

/// Largest side kept after import, px.
const MAX_SIDE: u32 = 256;
/// Colour distance (0..441) under which a pixel joins the background.
const TOLERANCE: f32 = 48.0;

pub fn tips_dir() -> PathBuf {
    crate::config::config_path().with_file_name("tips")
}

/// Loads a processed tip PNG. Missing or broken file → None (the built-in tip is used).
pub fn load(path: &Path) -> Option<Pixmap> {
    Pixmap::load_png(path).ok().filter(|p| p.width() > 0 && p.height() > 0)
}

/// Makes the background transparent: flood-fills from every border pixel through pixels whose
/// colour is close to the starting border colour.
pub fn remove_background(img: &mut Pixmap) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let data = img.data_mut();
    let px = |d: &[u8], i: usize| (d[i * 4] as f32, d[i * 4 + 1] as f32, d[i * 4 + 2] as f32);
    let mut seen = vec![false; w * h];
    let mut q = VecDeque::new();
    for x in 0..w {
        q.push_back((x, 0, x));
        q.push_back((x, h - 1, (h - 1) * w + x));
    }
    for y in 0..h {
        q.push_back((0, y, y * w));
        q.push_back((w - 1, y, y * w + w - 1));
    }
    let seeds: Vec<_> = q.drain(..).collect();
    for (sx, sy, si) in seeds {
        if seen[si] {
            continue;
        }
        let base = px(data, si);
        q.push_back((sx, sy, si));
        seen[si] = true;
        while let Some((x, y, i)) = q.pop_front() {
            let c = px(data, i);
            let d = ((c.0 - base.0).powi(2) + (c.1 - base.1).powi(2) + (c.2 - base.2).powi(2)).sqrt();
            if d > TOLERANCE && data[i * 4 + 3] > 0 {
                continue;
            }
            data[i * 4..i * 4 + 4].fill(0);
            let mut push = |nx: usize, ny: usize| {
                let ni = ny * w + nx;
                if !seen[ni] {
                    seen[ni] = true;
                    q.push_back((nx, ny, ni));
                }
            };
            if x > 0 { push(x - 1, y); }
            if x + 1 < w { push(x + 1, y); }
            if y > 0 { push(x, y - 1); }
            if y + 1 < h { push(x, y + 1); }
        }
    }
}

fn crop_and_fit(img: &Pixmap) -> Option<Pixmap> {
    let (w, h) = (img.width(), img.height());
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if img.pixel(x, y).is_some_and(|p| p.alpha() > 8) {
                x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x); y1 = y1.max(y);
            }
        }
    }
    if x1 < x0 || y1 < y0 {
        return None;
    }
    let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
    let s = (MAX_SIDE as f32 / cw.max(ch) as f32).min(1.0);
    let (ow, oh) = (((cw as f32) * s).ceil() as u32, ((ch as f32) * s).ceil() as u32);
    let mut out = Pixmap::new(ow.max(1), oh.max(1))?;
    let tf = tiny_skia::Transform::from_scale(s, s).pre_translate(-(x0 as f32), -(y0 as f32));
    out.draw_pixmap(0, 0, img.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() }, tf, None);
    Some(out)
}

pub fn import(src: &Path, name: &str) -> Result<PathBuf, String> {
    let mut img = Pixmap::load_png(src).map_err(|e| format!("Couldn't read that picture as PNG ({e})."))?;
    remove_background(&mut img);
    let out = crop_and_fit(&img).ok_or("The picture is empty after removing its background.")?;
    let dir = tips_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(format!("{name}.png"));
    out.save_png(&dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{Color, Paint, PathBuilder, Transform};

    fn sample() -> Pixmap {
        let mut p = Pixmap::new(60, 40).unwrap();
        p.fill(Color::from_rgba8(150, 160, 170, 255));
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(220, 40, 40, 255));
        let c = PathBuilder::from_circle(30.0, 20.0, 10.0).unwrap();
        p.fill_path(&c, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
        p
    }

    #[test]
    fn background_goes_transparent_subject_stays() {
        let mut p = sample();
        remove_background(&mut p);
        assert_eq!(p.pixel(0, 0).unwrap().alpha(), 0);
        assert_eq!(p.pixel(59, 39).unwrap().alpha(), 0);
        assert_eq!(p.pixel(30, 20).unwrap().alpha(), 255);
    }

    #[test]
    fn crop_keeps_only_the_subject() {
        let mut p = sample();
        remove_background(&mut p);
        let c = crop_and_fit(&p).unwrap();
        assert!(c.width() <= 22 && c.height() <= 22, "{}x{}", c.width(), c.height());
    }

    #[test]
    fn broken_file_loads_as_none() {
        let f = std::env::temp_dir().join(format!("njw-bad-{}.png", std::process::id()));
        std::fs::write(&f, b"not a png").unwrap();
        assert!(load(&f).is_none());
        let _ = std::fs::remove_file(&f);
    }
}
