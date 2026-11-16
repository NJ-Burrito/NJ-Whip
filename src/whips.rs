//! The whip catalogue: every whip type is data (look, tip, physics feel, crack effect, sound, messages).

use crate::messages::Intent;
use crate::physics::Params;
use crate::sound;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum WhipId {
    #[default]
    Classic,
    Neon,
    Elephant,
    Cowboy,
    Cheerleader,
    Sauce,
    DrillSergeant,
    Zen,
    Ninja,
    Pirate,
    Chef,
    Wizard,
    Vine,
    Penguin,
    Storm,
}

pub const ALL: [WhipId; 15] = [
    WhipId::Classic, WhipId::Neon, WhipId::Elephant, WhipId::Cowboy, WhipId::Cheerleader,
    WhipId::Sauce, WhipId::DrillSergeant, WhipId::Zen, WhipId::Ninja, WhipId::Pirate,
    WhipId::Chef, WhipId::Wizard, WhipId::Vine, WhipId::Penguin, WhipId::Storm,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pattern { Plain, Braid, Rings, Ribbon, Chunks }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tip { Frayed, Bead, Tail, PomPom, Drip, Bloom, Penguin, Cloud, Kunai }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect { Sparks, Flash, Thump, Lasso, Confetti, Exclaim, Ripple, Slash, Spray, Sizzle, Magic, Splash, Petals, Snow, Lightning }

/// Colours are 0xRRGGBB.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub w0: f32,
    pub w1: f32,
    pub body: u32,
    pub edge: u32,
    pub mark: u32,
    pub mark2: u32,
    pub hl: u32,
    pub cracker: u32,
    pub glow: Option<u32>,
    pub pattern: Pattern,
    pub handle: u32,
    pub wrap: u32,
    pub knob: u32,
    pub ferrule: u32,
}

pub struct WhipDef {
    pub id: WhipId,
    pub name: &'static str,
    pub blurb: &'static str,
    pub look: Look,
    pub tip: Tip,
    pub effect: Effect,
    pub intent: Intent,
    /// The whip's crack: one of these at random per crack.
    pub cracks: &'static [&'static [u8]],
    /// The whip's own clips, played on top of the crack.
    pub sounds: &'static [&'static [u8]],
    pub lines: &'static [&'static str],
    pub physics: Params,
    pub vibe: Vibe,
}

/// What the screen does at full heat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    /// A soft glow breathing at the screen edges.
    Glow,
    /// The glow strobes like a faulty tube or far lightning.
    Flicker,
    /// The glow sways slowly side to side.
    Wobble,
    /// The glow jumps on every crack.
    Jolt,
    /// Strongest along the bottom, breathing slowly: heat or growth from below.
    Rise,
    /// Strongest along the top, slower still: things drifting down.
    Fall,
    /// Left and right take turns gently; top and bottom stay low.
    Sides,
}

/// A whip's mood: its colour for the meter, glow and combo, its screen effect, its combo shout.
#[derive(Clone, Copy)]
pub struct Vibe {
    pub hue: u32,
    pub screen: Screen,
    pub shout: &'static str,
}

/// The Jolt screen effect: the glow jumps this far (px) on a crack and settles in this time (s).
const JOLT_AMP: f32 = 12.0;
const JOLT_FOR: f32 = 0.25;

