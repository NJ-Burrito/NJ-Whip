//! Sounds: each whip's own crack (one of its clips at random per crack; the Classic whip has the
//! five leather cracks), a voice or effect clip some whips play on top of the crack, and what the
//! user chose instead: any of the crack sounds we have, and one sound file of their own (at most 3 s).

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::whips::{self, WhipDef};

macro_rules! clip {
    ($name:ident, $file:literal) => {
        pub const $name: &[u8] = include_bytes!(concat!("../assets/sounds/", $file));
    };
}
include!("clips.rs");

/// The output stream is closed this long after the last sound (an open WASAPI stream keeps the
/// audio thread awake and can block idle sleep). The longest crack sound is well under this.
const KEEP_OPEN:Duration = Duration::from_secs(3);

const RATE: std::num::NonZero<u32> = std::num::NonZero::new(44_100).unwrap();
const MONO: std::num::NonZero<u16> = std::num::NonZero::new(1).unwrap();

/// A whoosh made on the spot: noise through a low-pass filter that opens and closes with the
/// swing. No recording, so nothing to license.
fn whoosh_samples(weight: f32) -> Vec<f32> {
    let weight = weight.clamp(0.5, 2.0);
    let n = (RATE.get() as f32 * (0.14 + 0.10 * weight)) as usize;
    let (mut lp, mut lp2) = (0.0f32, 0.0f32);
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let env = (std::f32::consts::PI * t).sin().powf(1.6);
            let cut = (0.03 + 0.20 * env) / weight.sqrt();
            lp += (fastrand::f32() * 2.0 - 1.0 - lp) * cut;
            lp2 += (lp - lp2) * cut;
            (lp * 0.5 + (lp - lp2) * 1.1) * env
        })
        .collect()
}

fn expired(last: Option<Instant>, now: Instant) -> bool {
    last.is_some_and(|t| now.saturating_duration_since(t) >= KEEP_OPEN)
}

/// A sound is only played (and the device only opened) when its final volume is above zero.
fn audible(volume: f32) -> bool {
    volume > 0.0
}

fn whoosh_volume(volume: f32, power: f32) -> f32 {
    volume * (0.2 + 0.5 * power.clamp(0.0, 1.0))
}

/// A whip's own clips (a trumpet, a "yeehaw") would pile up over a streak of cracks: after
/// they play they rest until they have ended and this long has passed. Cracks in between are
/// cracks only.
const EXTRAS_REST: Duration = Duration::from_secs(1);

fn extras_due(rest_until: Option<Instant>, now: Instant) -> bool {
    rest_until.is_none_or(|t| now >= t)
}

/// Sound bytes: built into the exe, or read from the user's own file. Cloning shares the bytes.
#[derive(Clone)]
pub enum Clip {
    Static(&'static [u8]),
    Owned(Arc<[u8]>),
}

impl AsRef<[u8]> for Clip {
    fn as_ref(&self) -> &[u8] {
        match self {
            Clip::Static(b) => b,
            Clip::Owned(b) => b,
        }
    }
}

/// The longest a custom sound may be.
pub const SFX_MAX: Duration = Duration::from_secs(3);
/// A custom sound file bigger than this is refused before it is read.
const SFX_MAX_BYTES: u64 = 16 << 20;
/// File types a custom sound may have (what the decoder is built with).
pub const SFX_EXTENSIONS: [&str; 4] = ["mp3", "wav", "ogg", "flac"];

/// How long a clip plays; at most `cap` (plus one sample) is decoded. None when it does not
/// decode or holds no sound.
fn measure(clip: Clip, cap: Duration) -> Option<Duration> {
    let src = Decoder::try_from(Cursor::new(clip)).ok()?;
    let per_second = src.sample_rate().get() as f64 * src.channels().get() as f64;
    let room = (per_second * cap.as_secs_f64()) as usize + 1;
    let n = src.take(room).count();
    (n > 0).then(|| Duration::from_secs_f64(n as f64 / per_second))
}

/// One crack sound we have, by a stable name (what config.toml stores).
pub struct CrackSound {
    /// The constant in clips.rs, for the test that every crack clip is listed.
    ident: &'static str,
    pub name: &'static str,
    pub label: &'static str,
    pub bytes: &'static [u8],
}

macro_rules! crack_list {
    ($(($id:ident, $name:literal, $label:literal)),* $(,)?) => {
        /// Every crack sound the exe holds: the five leather cracks and each whip's own.
        pub const CRACK_SOUNDS: &[CrackSound] =
            &[$(CrackSound { ident: stringify!($id), name: $name, label: $label, bytes: $id }),*];
    };
}
crack_list![
    (CRACK_1, "leather-1", "Leather 1"),
    (CRACK_2, "leather-2", "Leather 2"),
    (CRACK_3, "leather-3", "Leather 3"),
    (CRACK_4, "leather-4", "Leather 4"),
    (CRACK_5, "leather-5", "Leather 5"),
    (NEON_CRACK, "neon", "Neon"),
    (ELEPHANT_CRACK, "elephant", "Elephant"),
    (COWBOY_CRACK, "cowboy", "Cowboy"),
    (CHEER_CRACK, "cheer", "Cheer"),
    (SAUCE_CRACK, "sauce", "Sauce"),
    (DRILL_CRACK, "drill", "Drill"),
    (ZEN_CRACK, "zen", "Zen"),
    (NINJA_CRACK, "ninja", "Ninja"),
    (PIRATE_CRACK, "pirate", "Pirate"),
    (CHEF_CRACK, "chef", "Chef"),
    (WIZARD_CRACK, "wizard", "Wizard"),
    (VINE_CRACK, "vine", "Vine"),
    (PENGUIN_CRACK, "penguin", "Penguin"),
    (STORM_CRACK, "storm", "Storm"),
];

/// The whip's own cracks, as listed sounds.
pub fn default_cracks(def: &WhipDef) -> Vec<&'static CrackSound> {
    def.cracks.iter().filter_map(|b| CRACK_SOUNDS.iter().find(|s| s.bytes == *b)).collect()
}

