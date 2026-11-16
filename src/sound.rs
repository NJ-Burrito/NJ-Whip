//! Sounds embedded in the exe: each whip's own crack (one of its clips at random per crack; the
//! Classic whip has the five leather cracks) plus, for some whips, a voice or effect clip played
//! on top of the crack.

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

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

/// How long a clip plays.
fn clip_length(bytes: &'static [u8]) -> Duration {
    let Ok(src) = Decoder::try_from(Cursor::new(bytes)) else { return Duration::ZERO };
    let per_second = src.sample_rate().get() as f32 * src.channels().get() as f32;
    Duration::from_secs_f32(src.count() as f32 / per_second)
}

enum Cmd {
    /// The crack choices (one is played at random), the extras (all played, unless they are
    /// resting) and whether the extras play even while resting (a whip was picked or tested).
    Crack(&'static [&'static [u8]], &'static [&'static [u8]], bool),
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
        let _ = std::thread::Builder::new()
            .name("nj-whip-audio".into())
            .spawn(move || Audio { sink: None, last_used: None, volume, sfx_volume, extras_rest: None }.run(rx));
        Sound { tx }
    }

    pub fn set_volume(&self, volume: f32, sfx_volume: f32) {
        let _ = self.tx.send(Cmd::Volume(volume, sfx_volume));
    }

    /// One random clip of `cracks`, plus the whip's own clips (`extras`, if any) on top unless
    /// they are resting (see `EXTRAS_REST`).
    pub fn crack(&self, cracks: &'static [&'static [u8]], extras: &'static [&'static [u8]]) {
        let _ = self.tx.send(Cmd::Crack(cracks, extras, false));
    }

    /// A whip's whole sound, asked for by hand (picked or tested in the window): always in full.
    pub fn preview(&self, cracks: &'static [&'static [u8]], extras: &'static [&'static [u8]]) {
        let _ = self.tx.send(Cmd::Crack(cracks, extras, true));
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
            Cmd::Crack(cracks, extras, always) => {
                if !cracks.is_empty() {
                    self.play_bytes(cracks[fastrand::usize(..cracks.len())], self.volume);
                }
                let now = Instant::now();
                if !extras.is_empty() && (always || extras_due(self.extras_rest, now)) {
                    for bytes in extras {
                        self.play_bytes(bytes, self.sfx_volume);
                    }
                    let longest = extras.iter().map(|b| clip_length(b)).max().unwrap_or_default();
                    self.extras_rest = Some(now + longest + EXTRAS_REST);
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

    fn play_bytes(&mut self, bytes: &'static [u8], volume: f32) {
        if !audible(volume) {
            return;
        }
        let Ok(src) = Decoder::try_from(Cursor::new(bytes)) else { return };
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
}