impl Screen {
    /// The screen effect at clock `t` (s): the glow's level, its shift in px and the share of
    /// each edge (top, bottom, left, right). `crack` is the last crack: seconds since it and
    /// the direction (rad) it kicks the glow.
    pub fn look(self, t: f32, crack: Option<(f32, f32)>) -> (f32, (f32, f32), [f32; 4]) {
        const EVEN: [f32; 4] = [1.0; 4];
        // what is left of the kick (1 -> 0) and its direction
        let jolt = crack.map_or((0.0, 0.0), |(since, angle)| ((1.0 - since / JOLT_FOR).max(0.0), angle));
        let still = (0.0, 0.0);
        let breathe = 0.8 + 0.2 * (t * 5.0).sin();
        match self {
            Screen::Glow => (breathe, still, EVEN),
            Screen::Flicker => (0.75 + 0.25 * (t * 19.0).sin() * (t * 4.3).sin(), still, EVEN),
            Screen::Wobble => (0.85, (10.0 * (t * 3.0).sin(), 3.0 * (t * 6.0).sin()), EVEN),
            Screen::Jolt => (breathe, (JOLT_AMP * jolt.0 * jolt.1.cos(), JOLT_AMP * jolt.0 * jolt.1.sin()), EVEN),
            Screen::Rise => (0.85 + 0.15 * (t * 2.4).sin(), still, [0.3, 1.0, 0.65, 0.65]),
            Screen::Fall => (0.85 + 0.15 * (t * 1.5).sin(), still, [1.0, 0.3, 0.65, 0.65]),
            Screen::Sides => {
                // about 1.2 Hz
                let a = 0.3 * (t * 7.5).sin();
                (0.9, still, [0.35, 0.35, 0.7 + a, 0.7 - a])
            }
        }
    }
}

const fn look(w0: f32, w1: f32, body: u32, edge: u32, mark: u32, mark2: u32, hl: u32, cracker: u32, pattern: Pattern,
              handle: u32, wrap: u32, knob: u32, ferrule: u32) -> Look {
    Look { w0, w1, body, edge, mark, mark2, hl, cracker, glow: None, pattern, handle, wrap, knob, ferrule }
}

pub fn def(id: WhipId) -> &'static WhipDef {
    CATALOGUE.iter().find(|w| w.id == id).expect("every WhipId is in CATALOGUE")
}

use Effect as E;
use Intent as I;
use Pattern as P;
use Screen as S;

const D: Params = Params::DEFAULT;