/// The cracks that play for a whip: the user's picked names (unknown names are dropped), or the
/// whip's own when none is picked or none is known.
pub fn chosen_cracks(def: &WhipDef, picked: Option<&[String]>) -> Vec<&'static CrackSound> {
    let set: Vec<_> = picked
        .unwrap_or_default()
        .iter()
        .filter_map(|n| CRACK_SOUNDS.iter().find(|s| s.name == n))
        .collect();
    if set.is_empty() { default_cracks(def) } else { set }
}

/// What the audio thread plays for the current whip. Replaced whole when the settings change.
#[derive(Clone, Default)]
pub struct Loadout {
    cracks: Vec<Clip>,
    extras: Vec<Clip>,
    /// The longest extra, measured once here.
    extras_len: Duration,
}

pub fn sfx_dir() -> PathBuf {
    crate::config::config_path().with_file_name("sfx")
}

/// The user's sound file for a whip, read whole (None when missing or too big).
pub fn load_sfx(path: &Path) -> Option<Clip> {
    if std::fs::metadata(path).ok()?.len() > SFX_MAX_BYTES {
        return None;
    }
    Some(Clip::Owned(std::fs::read(path).ok()?.into()))
}

/// Builds the loadout of the current whip from the settings: the chosen cracks, and the user's
/// own sound as the whip's extra in place of the built-in ones.
pub fn loadout(cfg: &Config) -> Loadout {
    let def = whips::def(cfg.whip);
    let cracks = chosen_cracks(def, cfg.custom_cracks.get(&cfg.whip).map(Vec::as_slice))
        .into_iter()
        .map(|s| Clip::Static(s.bytes))
        .collect();
    let extras: Vec<Clip> = match cfg.custom_sfx.get(&cfg.whip).and_then(|p| load_sfx(p)) {
        Some(own) => vec![own],
        None => def.sounds.iter().map(|b| Clip::Static(b)).collect(),
    };
    let extras_len = extras.iter().filter_map(|c| measure(c.clone(), Duration::from_secs(60))).max().unwrap_or_default();
    Loadout { cracks, extras, extras_len }
}

/// Why a picked sound file was refused. `line()` is the English message (the window translates it).
#[derive(Debug, PartialEq)]
pub enum SfxError {
    Type,
    TooBig,
    Unreadable,
    NotAudio,
    TooLong,
}

impl SfxError {
    pub fn line(&self) -> &'static str {
        match self {
            SfxError::Type => "That file type is not supported. Use mp3, wav, ogg or flac.",
            SfxError::TooBig => "That file is too big for a short sound.",
            SfxError::Unreadable => "That file could not be read.",
            SfxError::NotAudio => "That file is not a sound that can be played.",
            SfxError::TooLong => "That sound is longer than 3 seconds. Pick a shorter one.",
        }
    }
}

