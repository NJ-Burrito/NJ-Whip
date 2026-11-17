//! The NJ-Whip engine for the website: the app's own whips, physics, swing detection, crack
//! effects, heat meter and edge glow, compiled to WebAssembly. Nothing is redrawn in JavaScript.
//!
//! The page owns a few "stages" (the room, the heat demo, one per gallery card). Each frame it
//! hands a stage the pointer and the frame time, and gets back opaque RGBA pixels over the Polar
//! Night background plus a small row of numbers (`OUT_*`). The loop in `Stage::frame` follows
//! `App::frame` in the app's `src/app.rs`.
//!
//! What stays on the page's side: sound (the wasm only names the clip files), the combo label
//! (page text in the web font: no font file is in this repo or in the wasm) and the clock
//! (`std::time::Instant` does not exist on this target, so the combo is timed here in seconds).
#![allow(dead_code)]

#[path = "../../src/effects.rs"]
mod effects;
#[path = "../../src/ink.rs"]
mod ink;
#[path = "../../src/messages.rs"]
mod messages;
#[path = "../../src/physics.rs"]
mod physics;
#[path = "../../src/rage.rs"]
mod rage;
#[path = "../../src/render.rs"]
mod render;
#[path = "../../src/swing.rs"]
mod swing;
#[path = "../../src/whipdraw.rs"]
mod whipdraw;
#[path = "../../src/whips.rs"]
mod whips;

/// The renderer only needs the label's em size from the app's glyph module; the label itself
/// is page text here.
mod glyphs {
    pub const EM: f32 = 100.0;
}

/// A clip is its file name: the page plays the file from `assets/sounds/`.
mod sound {
    macro_rules! clip {
        ($name:ident, $file:literal) => {
            pub const $name: &[u8] = $file.as_bytes();
        };
    }
    include!("../../src/clips.rs");
}

use effects::Fx;
use messages::{after_esc, instruction, Intent, Pack, Profile, Style};
use physics::{Params, Rope, V};
use rage::{Heat, COMBO_SHOW};
use render::{Canvas, Scene, R};
use std::cell::RefCell;
use swing::{Event, Swing};
use tiny_skia::Rect;
use whips::CATALOGUE;

// The app's frame constants (src/app.rs) and the combo gap (src/rage.rs).
const STEP: f32 = 1.0 / 60.0;
const SHOW_MOVE: f32 = 0.3;
const FLASH_FOR: f32 = 0.15;
const FIRST_CRACK_GRACE: f32 = 0.35;
const COMBO_GAP: f32 = 1.4;

/// Polar Night, the background every stage is composed over.
const NIGHT: [u32; 3] = [0x0A, 0x10, 0x24];

// The numbers a frame hands back to the page.
const OUT_DIRTY: usize = 0; // x0, y0, x1, y1 of what changed (all 0: nothing)
const OUT_CRACKS: usize = 4; // cracks so far on this stage
const OUT_WHOOSH: usize = 5; // power of a whoosh this frame, 0 = none
const OUT_COMBO: usize = 6; // count, age (s, -1 = no label), x, y, dir
const OUT_HEAT: usize = 11; // shown level, full (0/1), glow
const OUT_TIP: usize = 14; // x, y
const OUT_LEN: usize = 16;

const SWING: u32 = 1;
const METER: u32 = 2;

/// What the last crack left on the screen.
struct Mark {
    since: f32,
    at: V,
    dir: i8,
    angle: f32,
    lit: bool,
}

struct Stage {
    canvas: Canvas,
    /// The frame as the page shows it: opaque RGBA.
    page: Vec<u8>,
    whip: usize,
    /// The whip's physics, with the segment length scaled for a small stage.
    params: Params,
    seg_scale: f32,
    rope: Rope,
    shown: Vec<V>,
    swing: Swing,
    opts: u32,
    fx: Fx,
    heat: Heat,
    /// Combo count and seconds since its last crack.
    combo: (u32, f32),
    mark: Option<Mark>,
    mouse_prev: V,
    acc: f32,
    clock: f32,
    age: f32,
    cracks: u32,
    out: [f32; OUT_LEN],
}

