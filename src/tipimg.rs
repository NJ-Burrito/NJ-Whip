//! Custom tip images: import a user picture or GIF (background removed, cropped) and load it for drawing.
//!
//! Stored as `tips/<name>.png`. A still picture is that PNG alone. An animated one (GIF, animated
//! WebP) is a horizontal strip of equal frames in that PNG plus `tips/<name>.delays`, the frame
//! delays in ms separated by spaces; every frame shares one crop box and one background colour.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use tiny_skia::Pixmap;

/// Largest side kept after import, px.
const MAX_SIDE: u32 = 256;
/// Most frames kept (a longer animation drops frames evenly and keeps its length).
const MAX_FRAMES: usize = 120;
/// Most frames read from a file at all, so a huge GIF cannot fill the memory while importing.
const MAX_READ: usize = 360;
/// A delay of 10 ms or less plays as 100 ms, like browsers do.
const MIN_DELAY_MS: u32 = 10;
/// Colour distance (0..441) under which a pixel joins the background.
const TOLERANCE: f32 = 48.0;

pub fn tips_dir() -> PathBuf {
    crate::config::config_path().with_file_name("tips")
}

/// A tip as loaded: one frame, or the frames of an animation with the time each one ends.
pub struct Tip {
    frames: Vec<Pixmap>,
    /// Seconds from the start to the end of each frame (never empty, same length as `frames`).
    ends: Vec<f32>,
}

impl Tip {
    fn new(frames: Vec<Pixmap>, delays_ms: &[u32]) -> Tip {
        let mut t = 0.0;
        let ends = delays_ms.iter().map(|d| { t += *d as f32 / 1000.0; t }).collect();
        Tip { frames, ends }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    fn index_at(&self, t: f32) -> (usize, f32) {
        let t = t.rem_euclid(*self.ends.last().unwrap_or(&1.0)).max(0.0);
        (self.ends.iter().position(|e| t < *e).unwrap_or(0), t)
    }

    /// The frame showing `t` seconds after the start (the animation loops).
    pub fn frame_at(&self, t: f32) -> &Pixmap {
        &self.frames[self.index_at(t).0]
    }

    /// Seconds until the picture changes (None for a still one).
    pub fn next_change(&self, t: f32) -> Option<f32> {
        if self.frames.len() < 2 {
            return None;
        }
        let (i, t) = self.index_at(t);
        Some(self.ends[i] - t)
    }
}

/// Loads a processed tip. Missing or broken file: None (the built-in tip is used).
pub fn load(path: &Path) -> Option<Tip> {
    let strip = Pixmap::load_png(path).ok().filter(|p| p.width() > 0 && p.height() > 0)?;
    let delays: Vec<u32> = std::fs::read_to_string(path.with_extension("delays"))
        .map(|s| s.split_whitespace().filter_map(|d| d.parse().ok()).collect())
        .unwrap_or_default();
    let n = delays.len() as u32;
    if n < 2 || strip.width() % n != 0 {
        return Some(Tip::new(vec![strip], &[1000]));
    }
    let fw = strip.width() / n;
    let frames = (0..n)
        .map(|i| strip.clone_rect(tiny_skia::IntRect::from_xywh((i * fw) as i32, 0, fw, strip.height())?))
        .collect::<Option<Vec<_>>>()?;
    Some(Tip::new(frames, &delays))
}

type Rgb = (f32, f32, f32);

/// The background colour of a picture: the median of its opaque border pixels.
fn border_colour(img: &Pixmap) -> Rgb {
    let (w, h) = (img.width(), img.height());
    let mut ch: [Vec<u8>; 3] = Default::default();
    let border = (0..w).flat_map(|x| [(x, 0), (x, h - 1)]).chain((0..h).flat_map(|y| [(0, y), (w - 1, y)]));
    for (x, y) in border {
        if let Some(p) = img.pixel(x, y).filter(|p| p.alpha() == 255) {
            ch[0].push(p.red());
            ch[1].push(p.green());
            ch[2].push(p.blue());
        }
    }
    let med = |v: &mut Vec<u8>| { v.sort_unstable(); v.get(v.len() / 2).copied().unwrap_or(0) as f32 };
    let [r, g, b] = &mut ch;
    (med(r), med(g), med(b))
}

/// Makes the background transparent: flood-fills from every border pixel through pixels whose
/// colour is close to the starting border colour, or close to `reference` when one is given (an
/// animation's frames all use the first frame's, so the background cannot flicker).
pub fn remove_background(img: &mut Pixmap, reference: Option<Rgb>) {
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
        let base = reference.unwrap_or_else(|| px(data, si));
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

/// The box (x0, y0, x1, y1, inclusive) around what is visible, or None for an empty picture.
fn subject_box(img: &Pixmap) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = (img.width(), img.height());
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if img.pixel(x, y).is_some_and(|p| p.alpha() > 8) {
                x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x); y1 = y1.max(y);
            }
        }
    }
    (x1 >= x0 && y1 >= y0).then_some((x0, y0, x1, y1))
}