/// Checks a picked file (type, size, decodes, at most `SFX_MAX`) and copies it to
/// `dir/<name>.<ext>`, replacing any earlier copy for that name. Returns the copy's path.
pub fn import_sfx(src: &Path, dir: &Path, name: &str) -> Result<PathBuf, SfxError> {
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .filter(|e| SFX_EXTENSIONS.contains(&e.as_str()))
        .ok_or(SfxError::Type)?;
    let clip = load_sfx(src).ok_or_else(|| match std::fs::metadata(src) {
        Ok(m) if m.len() > SFX_MAX_BYTES => SfxError::TooBig,
        _ => SfxError::Unreadable,
    })?;
    let len = measure(clip.clone(), SFX_MAX).ok_or(SfxError::NotAudio)?;
    if len > SFX_MAX {
        return Err(SfxError::TooLong);
    }
    std::fs::create_dir_all(dir).map_err(|_| SfxError::Unreadable)?;
    remove_sfx(dir, name);
    let dest = dir.join(format!("{name}.{ext}"));
    std::fs::write(&dest, clip.as_ref()).map_err(|_| SfxError::Unreadable)?;
    Ok(dest)
}

/// Removes the app's copies of a whip's sound (every type), so none piles up.
pub fn remove_sfx(dir: &Path, name: &str) {
    for ext in SFX_EXTENSIONS {
        let _ = std::fs::remove_file(dir.join(format!("{name}.{ext}")));
    }
}

enum Cmd {
    /// One random crack of the loadout, plus its extras unless they are resting; `always` plays
    /// the extras even while resting (a whip was picked or tested).
    Crack(bool),
    /// Replace the loadout (the old one is dropped).
    Loadout(Loadout),
    /// One clip, by hand (a preview in the window); `extra` = at the extras' volume.
    Play(Clip, bool),
    Whoosh(f32, f32),
    Volume(f32, f32),
}

/// Thin handle: all audio work (device, decoding, synthesis) runs on its own thread so the
/// engine's frame thread never waits on it. The thread ends when this handle is dropped.
pub struct Sound {
    tx: Sender<Cmd>,
}

impl Sound {
    pub fn new(volume: f32, sfx_volume: f32) -> Self {
        let (tx, rx) = mpsc::channel();
        // If the thread cannot start, `rx` is dropped and every send fails silently: no sound.
        let _ = std::thread::Builder::new().name("nj-whip-audio".into()).spawn(move || {
            Audio { sink: None, last_used: None, volume, sfx_volume, extras_rest: None, loadout: Loadout::default() }.run(rx)
        });
        Sound { tx }
    }

    pub fn set_volume(&self, volume: f32, sfx_volume: f32) {
        let _ = self.tx.send(Cmd::Volume(volume, sfx_volume));
    }

    pub fn set_loadout(&self, loadout: Loadout) {
        let _ = self.tx.send(Cmd::Loadout(loadout));
    }

    /// One random crack of the loadout, plus the whip's extras on top unless they are resting
    /// (see `EXTRAS_REST`).
    pub fn crack(&self) {
        let _ = self.tx.send(Cmd::Crack(false));
    }

    /// A whip's whole sound, asked for by hand (picked or tested in the window): always in full.
    pub fn preview(&self) {
        let _ = self.tx.send(Cmd::Crack(true));
    }

    /// One clip on its own (`extra`: at the whips' own sounds' volume).
    pub fn play(&self, clip: Clip, extra: bool) {
        let _ = self.tx.send(Cmd::Play(clip, extra));
    }

    /// The whip cutting the air before a crack. `power` 0..1 = how hard the swing is;
    /// a heavier whip (`weight`, 1 = normal) sounds longer and lower.
    pub fn whoosh(&self, power: f32, weight: f32) {
        let _ = self.tx.send(Cmd::Whoosh(power, weight));
    }
}

struct Audio {
    /// Opened by the first audible sound after a quiet spell, dropped after `KEEP_OPEN` idle;
    /// reopening also follows a changed default device. `None` when closed or no audio device.
    sink: Option<MixerDeviceSink>,
    last_used: Option<Instant>,
    volume: f32,
    sfx_volume: f32,
    /// The whips' own clips play again from this moment on.
    extras_rest: Option<Instant>,
    loadout: Loadout,
}