/// From `src/app.rs`: the fixed physics steps that fit in a frame, each with the point the
/// pointer had reached at that step's own time.
fn each_step(acc: &mut f32, dt: f32, from: V, to: V, mut step: impl FnMut(V)) {
    *acc += dt;
    while *acc >= STEP {
        *acc -= STEP;
        let u = (1.0 - *acc / dt.max(1e-6)).clamp(0.0, 1.0);
        step((from.0 + (to.0 - from.0) * u, from.1 + (to.1 - from.1) * u));
    }
}

fn scaled(whip: usize, seg_scale: f32) -> Params {
    let p = CATALOGUE[whip].physics;
    Params { seg_len: p.seg_len * seg_scale, ..p }
}

impl Stage {
    fn new(w: u32, h: u32, seg_scale: f32) -> Option<Stage> {
        let params = scaled(0, seg_scale);
        let hand = (w as f32 / 2.0, h as f32 / 2.0);
        let rope = Rope::spawn(&params, hand);
        let mut page = vec![255u8; (w * h * 4) as usize];
        for px in page.chunks_exact_mut(4) {
            px[0] = NIGHT[0] as u8;
            px[1] = NIGHT[1] as u8;
            px[2] = NIGHT[2] as u8;
        }
        Some(Stage {
            canvas: Canvas::new(w, h)?,
            page,
            whip: 0,
            params,
            seg_scale,
            shown: rope.pts.clone(),
            rope,
            swing: Swing::default(),
            opts: 0,
            fx: Fx::default(),
            heat: Heat::default(),
            combo: (0, f32::MAX),
            mark: None,
            mouse_prev: hand,
            acc: 0.0,
            clock: 0.0,
            age: 0.0,
            cracks: 0,
            out: [0.0; OUT_LEN],
        })
    }

    fn size(&self) -> V {
        let (w, h) = self.canvas.size();
        (w as f32, h as f32)
    }

    fn bounds(&self) -> (f32, f32, f32, f32) {
        let (w, h) = self.size();
        (0.0, 0.0, w, h)
    }

    /// Put whip `whip` in the hand at `hand`. Heat and effects in the air stay, as in the app.
    fn take(&mut self, whip: usize, hand: V) {
        self.whip = whip.min(CATALOGUE.len() - 1);
        self.params = scaled(self.whip, self.seg_scale);
        self.rope = Rope::spawn(&self.params, hand);
        self.shown.clone_from(&self.rope.pts);
        self.swing = Swing::default();
        self.mouse_prev = hand;
        self.acc = 0.0;
        self.age = 0.0;
    }

    /// Let the lash hang: physics steps with the hand still, nothing shown.
    fn settle(&mut self, steps: u32) {
        let b = self.bounds();
        for _ in 0..steps {
            self.rope.step(&self.params, self.mouse_prev, b, false);
        }
        self.shown.clone_from(&self.rope.pts);
    }

    fn reset(&mut self) {
        self.fx = Fx::default();
        self.heat = Heat::default();
        self.combo = (0, f32::MAX);
        self.mark = None;
    }

    fn tip(&self) -> V {
        *self.rope.pts.last().expect("a rope has points")
    }

    /// One crack: heat, combo, the lash thrown toward `dir`, the whip's effect at the tip.
    fn crack(&mut self, dir: i8) {
        if self.opts & METER != 0 {
            self.heat.crack();
        }
        if self.combo.1 > COMBO_GAP {
            self.combo.0 = 0;
        }
        self.combo = (self.combo.0 + 1, 0.0);
        self.rope.kick(dir);
        let at = self.tip();
        self.fx.burst(CATALOGUE[self.whip].effect, at, self.size());
        self.mark = Some(Mark { since: 0.0, at, dir, angle: fastrand::f32() * std::f32::consts::TAU, lit: true });
        self.cracks += 1;
    }