fn union_box(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}

/// Size of the box after it is fitted to MAX_SIDE, and the scale that does it.
fn fitted(b: (u32, u32, u32, u32)) -> (u32, u32, f32) {
    let (cw, ch) = (b.2 - b.0 + 1, b.3 - b.1 + 1);
    let s = (MAX_SIDE as f32 / cw.max(ch) as f32).min(1.0);
    ((((cw as f32) * s).ceil() as u32).max(1), (((ch as f32) * s).ceil() as u32).max(1), s)
}

/// Draws the box of `img` into `out` with its left edge at `dx`.
fn draw_box(out: &mut Pixmap, dx: i32, img: &Pixmap, b: (u32, u32, u32, u32), s: f32) {
    let tf = tiny_skia::Transform::from_scale(s, s).pre_translate(-(b.0 as f32), -(b.1 as f32)).post_translate(dx as f32, 0.0);
    out.draw_pixmap(0, 0, img.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() }, tf, None);
}

#[cfg(test)]
fn crop_and_fit(img: &Pixmap) -> Option<Pixmap> {
    let b = subject_box(img)?;
    let (ow, oh, s) = fitted(b);
    let mut out = Pixmap::new(ow, oh)?;
    draw_box(&mut out, 0, img, b, s);
    Some(out)
}

/// Every frame of the file with its delay in ms: a still picture is one frame.
fn decode(src: &Path) -> Result<Vec<(Pixmap, u32)>, String> {
    use image::{AnimationDecoder, ImageFormat};
    let bad = |e: &dyn std::fmt::Display| format!("Couldn't read that picture ({e}).");
    let bytes = std::fs::read(src).map_err(|e| bad(&e))?;
    let rgba = |b: &image::RgbaImage| -> Option<Pixmap> {
        let mut p = Pixmap::new(b.width(), b.height())?;
        for (d, s) in p.data_mut().chunks_exact_mut(4).zip(b.pixels()) {
            let a = s.0[3] as u32;
            for i in 0..3 {
                d[i] = ((s.0[i] as u32 * a + 127) / 255) as u8;
            }
            d[3] = a as u8;
        }
        Some(p)
    };
    let frames = |it: image::Frames| -> Result<Vec<(Pixmap, u32)>, String> {
        it.take(MAX_READ)
            .map(|f| {
                let f = f.map_err(|e| bad(&e))?;
                let (n, d) = f.delay().numer_denom_ms();
                let ms = if d == 0 { 100 } else { n / d };
                Ok((rgba(f.buffer()).ok_or("The picture is empty.")?, if ms <= MIN_DELAY_MS { 100 } else { ms }))
            })
            .collect()
    };
    let still = |img: image::DynamicImage| -> Result<Vec<(Pixmap, u32)>, String> {
        Ok(vec![(rgba(&img.to_rgba8()).ok_or("The picture is empty.")?, 1000)])
    };
    let cursor = std::io::Cursor::new(&bytes);
    match image::guess_format(&bytes) {
        Ok(ImageFormat::Gif) => frames(image::codecs::gif::GifDecoder::new(cursor).map_err(|e| bad(&e))?.into_frames()),
        Ok(ImageFormat::WebP) => {
            let d = image::codecs::webp::WebPDecoder::new(cursor).map_err(|e| bad(&e))?;
            if d.has_animation() {
                return frames(d.into_frames());
            }
            still(image::DynamicImage::from_decoder(d).map_err(|e| bad(&e))?)
        }
        _ => still(image::load_from_memory(&bytes).map_err(|e| bad(&e))?),
    }
}

