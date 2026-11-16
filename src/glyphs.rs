//! Text for the combo label: Thmanyah outlines turned into tiny-skia paths.
//! The font is read from the user's fonts folder (licensed files, never embedded); without it
//! there is simply no label.

use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::*;
use std::collections::HashMap;
use tiny_skia::{Path, PathBuilder, Transform};

/// Glyphs are stored at this size (px per em); callers scale.
pub const EM: f32 = 100.0;

/// A font file from the per-user fonts folder, where Thmanyah is installed.
pub fn user_font(file: &str) -> Option<Vec<u8>> {
    let dir = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(r"Microsoft\Windows\Fonts");
    std::fs::read(dir.join(file)).ok()
}

struct Pen(PathBuilder);

impl OutlinePen for Pen {
    // Font y points up, screen y points down.
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, -y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, -y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.0.quad_to(cx, -cy, x, -y);
    }
    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        self.0.cubic_to(ax, -ay, bx, -by, x, -y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}

pub struct Glyphs {
    map: HashMap<char, (Option<Path>, f32)>,
}

impl Glyphs {
    pub fn load() -> Option<Glyphs> {
        ["thmanyahserifdisplay-Black.otf", "thmanyahsans-Black.otf", "thmanyahsans-Bold.otf"].iter().find_map(|f| Glyphs::from_bytes(&user_font(f)?))
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Glyphs> {
        let font = FontRef::new(bytes).ok()?;
        let (outlines, cmap) = (font.outline_glyphs(), font.charmap());
        let metrics = font.glyph_metrics(Size::new(EM), LocationRef::default());
        let mut map = HashMap::new();
        for ch in (' '..='Z').chain(['×']) {
            let Some(gid) = cmap.map(ch) else { continue };
            let mut pen = Pen(PathBuilder::new());
            if let Some(g) = outlines.get(gid) {
                let _ = g.draw(DrawSettings::unhinted(Size::new(EM), LocationRef::default()), &mut pen);
            }
            map.insert(ch, (pen.0.finish(), metrics.advance_width(gid).unwrap_or(EM * 0.5)));
        }
        map.contains_key(&'0').then_some(Glyphs { map })
    }

    /// One line of text as a path (baseline at y = 0, starting at x = 0) and its width, at `EM`.
    pub fn line(&self, text: &str) -> Option<(Path, f32)> {
        let mut pb = PathBuilder::new();
        let mut x = 0.0;
        for ch in text.chars() {
            let ch = if ch == '×' && !self.map.contains_key(&'×') { 'X' } else { ch };
            let Some((path, adv)) = self.map.get(&ch) else { continue };
            // The multiplication sign is small in this face: draw it 1.4x (about the baseline)
            // with a little air each side so it is not swallowed by the digit.
            let (k, pad) = if ch == '×' { (1.4, EM * 0.08) } else { (1.0, 0.0) };
            x += pad;
            if let Some(p) = path.clone().and_then(|p| p.transform(Transform::from_row(k, 0.0, 0.0, k, x, 0.0))) {
                pb.push_path(&p);
            }
            x += adv * k + pad;
        }
        Some((pb.finish()?, x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs on the real installed font; a machine without Thmanyah has no label (and no test).
    #[test]
    fn combo_text_becomes_a_path() {
        let Some(g) = Glyphs::load() else { return };
        let (path, width) = g.line("CRACK ×3").expect("label path");
        assert!(width > EM * 2.0 && path.bounds().height() > EM * 0.4, "width {width}");
        assert!(path.bounds().bottom() < EM * 0.3, "sits on the baseline, y down");
        let (_, without) = g.line("CRACK 3").expect("label path");
        assert!(width > without, "the × adds width: {width} vs {without}");
    }
}