    /// One frame `dt` seconds long with the pointer at `mouse`. Returns 1 when pixels changed,
    /// plus 2 when a swing cracked and 4 when it whooshed.
    fn frame(&mut self, dt: f32, mouse: V) -> u32 {
        let dt = dt.clamp(0.0, 0.1);
        let def = &CATALOGUE[self.whip];
        let (w, h) = self.size();
        self.clock += dt;
        self.age += dt;
        self.combo.1 += dt;
        if let Some(m) = &mut self.mark {
            m.since += dt;
        }

        let (mut whoosh, mut cracked) = (None, None);
        let live = self.opts & SWING != 0 && self.age >= FIRST_CRACK_GRACE;
        let (rope, swing, params) = (&mut self.rope, &mut self.swing, &self.params);
        each_step(&mut self.acc, dt, self.mouse_prev, mouse, |hand| {
            rope.step(params, hand, (0.0, 0.0, w, h), false);
            if !live {
                return;
            }
            match swing.step(rope.hand_vel().0) {
                Some(Event::Whoosh { power, .. }) => whoosh = Some(power),
                Some(Event::Crack { dir, .. }) => cracked = Some(dir),
                None => {}
            }
        });
        let pts = &self.rope.pts;
        if self.shown.len() != pts.len() || pts.iter().zip(&self.shown).any(|(a, b)| (a.0 - b.0).hypot(a.1 - b.1) > SHOW_MOVE) {
            self.shown.clone_from(pts);
        }
        self.mouse_prev = mouse;
        let mut bits = 0;
        if let Some(dir) = cracked {
            self.crack(dir);
            bits |= 2;
        } else if whoosh.is_some() {
            bits |= 4;
        }

        self.fx.step(dt);
        self.heat.step(dt);
        let flash = self.mark.as_ref().filter(|m| m.lit).map_or(0.0, |m| (1.0 - m.since / FLASH_FOR).max(0.0));
        let vibe = def.vibe;
        let (level, offset, edges) = vibe.screen.look(self.clock, self.mark.as_ref().map(|m| (m.since, m.angle)));
        let scene = Scene {
            whip: Some((&self.shown[..], def, None)),
            flash,
            glow: self.heat.glow() * level,
            edges,
            hue: vibe.hue,
            offset,
            scale: 1.0,
            meter: (self.opts & METER != 0).then(|| (self.heat.shown(), self.heat.full())),
            clock: self.clock,
            // the label is page text over the canvas
            combo: None,
            burst: Some(&self.fx),
        };
        let dirty = self.canvas.draw(&scene, Rect::from_xywh(0.0, 0.0, w, h).expect("a stage has a size"));
        let mut bbox: Option<R> = None;
        for &r in &dirty {
            compose(&self.canvas.pm, &mut self.page, r);
            bbox = Some(bbox.map_or(r, |b| (b.0.min(r.0), b.1.min(r.1), b.2.max(r.2), b.3.max(r.3))));
        }
        if bbox.is_some() {
            bits |= 1;
        }

        let b = bbox.unwrap_or((0, 0, 0, 0));
        let label = self.combo.0 >= 2 && self.combo.1 < COMBO_SHOW;
        let at = self.mark.as_ref().map_or((0.0, 0.0, 0), |m| (m.at.0, m.at.1, m.dir));
        let tip = self.tip();
        self.out = [
            b.0 as f32, b.1 as f32, b.2 as f32, b.3 as f32,
            self.cracks as f32,
            whoosh.filter(|_| cracked.is_none()).unwrap_or(0.0),
            self.combo.0 as f32, if label { self.combo.1 } else { -1.0 }, at.0, at.1, at.2 as f32,
            self.heat.shown(), self.heat.full() as u32 as f32, self.heat.glow(),
            tip.0, tip.1,
        ];
        bits
    }
}

/// Source-over of the premultiplied frame on the night background, inside box `r`.
fn compose(pm: &tiny_skia::Pixmap, page: &mut [u8], r: R) {
    let w = pm.width() as usize;
    let src = pm.data();
    for y in r.1.max(0) as usize..r.3.max(0) as usize {
        let (a, z) = ((y * w + r.0.max(0) as usize) * 4, (y * w + r.2.max(0) as usize) * 4);
        for (d, s) in page[a..z].chunks_exact_mut(4).zip(src[a..z].chunks_exact(4)) {
            let inv = 255 - s[3] as u32;
            for c in 0..3 {
                d[c] = (s[c] as u32 + (NIGHT[c] * inv + 127) / 255).min(255) as u8;
            }
        }
    }
}