/// Over MAX_FRAMES: keeps every k-th frame and gives it the delays of the ones dropped after it.
fn thin(frames: Vec<(Pixmap, u32)>) -> Vec<(Pixmap, u32)> {
    let k = frames.len().div_ceil(MAX_FRAMES);
    if k < 2 {
        return frames;
    }
    let mut out: Vec<(Pixmap, u32)> = Vec::new();
    for (i, f) in frames.into_iter().enumerate() {
        match out.last_mut() {
            Some(last) if i % k != 0 => last.1 += f.1,
            _ => out.push(f),
        }
    }
    out
}

pub fn import(src: &Path, name: &str) -> Result<PathBuf, String> {
    import_to(src, &tips_dir(), name)
}

fn import_to(src: &Path, dir: &Path, name: &str) -> Result<PathBuf, String> {
    let mut frames = thin(decode(src)?);
    let reference = (frames.len() > 1).then(|| border_colour(&frames[0].0));
    let mut b = None;
    for (img, _) in &mut frames {
        remove_background(img, reference);
        b = match (b, subject_box(img)) {
            (a, None) | (None, a) => a,
            (Some(a), Some(c)) => Some(union_box(a, c)),
        };
    }
    let b = b.ok_or("The picture is empty after removing its background.")?;
    let (ow, oh, s) = fitted(b);
    let mut strip = Pixmap::new(ow * frames.len() as u32, oh).ok_or("The picture is too large.")?;
    for (i, (img, _)) in frames.iter().enumerate() {
        draw_box(&mut strip, (i as u32 * ow) as i32, img, b, s);
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dest = dir.join(format!("{name}.png"));
    let delays = dest.with_extension("delays");
    strip.save_png(&dest).map_err(|e| e.to_string())?;
    if frames.len() > 1 {
        let list: Vec<String> = frames.iter().map(|f| f.1.to_string()).collect();
        std::fs::write(&delays, list.join(" ")).map_err(|e| e.to_string())?;
    } else {
        let _ = std::fs::remove_file(&delays);
    }
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
        remove_background(&mut p, None);
        assert_eq!(p.pixel(0, 0).unwrap().alpha(), 0);
        assert_eq!(p.pixel(59, 39).unwrap().alpha(), 0);
        assert_eq!(p.pixel(30, 20).unwrap().alpha(), 255);
    }

    #[test]
    fn crop_keeps_only_the_subject() {
        let mut p = sample();
        remove_background(&mut p, None);
        let c = crop_and_fit(&p).unwrap();
        assert!(c.width() <= 22 && c.height() <= 22, "{}x{}", c.width(), c.height());
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("njw-tip-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Grey 60x40 picture with a red disc at `cx`.
    fn disc(cx: f32) -> Pixmap {
        let mut p = sample();
        p.fill(Color::from_rgba8(150, 160, 170, 255));
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(220, 40, 40, 255));
        let c = PathBuilder::from_circle(cx, 20.0, 8.0).unwrap();
        p.fill_path(&c, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
        p
    }

    fn rgba_image(p: &Pixmap) -> image::RgbaImage {
        image::RgbaImage::from_fn(p.width(), p.height(), |x, y| {
            let c = p.pixel(x, y).unwrap().demultiply();
            image::Rgba([c.red(), c.green(), c.blue(), c.alpha()])
        })
    }

    #[test]
    fn jpeg_imports_as_one_still_tip() {
        let dir = temp("jpg");
        let src = dir.join("a.jpg");
        image::DynamicImage::ImageRgba8(rgba_image(&sample())).to_rgb8().save(&src).unwrap();
        let dest = import_to(&src, &dir.join("tips"), "t").unwrap();
        assert!(!dest.with_extension("delays").exists());
        let tip = load(&dest).unwrap();
        assert_eq!(tip.len(), 1);
        assert_eq!(tip.next_change(0.3), None);
        let f = tip.frame_at(5.0);
        assert!(f.width() <= 24 && f.height() <= 24, "{}x{}", f.width(), f.height());
        assert_eq!(f.pixel(f.width() / 2, f.height() / 2).unwrap().alpha(), 255);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn gif_keeps_frames_delays_and_one_crop() {
        use image::{codecs::gif::GifEncoder, Delay, Frame};
        let dir = temp("gif");
        let src = dir.join("a.gif");
        let mut enc = GifEncoder::new(std::fs::File::create(&src).unwrap());
        for (cx, ms) in [(15.0, 100), (30.0, 200), (45.0, 100)] {
            enc.encode_frame(Frame::from_parts(rgba_image(&disc(cx)), 0, 0, Delay::from_numer_denom_ms(ms, 1))).unwrap();
        }
        drop(enc);
        let dest = import_to(&src, &dir.join("tips"), "t").unwrap();
        assert_eq!(std::fs::read_to_string(dest.with_extension("delays")).unwrap(), "100 200 100");
        let tip = load(&dest).unwrap();
        assert_eq!(tip.len(), 3);
        let (w, h) = (tip.frames[0].width(), tip.frames[0].height());
        // one shared box: the union of the three discs, not each disc alone
        assert!(w > 40 && h < 25 && tip.frames.iter().all(|f| (f.width(), f.height()) == (w, h)), "{w}x{h}");
        let left = |f: &Pixmap| (0..w).find(|&x| (0..h).any(|y| f.pixel(x, y).unwrap().alpha() > 200)).unwrap();
        assert!(left(&tip.frames[2]) > left(&tip.frames[0]) + 20);
        // the same background removed in every frame
        for f in &tip.frames {
            assert_eq!(f.pixel(0, 0).unwrap().alpha(), 0);
            assert_eq!(f.pixel(w - 1, h - 1).unwrap().alpha(), 0);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn frame_follows_time_and_loops() {
        let mut frames = Vec::new();
        for g in [10, 20, 30] {
            let mut p = Pixmap::new(2, 2).unwrap();
            p.fill(Color::from_rgba8(g, g, g, 255));
            frames.push(p);
        }
        let tip = Tip::new(frames, &[100, 200, 100]);
        let which = |t: f32| tip.frame_at(t).pixel(0, 0).unwrap().red();
        assert_eq!([0.0, 0.05, 0.1, 0.29, 0.31, 0.39, 0.4, 0.45, 4.85].map(which), [10, 10, 20, 20, 30, 30, 10, 10, 10]);
        assert!((tip.next_change(0.05).unwrap() - 0.05).abs() < 1e-5);
        assert!((tip.next_change(0.15).unwrap() - 0.15).abs() < 1e-5);
    }

    #[test]
    fn long_animations_are_thinned_keeping_their_length() {
        let frames: Vec<_> = (0..250).map(|_| (Pixmap::new(1, 1).unwrap(), 40)).collect();
        let out = thin(frames);
        assert!(out.len() <= MAX_FRAMES);
        assert_eq!(out.iter().map(|f| f.1).sum::<u32>(), 250 * 40);
    }

    #[test]
    fn broken_file_loads_as_none() {
        let f = std::env::temp_dir().join(format!("njw-bad-{}.png", std::process::id()));
        std::fs::write(&f, b"not a png").unwrap();
        assert!(load(&f).is_none());
        let _ = std::fs::remove_file(&f);
    }
}
