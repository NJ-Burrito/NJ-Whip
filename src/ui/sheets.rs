//! Off-screen pictures of the window (ignored tests: they write PNGs, they never open a window).
//!
//! `cargo test --release --locked ui:: -- --ignored --nocapture` renders every tab, in English and
//! Arabic, dark and light, to `target/ui-sheets` (or `NJ_WHIP_UI_OUT`). `NJ_WHIP_PPP`
//! sets the scale (default 1), `NJ_WHIP_TAB_ONLY` one tab, `NJ_WHIP_H` the height (default 760, the
//! window's own; a taller one shows the whole scroll), `NJ_WHIP_LIGHT` adds the Glacier theme.

use super::*;

/// A software rasteriser for egui's triangle meshes: bilinear texture sample times vertex colour,
/// source-over in byte space, the same blend the live window's GL backend uses.
struct Raster {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
    textures: std::collections::HashMap<egui::TextureId, (usize, usize, Vec<Color32>)>,
}

impl Raster {
    fn new(w: usize, h: usize) -> Self {
        Raster { w, h, px: vec![[0.0, 0.0, 0.0, 255.0]; w * h], textures: Default::default() }
    }

    fn apply(&mut self, set: &egui::epaint::textures::TexturesDelta) {
        for (id, d) in set.set.iter().flat_map(|(id, ds)| ds.iter().map(move |d| (id, d))) {
            let egui::epaint::ImageData::Color(img) = &d.image;
            let [iw, ih] = img.size;
            match d.pos {
                None => {
                    self.textures.insert(*id, (iw, ih, img.pixels.clone()));
                }
                Some([x0, y0]) => {
                    let t = self.textures.get_mut(id).expect("patch of a known texture");
                    for y in 0..ih {
                        for x in 0..iw {
                            t.2[(y0 + y) * t.0 + x0 + x] = img.pixels[y * iw + x];
                        }
                    }
                }
            }
        }
    }