/// Pixels all live stages may hold together (8 bytes each: the canvas and the page). The site
/// needs about 4 M; past this a new or bigger stage is refused, so a caller that never frees its
/// stages gets `u32::MAX` back instead of growing the wasm memory to its 4 GiB limit and trapping.
const MAX_LIVE_PX: u64 = 32 << 20;

fn px(s: &Stage) -> u64 {
    let (w, h) = s.canvas.size();
    w as u64 * h as u64
}

/// Would `add` more pixels (after `freed` go) still fit in `MAX_LIVE_PX`?
fn fits(stages: &[Option<Box<Stage>>], add: u64, freed: u64) -> bool {
    stages.iter().flatten().map(|s| px(s)).sum::<u64>() - freed + add <= MAX_LIVE_PX
}

thread_local! {
    // boxed: the page keeps pointers into a stage (`stage_out`) while more stages are made.
    // A freed stage leaves `None`; its slot (id) is given to the next new stage.
    static STAGES: RefCell<Vec<Option<Box<Stage>>>> = const { RefCell::new(Vec::new()) };
    static TEXT: RefCell<String> = const { RefCell::new(String::new()) };
}

fn with<T: Default>(id: u32, f: impl FnOnce(&mut Stage) -> T) -> T {
    STAGES.with_borrow_mut(|s| s.get_mut(id as usize).and_then(Option::as_mut).map(|s| f(s)).unwrap_or_default())
}

fn quote(s: &str) -> String {
    let mut q = String::with_capacity(s.len() + 2);
    q.push('"');
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                q.push('\\');
                q.push(c);
            }
            c if (c as u32) < 0x20 => q.push(' '),
            c => q.push(c),
        }
    }
    q.push('"');
    q
}

fn list(items: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(","))
}

fn files(clips: &[&[u8]]) -> String {
    list(clips.iter().map(|c| quote(std::str::from_utf8(c).unwrap_or(""))))
}

const INTENTS: [Intent; 5] = [Intent::Faster, Intent::Harder, Intent::Careful, Intent::Happier, Intent::Focus];

/// The whole catalogue as JSON: every whip (name, colours, lines, the sound files it uses) and
/// every pack's instruction per direction, with how it is sent.
fn catalogue_json() -> String {
    let whips = list(CATALOGUE.iter().map(|w| {
        format!(
            "{{\"name\":{},\"blurb\":{},\"hue\":\"#{:06X}\",\"shout\":{},\"intent\":{},\"effect\":\"{:?}\",\"screen\":\"{:?}\",\"weight\":{},\"cracks\":{},\"sounds\":{},\"lines\":{}}}",
            quote(w.name), quote(w.blurb), w.vibe.hue, quote(w.vibe.shout), quote(w.intent.name()), w.effect, w.vibe.screen,
            w.physics.weight, files(w.cracks), files(w.sounds), list(w.lines.iter().map(|l| quote(l))),
        )
    }));
    let packs = list(Pack::ALL.into_iter().map(|p| {
        let rows = list(INTENTS.into_iter().map(|i| {
            format!(
                "{{\"light\":{},\"heavy\":{},\"esc\":{}}}",
                quote(instruction(i, p, Profile::Light)), quote(instruction(i, p, Profile::Heavy)),
                after_esc(i, Style { pack: p, ..Style::default() }),
            )
        }));
        format!("{{\"name\":{},\"note\":{},\"profile\":{},\"rows\":{}}}", quote(p.name()), quote(p.note()), p.uses_profile(), rows)
    }));
    let intents = list(INTENTS.into_iter().map(|i| quote(i.name())));
    format!("{{\"whips\":{whips},\"intents\":{intents},\"packs\":{packs},\"cracksToFull\":{},\"markLife\":{}}}", rage::CRACKS_TO_FULL, effects::MARK_LIFE)
}

#[no_mangle]
pub extern "C" fn seed(lo: u32, hi: u32) {
    fastrand::seed(((hi as u64) << 32) | lo as u64);
}

/// The catalogue JSON (UTF-8): its address, and its length from `catalogue_len`.
#[no_mangle]
pub extern "C" fn catalogue() -> *const u8 {
    TEXT.with_borrow_mut(|t| {
        if t.is_empty() {
            *t = catalogue_json();
        }
        t.as_ptr()
    })
}