impl Audio {
    fn run(mut self, rx: Receiver<Cmd>) {
        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(cmd) => self.handle(cmd),
                Err(RecvTimeoutError::Timeout) => {
                    if self.sink.is_some() && expired(self.last_used, Instant::now()) {
                        self.sink = None;
                        self.last_used = None;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Volume(v, sfx) => (self.volume, self.sfx_volume) = (v, sfx),
            Cmd::Loadout(l) => {
                self.loadout = l;
                self.extras_rest = None;
            }
            Cmd::Play(clip, extra) => self.play_bytes(clip, if extra { self.sfx_volume } else { self.volume }),
            Cmd::Crack(always) => {
                if !self.loadout.cracks.is_empty() {
                    let clip = self.loadout.cracks[fastrand::usize(..self.loadout.cracks.len())].clone();
                    self.play_bytes(clip, self.volume);
                }
                let now = Instant::now();
                if !self.loadout.extras.is_empty() && (always || extras_due(self.extras_rest, now)) {
                    for clip in self.loadout.extras.clone() {
                        self.play_bytes(clip, self.sfx_volume);
                    }
                    self.extras_rest = Some(now + self.loadout.extras_len + EXTRAS_REST);
                }
            }
            Cmd::Whoosh(power, weight) => {
                let volume = whoosh_volume(self.volume, power);
                if !audible(volume) {
                    return;
                }
                if let Some(player) = self.player(volume) {
                    player.append(SamplesBuffer::new(MONO, RATE, whoosh_samples(weight)));
                    player.detach();
                }
            }
        }
    }

    fn play_bytes(&mut self, clip: Clip, volume: f32) {
        if !audible(volume) {
            return;
        }
        let Ok(src) = Decoder::try_from(Cursor::new(clip)) else { return };
        if let Some(player) = self.player(volume) {
            player.append(src);
            player.detach();
        }
    }

    /// A player on the (opened on demand) output stream; None when there is no audio device.
    fn player(&mut self, volume: f32) -> Option<Player> {
        if self.sink.is_none() {
            self.sink = DeviceSinkBuilder::open_default_sink().ok().map(|mut s| {
                s.log_on_drop(false);
                s
            });
        }
        let sink = self.sink.as_ref()?;
        self.last_used = Some(Instant::now());
        let player = Player::connect_new(sink.mixer());
        player.set_volume(volume);
        Some(player)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::whips::{WhipId, ALL};

    #[test]
    fn whoosh_is_short_quiet_at_both_ends_and_longer_when_heavy() {
        let (light, heavy) = (whoosh_samples(0.6), whoosh_samples(1.8));
        assert!(heavy.len() > light.len());
        for w in [&light, &heavy] {
            assert!(w.len() < RATE.get() as usize / 2, "under half a second");
            assert!(w.iter().all(|s| s.abs() <= 1.0));
            assert!(w[0].abs() < 0.01 && w[w.len() - 1].abs() < 0.01, "no click at the ends");
            assert!(w.iter().any(|s| s.abs() > 0.05), "audible");
        }
    }

    #[test]
    fn zero_volume_is_not_audible() {
        assert!(!audible(0.0) && !audible(-0.1) && !audible(f32::NAN));
        assert!(audible(0.01));
        // muted crack volume mutes the whoosh at any power; otherwise it stays audible
        assert!(!audible(whoosh_volume(0.0, 1.0)));
        assert!(audible(whoosh_volume(0.5, 0.0)));
    }

    #[test]
    fn closes_only_after_keep_open() {
        let t0 = Instant::now();
        assert!(!expired(None, t0));
        assert!(!expired(Some(t0), t0 + Duration::from_millis(2900)));
        assert!(expired(Some(t0), t0 + KEEP_OPEN));
    }

    /// A mono 16-bit PCM wav of silence-with-a-tone, `secs` long.
    fn wav(secs: f32) -> Vec<u8> {
        let rate = 8000u32;
        let n = (rate as f32 * secs) as u32;
        let mut b = Vec::new();
        b.extend(b"RIFF");
        b.extend((36 + n * 2).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend([1, 0, 1, 0]);
        b.extend(rate.to_le_bytes());
        b.extend((rate * 2).to_le_bytes());
        b.extend([2, 0, 16, 0]);
        b.extend(b"data");
        b.extend((n * 2).to_le_bytes());
        for i in 0..n {
            b.extend((((i % 40) as i16 - 20) * 300).to_le_bytes());
        }
        b
    }

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("njw-sfx-{}-{name}", std::process::id()))
    }

    #[test]
    fn custom_sound_over_three_seconds_is_refused_and_a_short_one_kept() {
        let dir = tmp("import");
        std::fs::create_dir_all(&dir).unwrap();
        let (short, long, edge) = (dir.join("short.wav"), dir.join("long.wav"), dir.join("edge.wav"));
        std::fs::write(&short, wav(1.0)).unwrap();
        std::fs::write(&long, wav(3.5)).unwrap();
        std::fs::write(&edge, wav(3.0)).unwrap();
        let out = dir.join("sfx");

        assert_eq!(import_sfx(&long, &out, "zen"), Err(SfxError::TooLong));
        assert!(!out.join("zen.wav").exists(), "a refused file is not copied");
        let kept = import_sfx(&short, &out, "zen").unwrap();
        assert_eq!(kept, out.join("zen.wav"));
        assert_eq!(std::fs::read(&kept).unwrap(), wav(1.0));
        assert!(import_sfx(&edge, &out, "zen").is_ok(), "exactly 3 s is allowed");

        // not a sound, wrong type
        let junk = dir.join("junk.mp3");
        std::fs::write(&junk, b"this is not audio at all").unwrap();
        assert_eq!(import_sfx(&junk, &out, "zen"), Err(SfxError::NotAudio));
        let txt = dir.join("a.txt");
        std::fs::write(&txt, b"x").unwrap();
        assert_eq!(import_sfx(&txt, &out, "zen"), Err(SfxError::Type));
        assert_eq!(import_sfx(&dir.join("none.wav"), &out, "zen"), Err(SfxError::Unreadable));

        remove_sfx(&out, "zen");
        assert!(!out.join("zen.wav").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_copy_replaces_the_old_one_of_another_type() {
        let dir = tmp("replace");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("neon.ogg"), b"old").unwrap();
        let src = dir.join("in.wav");
        std::fs::write(&src, wav(0.5)).unwrap();
        import_sfx(&src, &dir, "neon").unwrap();
        assert!(!dir.join("neon.ogg").exists() && dir.join("neon.wav").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every crack clip in clips.rs (`CRACK_n`, `*_CRACK`) is in the list, once, with a unique name.
    #[test]
    fn crack_list_covers_every_crack_clip() {
        let mut found: Vec<&str> = include_str!("clips.rs")
            .lines()
            .filter_map(|l| l.strip_prefix("clip!(")?.split(',').next())
            .filter(|id| id.ends_with("_CRACK") || id.starts_with("CRACK_"))
            .collect();
        let mut listed: Vec<&str> = CRACK_SOUNDS.iter().map(|s| s.ident).collect();
        found.sort();
        listed.sort();
        assert_eq!(found, listed);
        let mut names: Vec<_> = CRACK_SOUNDS.iter().map(|s| s.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), CRACK_SOUNDS.len(), "names are unique");
        assert!(CRACK_SOUNDS.iter().all(|s| !s.bytes.is_empty() && !s.label.is_empty()));
    }

    #[test]
    fn chosen_cracks_resolve_picked_names_or_the_whips_own() {
        let zen = whips::def(WhipId::Zen);
        let names = |v: Vec<&CrackSound>| v.iter().map(|s| s.name).collect::<Vec<_>>();
        assert_eq!(names(chosen_cracks(zen, None)), ["zen"]);
        assert_eq!(names(chosen_cracks(zen, Some(&[]))), ["zen"], "empty = default");
        let own = ["storm".to_string(), "leather-2".to_string(), "nope".to_string()];
        assert_eq!(names(chosen_cracks(zen, Some(&own))), ["storm", "leather-2"], "unknown names dropped");
        assert_eq!(names(chosen_cracks(zen, Some(&["nope".to_string()]))), ["zen"], "none known = default");
        assert_eq!(names(chosen_cracks(whips::def(WhipId::Classic), None)).len(), 5);
        // every whip's own cracks are all in the list
        for &id in ALL.iter() {
            let def = whips::def(id);
            assert_eq!(default_cracks(def).len(), def.cracks.len(), "{}", def.name);
        }
    }

    #[test]
    fn loadout_follows_the_settings_and_the_users_sound_replaces_the_built_in_extra() {
        let dir = tmp("loadout");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("mine.wav");
        std::fs::write(&file, wav(1.0)).unwrap();

        let mut cfg = Config { whip: WhipId::Zen, ..Config::default() };
        let base = loadout(&cfg);
        assert_eq!((base.cracks.len(), base.extras.len()), (1, 1), "Zen: its crack and its gong");
        assert!(base.extras_len > Duration::ZERO);

        cfg.custom_cracks.insert(WhipId::Zen, vec!["storm".into(), "neon".into(), "leather-1".into()]);
        cfg.custom_sfx.insert(WhipId::Zen, file.clone());
        let own = loadout(&cfg);
        assert_eq!(own.cracks.len(), 3);
        assert!(matches!(own.extras[..], [Clip::Owned(_)]), "the user's sound, not the gong");
        assert!((own.extras_len.as_secs_f32() - 1.0).abs() < 0.05);

        // a missing file falls back to the built-in extras
        cfg.custom_sfx.insert(WhipId::Zen, dir.join("gone.wav"));
        assert!(matches!(loadout(&cfg).extras[..], [Clip::Static(_)]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