    fn sample(tex: &(usize, usize, Vec<Color32>), u: f32, v: f32) -> [f32; 4] {
        let (tw, th, px) = (tex.0, tex.1, &tex.2);
        let x = (u * tw as f32 - 0.5).clamp(0.0, tw as f32 - 1.0);
        let y = (v * th as f32 - 0.5).clamp(0.0, th as f32 - 1.0);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(tw - 1), (y0 + 1).min(th - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let at = |x: usize, y: usize| {
            let c = px[y * tw + x];
            [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
        };
        let (a, b, c2, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
        let mut o = [0.0; 4];
        for k in 0..4 {
            o[k] = (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c2[k] * (1.0 - fx) + d[k] * fx) * fy;
        }
        o
    }

    fn draw(&mut self, prims: &[egui::ClippedPrimitive], ppp: f32) {
        for cp in prims {
            let egui::epaint::Primitive::Mesh(mesh) = &cp.primitive else { continue };
            let tex = self.textures.get(&mesh.texture_id).cloned();
            let clip = cp.clip_rect;
            let (cx0, cy0) = ((clip.min.x * ppp).round().max(0.0) as usize, (clip.min.y * ppp).round().max(0.0) as usize);
            let (cx1, cy1) = (((clip.max.x * ppp).round().max(0.0) as usize).min(self.w), ((clip.max.y * ppp).round().max(0.0) as usize).min(self.h));
            for t in mesh.indices.chunks_exact(3) {
                let v = [mesh.vertices[t[0] as usize], mesh.vertices[t[1] as usize], mesh.vertices[t[2] as usize]];
                let p = v.map(|v| (v.pos.x * ppp, v.pos.y * ppp));
                let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
                if area.abs() < 1e-6 {
                    continue;
                }
                let minx = p.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor().max(cx0 as f32) as usize;
                let maxx = (p.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(cx1);
                let miny = p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor().max(cy0 as f32) as usize;
                let maxy = (p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(cy1);
                for y in miny..maxy {
                    for x in minx..maxx {
                        let (sx, sy) = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((p[1].0 - sx) * (p[2].1 - sy) - (p[2].0 - sx) * (p[1].1 - sy)) / area;
                        let w1 = ((p[2].0 - sx) * (p[0].1 - sy) - (p[0].0 - sx) * (p[2].1 - sy)) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                            continue;
                        }
                        let col = |c: Color32| [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32];
                        let (c0, c1, c2) = (col(v[0].color), col(v[1].color), col(v[2].color));
                        let mut src = [0.0f32; 4];
                        for k in 0..4 {
                            src[k] = c0[k] * w0 + c1[k] * w1 + c2[k] * w2;
                        }
                        if let Some(tex) = &tex {
                            let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                            let vv = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                            let s = Self::sample(tex, u, vv);
                            for k in 0..4 {
                                src[k] = src[k] * s[k] / 255.0;
                            }
                        }
                        let d = &mut self.px[y * self.w + x];
                        let inv = 1.0 - src[3] / 255.0;
                        for k in 0..3 {
                            d[k] = src[k] + d[k] * inv;
                        }
                    }
                }
            }
        }
    }

    fn save(&self, path: &std::path::Path) {
        let mut img = image::RgbImage::new(self.w as u32, self.h as u32);
        for (i, p) in self.px.iter().enumerate() {
            img.put_pixel((i % self.w) as u32, (i / self.w) as u32, image::Rgb([p[0].clamp(0.0, 255.0) as u8, p[1].clamp(0.0, 255.0) as u8, p[2].clamp(0.0, 255.0) as u8]));
        }
        img.save(path).expect("write png");
    }
}

/// One tab of the window, in one language and one theme, drawn as the live window draws it.
fn render_tab(tab: Tab, lang: Lang, dark: bool, ppp: f32) -> Raster {
    let h: f32 = std::env::var("NJ_WHIP_H").ok().and_then(|v| v.parse().ok()).unwrap_or(760.0);
    let w = 560.0;
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(ppp);
    let mut win = Window::build(&ctx);
    // the real settings file is never read or written here
    win.cfg = Config::default();
    win.cfg.lang = lang;
    win.cfg.theme = if dark { Theme::Dark } else { Theme::Light };
    win.words = Rc::new(Words::new(lang));
    win.cfg_stamp = config::mtime();
    win.errors = String::new();
    win.errors_stamp = error_stamp();
    win.tab = tab;
    let mut raster = Raster::new((w * ppp) as usize, (h * ppp) as usize);
    for frame in 0..6 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(w, h)));
        input.time = Some(frame as f64 * 0.12);
        let out = ctx.run_ui(input, |ui| win.show(ui));
        raster.apply(&out.textures_delta);
        if frame == 5 {
            let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
            raster.draw(&prims, out.pixels_per_point);
        }
    }
    raster
}

#[test]
#[ignore = "writes PNGs of every tab to target/ui-sheets"]
fn render_sheets() {
    let dir = std::path::PathBuf::from(std::env::var("NJ_WHIP_UI_OUT").unwrap_or_else(|_| "target/ui-sheets".into()));
    std::fs::create_dir_all(&dir).expect("out dir");
    let ppp: f32 = std::env::var("NJ_WHIP_PPP").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let only = std::env::var("NJ_WHIP_TAB_ONLY").ok();
    let themes: &[bool] = if std::env::var_os("NJ_WHIP_LIGHT").is_some() { &[true, false] } else { &[true] };
    let mut index = String::new();
    for lang in [Lang::En, Lang::Ar] {
        for &dark in themes {
            for tab in Tab::ALL {
                if only.as_deref().is_some_and(|o| !o.eq_ignore_ascii_case(tab.name())) {
                    continue;
                }
                let name = format!("{}-{}-{}.png", tab.name().to_lowercase(), if lang == Lang::En { "en" } else { "ar" }, if dark { "dark" } else { "light" });
                render_tab(tab, lang, dark, ppp).save(&dir.join(&name));
                index.push_str(&name);
                index.push('\n');
            }
        }
    }
    std::fs::write(dir.join("index.txt"), index).expect("index");
    println!("sheets in {}", dir.display());
}