#[no_mangle]
pub extern "C" fn catalogue_len() -> u32 {
    catalogue();
    TEXT.with_borrow(|t| t.len() as u32)
}

/// A new stage `w` x `h` px. `seg_scale` shortens the lash for a small stage (1 = the app's
/// whip). Returns its id, or u32::MAX when it cannot be made (or would pass `MAX_LIVE_PX`).
/// A stage lives until `stage_free`.
#[no_mangle]
pub extern "C" fn stage_new(w: u32, h: u32, seg_scale: f32) -> u32 {
    let (w, h) = (w.clamp(16, 4096), h.clamp(16, 4096));
    STAGES.with_borrow_mut(|s| {
        if !fits(s, w as u64 * h as u64, 0) {
            return u32::MAX;
        }
        let Some(st) = Stage::new(w, h, seg_scale.clamp(0.2, 2.0)) else { return u32::MAX };
        let id = s.iter().position(Option::is_none).unwrap_or(s.len());
        if id == s.len() {
            s.push(None);
        }
        s[id] = Some(Box::new(st));
        id as u32
    })
}

/// Drops a stage and its pixels; its id may be handed out again by `stage_new`.
#[no_mangle]
pub extern "C" fn stage_free(id: u32) {
    STAGES.with_borrow_mut(|s| {
        if let Some(slot) = s.get_mut(id as usize) {
            *slot = None;
        }
    })
}

/// A new size for a stage (the page was resized): the whip, the options and the pointer stay,
/// effects in the air are dropped. A size past `MAX_LIVE_PX` is refused (the old size stays).
#[no_mangle]
pub extern "C" fn stage_resize(id: u32, w: u32, h: u32) {
    let (w, h) = (w.clamp(16, 4096), h.clamp(16, 4096));
    STAGES.with_borrow_mut(|stages| {
        let old = match stages.get(id as usize) {
            Some(Some(s)) => px(s),
            _ => return,
        };
        if !fits(stages, w as u64 * h as u64, old) {
            return;
        }
        let Some(s) = stages[id as usize].as_mut() else { return };
        if let Some(mut st) = Stage::new(w, h, s.seg_scale) {
            st.opts = s.opts;
            st.take(s.whip, (w as f32 / 2.0, h as f32 / 2.0));
            **s = st;
        }
    })
}

/// Options: 1 = swings crack, 2 = the heat meter is on.
#[no_mangle]
pub extern "C" fn stage_opts(id: u32, opts: u32) {
    with(id, |s| s.opts = opts)
}

#[no_mangle]
pub extern "C" fn stage_whip(id: u32, whip: u32, x: f32, y: f32) {
    with(id, |s| s.take(whip as usize, (x, y)))
}

#[no_mangle]
pub extern "C" fn stage_settle(id: u32, steps: u32) {
    with(id, |s| s.settle(steps.min(2000)))
}

/// Clears heat, combo and every effect in the air.
#[no_mangle]
pub extern "C" fn stage_reset(id: u32) {
    with(id, Stage::reset)
}

#[no_mangle]
pub extern "C" fn stage_crack(id: u32, dir: i32) {
    with(id, |s| s.crack(dir.signum() as i8))
}

#[no_mangle]
pub extern "C" fn stage_frame(id: u32, dt: f32, x: f32, y: f32) -> u32 {
    with(id, |s| s.frame(dt, (x, y)))
}

/// The stage's pixels: width x height x 4 bytes, opaque RGBA.
#[no_mangle]
pub extern "C" fn stage_pixels(id: u32) -> *const u8 {
    STAGES.with_borrow(|s| s.get(id as usize).and_then(Option::as_ref).map_or(std::ptr::null(), |s| s.page.as_ptr()))
}

/// The numbers of the last frame (`OUT_LEN` f32s, see the `OUT_*` constants).
#[no_mangle]
pub extern "C" fn stage_out(id: u32) -> *const f32 {
    STAGES.with_borrow(|s| s.get(id as usize).and_then(Option::as_ref).map_or(std::ptr::null(), |s| s.out.as_ptr()))
}