pub static CATALOGUE: [WhipDef; 15] = [
    WhipDef { id: WhipId::Classic, name: "Classic", blurb: "Navy plaited leather, ice-blue braid, silver fittings.",
        look: look(13.0, 2.2, 0x1B3A5C, 0x0A1830, 0x7FC4EC, 0x7FC4EC, 0xBFE6FF, 0xE8F4FF, P::Braid, 0x0F2440, 0x7FC4EC, 0xC9D6E2, 0xC9D6E2),
        tip: Tip::Frayed, effect: E::Sparks, intent: I::Faster, cracks: &sound::LEATHER, sounds: &[],
        lines: &["Crack! Straight line, no detours.", "Snap! One path, straight ahead.", "Crack! Short trail, clear road.", "Eyes on the shortest route.", "Straight to the point:"],
        physics: D,
        vibe: Vibe { hue: 0x7FC4EC, screen: S::Glow, shout: "CRACK" } },
    WhipDef { id: WhipId::Neon, name: "Neon", blurb: "Glowing tube, clean light bead. Floods the screen with colour.",
        look: Look { glow: Some(0x33D6FF), ..look(9.0, 2.5, 0xD9FBFF, 0x22C8F0, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFF5BD7, P::Plain, 0x101624, 0x33D6FF, 0x33D6FF, 0xFF5BD7) },
        tip: Tip::Bead, effect: E::Flash, intent: I::Focus, cracks: &sound::LEATHER, sounds: &[sound::NEON],
        lines: &["Lights on! Spotlight on one thing.", "Bzzzt! One bright beam, one target.","Flash! Look here, not over there.", "Insert coin: one clean focus, please.", "Zap! Back to the main quest.", "Buzz, flash, focus. Now:"],
        physics: Params { weight: 0.8, ..D },
        vibe: Vibe { hue: 0x33D6FF, screen: S::Flicker, shout: "ZAP" } },
    WhipDef { id: WhipId::Elephant, name: "Elephant", blurb: "Heavy grey hide, tufted tail tip. Thumps the screen.",
        look: look(21.0, 4.0, 0x8B8F96, 0x454950, 0x62676F, 0x62676F, 0xC8CCD2, 0xC8CCD2, P::Rings, 0xEFE8D8, 0xCBBFA6, 0xEFE8D8, 0x8B8F96),
        tip: Tip::Tail, effect: E::Thump, intent: I::Harder, cracks: &sound::LEATHER, sounds: &[sound::ELEPHANT],
        lines: &["Trunk swing! Move the mountain, step by step.", "Stomp stomp. Heavy lifting time.", "Pachyderm power engaged. Every step firm.", "Solid footing under every step.", "Trumpet blast! Load up and carry it.", "Boom. Heavy steps, solid ground:"],
        physics: Params { points: 24, seg_len: 28.0, gravity: 1.6, damping: 0.955, tip_damping: 0.95, weight: 1.8, ..D },
        vibe: Vibe { hue: 0xF0C674, screen: S::Jolt, shout: "STOMP" } },
    WhipDef { id: WhipId::Cowboy, name: "Cowboy", blurb: "Tan rawhide braid, brass knob. Dust and a lasso swirl.",
        look: look(12.0, 2.0, 0xB97A43, 0x5E3519, 0xE8B983, 0xE8B983, 0xF4D8AE, 0xF4D8AE, P::Braid, 0x7A4A26, 0xE8B983, 0xC9A04A, 0xC9A04A),
        tip: Tip::Frayed, effect: E::Lasso, intent: I::Faster, cracks: &sound::LEATHER, sounds: &[sound::COWBOY],
        lines: &["Giddy-up, partner! Straight trail ahead.", "Yeehaw! Straight trail, no detours:", "Main road, no side canyons.", "Dust rises on the short trail.", "Saddle up, one trail only.", "Yonder's the finish line, straight ahead."],
        physics: Params { points: 30, seg_len: 23.0, weight: 1.2, ..D },
        vibe: Vibe { hue: 0xE8B983, screen: S::Glow, shout: "YEEHAW" } },
    WhipDef { id: WhipId::Cheerleader, name: "Cheerleader", blurb: "Pink-and-gold ribbon, pom-pom tip. Confetti and cheers.",
        look: look(11.0, 2.5, 0xFF8CC6, 0xC93E86, 0xFFFFFF, 0xFFD25A, 0xFFE3F1, 0xFFD25A, P::Ribbon, 0xFFD25A, 0xFF8CC6, 0xFF8CC6, 0xFFFFFF),
        tip: Tip::PomPom, effect: E::Confetti, intent: I::Happier, cracks: &sound::LEATHER, sounds: &[sound::CHEER],
        lines: &["Two, four, six, eight, steady does it!", "Pom-poms up! Steady pace, honest scoreboard:", "Rah rah! We cheer for true results:", "Ready, okay! Calm pace, clear report.", "Confetti in the air, steady beat.", "Cheer, cheer, steady as we go:"],
        physics: Params { damping: 0.972, stiff_tip: 0.04, weight: 0.7, ..D },
        vibe: Vibe { hue: 0xFF7CC8, screen: S::Sides, shout: "RAH" } },
    WhipDef { id: WhipId::Sauce, name: "Sauce", blurb: "Glossy salsa with a hanging drop. Fills Claude up.",
        look: look(12.0, 3.0, 0xD93A22, 0x7E1A0C, 0xFFF3E0, 0x5FA84A, 0xFFB3A0, 0xD93A22, P::Chunks, 0xE9C98F, 0xD93A22, 0x5FA84A, 0xC8CCD2),
        tip: Tip::Drip, effect: E::Splash, intent: I::Happier, cracks: &sound::LEATHER, sounds: &[],
        lines: &["Mmm, fresh salsa. Fed and steady.", "Full belly, clear head.", "Warm salsa, easy pace.", "That hit the spot. Now:", "Fuelled up and cosy:"],
        physics: Params { gravity: 1.45, damping: 0.96, weight: 1.3, ..D },
        vibe: Vibe { hue: 0xFF8A4A, screen: S::Wobble, shout: "YUM" } },
    WhipDef { id: WhipId::DrillSergeant, name: "Drill Sergeant", blurb: "Olive webbing, brass buckle. A stern red “!”.",
        look: look(12.0, 2.4, 0x6B7A3A, 0x2F3818, 0xA9B97A, 0xA9B97A, 0xC9D59E, 0xA9B97A, P::Braid, 0x1E1F1B, 0x6B7A3A, 0x1E1F1B, 0xC9A04A),
        tip: Tip::Frayed, effect: E::Exclaim, intent: I::Harder, cracks: &sound::LEATHER, sounds: &[sound::DRILL],
        lines: &["Attention! No shortcuts on my watch.", "Inspection time! Every corner, by the book:", "Detail first. Every bolt, every seam.", "Chin up, shoulders back, eyes on detail.", "Boots polished, buttons checked, all of it:"],
        // a greener olive than the lash's 0xC9D59E: clear of the Elephant's gold, the other Jolt
        physics: Params { stiff_handle: 0.92, weight: 1.2, ..D },
        vibe: Vibe { hue: 0xBFD98F, screen: S::Jolt, shout: "HUP" } },
    WhipDef { id: WhipId::Zen, name: "Zen", blurb: "Silk thread on bamboo. An ink ripple and a falling petal.",
        look: look(6.0, 1.5, 0xF3EBDD, 0x7A6A55, 0xD8CDB8, 0xD8CDB8, 0xFFFFFF, 0x7A6A55, P::Plain, 0xC9B26E, 0x8A7A3E, 0xC9B26E, 0x8A7A3E),
        tip: Tip::Frayed, effect: E::Ripple, intent: I::Careful, cracks: &sound::LEATHER, sounds: &[sound::GONG],
        lines: &["Breathe in. Look twice. Then act.", "Slow hands, steady mind.", "Pause. One careful step at a time.", "Empty the cup. Read before you write:", "Gentle with the code, sure with changes."],
        physics: Params { seg_len: 22.0, gravity: 1.2, stiff_tip: 0.1, damping: 0.96, tip_damping: 0.93, weight: 0.6, ..D },
        vibe: Vibe { hue: 0xFFC2D6, screen: S::Fall, shout: "OM" } },
    WhipDef { id: WhipId::Ninja, name: "Ninja", blurb: "Black cord, steel kunai tip. A clean slash and smoke.",
        look: look(9.0, 2.0, 0x1E1F24, 0x000000, 0x4A4D57, 0x4A4D57, 0x6E7280, 0xC8CCD2, P::Braid, 0x1E1F24, 0xB32B2B, 0x1E1F24, 0xC8CCD2),
        tip: Tip::Kunai, effect: E::Slash, intent: I::Focus, cracks: &sound::LEATHER, sounds: &[sound::NINJA],
        lines: &["Shadow strike. One target, nothing else.", "A true ninja leaves no extra footprints.", "Thwip! The side quests have vanished.", "One clean cut, nothing extra.", "Precision is the whole art. Now:"],
        physics: Params { points: 22, seg_len: 22.0, damping: 0.97, weight: 0.7, ..D },
        vibe: Vibe { hue: 0x7C8CFF, screen: S::Sides, shout: "SLICE" } },
    WhipDef { id: WhipId::Pirate, name: "Pirate", blurb: "Tarred hemp, cutlass hilt. Sea spray and a cannon puff.",
        look: look(12.0, 2.4, 0x6B4E2E, 0x2E2010, 0xA88B5E, 0xA88B5E, 0xC9B08A, 0xC9B08A, P::Braid, 0x3A2A18, 0xC9A04A, 0xC9A04A, 0xC9A04A),
        tip: Tip::Frayed, effect: E::Spray, intent: I::Faster, cracks: &sound::LEATHER, sounds: &[sound::PIRATE],
        lines: &["Arr! One course, no island-hopping:", "Wind's up, matey! Straight course ahead.", "Steady wind, one heading, sails trimmed.", "X marks the spot, straight ahead.", "Cannon's loaded. Aim straight:"],
        physics: Params { weight: 1.3, ..D },
        vibe: Vibe { hue: 0x3FD9B4, screen: S::Wobble, shout: "ARR" } },
    WhipDef { id: WhipId::Chef, name: "Chef", blurb: "Kitchen-towel lash, wooden spoon. Sizzle and steam.",
        look: look(11.0, 2.5, 0xF4F1EA, 0x9A938A, 0xD9D2C5, 0xD9D2C5, 0xFFFFFF, 0x9A938A, P::Rings, 0xC49A5E, 0x8A6A3E, 0xC49A5E, 0xC8CCD2),
        tip: Tip::Frayed, effect: E::Sizzle, intent: I::Careful, cracks: &sound::LEATHER, sounds: &[],
        lines: &["Taste it before you serve it, chef.", "Mise en place: check everything is ready first.", "Hot pan, steady hands, clean plate.", "Chef says: read the recipe twice.", "Order up, but only after checking. Now:"],
        physics: D,
        vibe: Vibe { hue: 0xFFB03B, screen: S::Rise, shout: "SIZZLE" } },
    WhipDef { id: WhipId::Wizard, name: "Wizard", blurb: "A ribbon of starlight, crystal bead. A magic circle.",
        look: Look { glow: Some(0x9DB4FF), ..look(9.0, 2.0, 0xBFD3FF, 0x4B5DA8, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xC7A8FF, P::Plain, 0x6B4A2E, 0xC7A8FF, 0xC7A8FF, 0xC9A04A) },
        tip: Tip::Bead, effect: E::Magic, intent: I::Focus, cracks: &sound::LEATHER, sounds: &[],
        lines: &["Abracadabra! Now, one spell at a time.", "By starlight, a single bright beam.", "Poof! All the distractions have vanished.", "Merlin's staff points one way.", "Hocus pocus, eyes on the focus:"],
        physics: Params { weight: 0.8, ..D },
        vibe: Vibe { hue: 0xB48CFF, screen: S::Glow, shout: "POOF" } },
    WhipDef { id: WhipId::Vine, name: "Vine", blurb: "A living vine with leaves. The bud blooms on a crack.",
        look: look(11.0, 2.4, 0x4E9A4B, 0x24572A, 0x86C77A, 0x86C77A, 0xC8F0B8, 0x86C77A, P::Braid, 0x7A5A3A, 0x4E9A4B, 0x7A5A3A, 0xC9A04A),
        tip: Tip::Bloom, effect: E::Petals, intent: I::Careful, cracks: &[sound::VINE_CRACK], sounds: &[],
        lines: &["Grow it slowly, root it well.", "Good gardens are checked daily.", "Water first, then prune.", "A careful sprout becomes a tree:", "Tend it gently. Now:"],
        physics: Params { stiff_tip: 0.1, weight: 0.9, ..D },
        vibe: Vibe { hue: 0x86D77A, screen: S::Rise, shout: "BLOOM" } },
    WhipDef { id: WhipId::Penguin, name: "Penguin", blurb: "A clay penguin rides the tip. Honk honk, snow puff.",
        look: look(12.0, 3.0, 0x2B3247, 0x0B0E16, 0xFFFFFF, 0xFF8A2B, 0x8DA2C8, 0xFF8A2B, P::Ribbon, 0xFFFFFF, 0x2B3247, 0xFF8A2B, 0xFF8A2B),
        tip: Tip::Penguin, effect: E::Snow, intent: I::Faster, cracks: &sound::LEATHER, sounds: &[],
        lines: &["Honk honk! One straight line across the ice:", "Belly slide, straight down the slope.", "Honk honk! Smooth ice, one clear path.", "Honk! The shortest path to the fish.", "Waddle waddle, straight ahead:"],
        physics: Params { points: 22, weight: 1.1, ..D },
        vibe: Vibe { hue: 0xCFE8FF, screen: S::Fall, shout: "HONK" } },
    WhipDef { id: WhipId::Storm, name: "Storm", blurb: "Thundercloud tip. Lightning across the screen.",
        look: look(10.0, 2.4, 0x6C7FA6, 0x2E3A58, 0xE8EEF8, 0xE8EEF8, 0xE8EEF8, 0xFFD84A, P::Braid, 0x2E3A58, 0xFFD84A, 0xE8EEF8, 0xFFD84A),
        tip: Tip::Cloud, effect: E::Lightning, intent: I::Harder, cracks: &sound::LEATHER, sounds: &[],
        lines: &["Rumble. Steady rain reaches every root:", "The storm doesn't do halfway.", "Lightning finds every weak spot.", "Thunder rolls across the whole sky.", "Dark clouds, steady downpour:"],
        physics: Params { weight: 1.1, ..D },
        vibe: Vibe { hue: 0xFFD84A, screen: S::Flicker, shout: "BOOM" } },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_covers_every_id_once() {
        for id in ALL {
            assert_eq!(CATALOGUE.iter().filter(|w| w.id == id).count(), 1, "{id:?}");
        }
    }

    /// The tone rules: flavour only, no pressure, blame or second instruction.
    #[test]
    fn fun_lines_follow_the_tone_rules() {
        const BANNED: [&str; 18] = [
            "hurry", "quick", "quicker", "fast", "faster", "fastest", "schedule", "behind", "deadline", "or else",
            "finishes first", "wins", "counting on", "i said", "lazy", "never", "ultrathink", "normal mode",
        ];
        for w in &CATALOGUE {
            assert!(w.lines.len() >= 5, "{}: fewer than 5 lines", w.name);
            assert!(w.lines.iter().any(|l| l.ends_with(':')), "{}: no line leads into the instruction with ':'", w.name);
            for l in w.lines {
                let ctx = format!("{}: {l}", w.name);
                assert!(l.split_whitespace().count() <= 8, "{ctx}");
                assert!(l.is_ascii(), "{ctx}");
                assert!(l.chars().next().is_some_and(|c| c.is_alphabetic()), "{ctx}");
                assert!(!l.contains('?') && !l.contains('@'), "{ctx}");
                assert!(l.matches('!').count() <= 1, "{ctx}");
                let words: Vec<String> = l.split(|c: char| !c.is_ascii_alphanumeric() && c != '\'').filter(|s| !s.is_empty()).map(str::to_string).collect();
                assert!(!words.iter().any(|x| x.len() >= 4 && x.chars().all(|c| c.is_ascii_uppercase())), "{ctx}");
                let padded = format!(" {} ", words.join(" ").to_lowercase());
                for b in BANNED {
                    assert!(!padded.contains(&format!(" {b} ")), "{ctx} (banned: {b})");
                }
            }
        }
    }

    #[test]
    fn no_whip_glows_red() {
        for w in &CATALOGUE {
            let h = w.vibe.hue;
            let (r, g, b) = ((h >> 16) & 255, (h >> 8) & 255, h & 255);
            assert!(g.max(b) * 2 > r, "{}: {h:06X} reads as red", w.name);
        }
    }

    /// The combo label draws only the glyphs `Glyphs` keeps (' ' to 'Z'); any other character is left out.
    #[test]
    fn shouts_use_only_the_label_glyphs() {
        for w in &CATALOGUE {
            let s = w.vibe.shout;
            assert!(!s.is_empty() && s.chars().all(|c| (' '..='Z').contains(&c)), "{}: {s}", w.name);
        }
    }

    /// Hue angle of a colour, 0..360.
    fn hue_deg(rgb: u32) -> f32 {
        let (r, g, b) = (((rgb >> 16) & 255) as f32, ((rgb >> 8) & 255) as f32, (rgb & 255) as f32);
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let d = (max - min).max(1.0);
        let h = if max == r { (g - b) / d } else if max == g { 2.0 + (b - r) / d } else { 4.0 + (r - g) / d };
        (h * 60.0).rem_euclid(360.0)
    }

    /// Two whips may share a hue family (within 40 degrees) or a screen behaviour, never both.
    #[test]
    fn no_two_whips_share_hue_family_and_behaviour() {
        for (i, a) in CATALOGUE.iter().enumerate() {
            for b in &CATALOGUE[i + 1..] {
                let (va, vb) = (a.vibe, b.vibe);
                let apart = (hue_deg(va.hue) - hue_deg(vb.hue)).abs();
                let apart = apart.min(360.0 - apart);
                assert!(va.screen != vb.screen || apart > 40.0, "{} and {}: both {:?}, hues {apart:.0} degrees apart", a.name, b.name, va.screen);
            }
        }
    }

    #[test]
    fn physics_params_are_sane() {
        for w in &CATALOGUE {
            let p = w.physics;
            assert!(p.points >= 16 && p.seg_len > 0.0 && p.damping < 1.0, "{}", w.name);
            assert!((0.5..=2.0).contains(&p.weight), "{}", w.name);
        }
    }
}
