//! One engine frame: input, physics, effects, rendering and the push to the screen.

use super::*;

impl App {
    pub(super) fn frame(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        if self.overlay.move_to_cursor_monitor() {
            // The frame buffer was drawn for the old monitor (its work area may differ).
            self.canvas = None;
            if self.whip.is_none() {
                self.start_ghost();
            } else if self.whip.as_ref().is_some_and(|w| w.dropping) {
                // The cursor changed monitor. A whip already being dropped is hidden at once
                // (it was leaving anyway); otherwise restart the whip on the new monitor.
                self.hide_whip();
                return;
            } else {
                self.spawn_whip();
            }
        }
        let (w, h) = self.overlay.size();
        let def = whips::def(self.cfg.whip);
        let params = &def.physics;

        let mouse = self.overlay.cursor_local();
        let (mut whoosh, mut cracked) = (None, None);
        if let Some(whip) = &mut self.whip {
            let live = self.cfg.swing_crack
                && !whip.dropping
                && now.duration_since(whip.spawned) >= FIRST_CRACK_GRACE;
            let swing = &mut self.swing;
            each_step(&mut self.acc, dt, self.mouse_prev, mouse, |hand| {
                whip.rope.step(params, hand, (0.0, 0.0, w as f32, h as f32), whip.dropping);
                if !live {
                    return;
                }
                match swing.step(whip.rope.hand_vel().0) {
                    Some(Event::Whoosh { power, .. }) => whoosh = Some(power),
                    Some(Event::Crack { dir, .. }) => cracked = Some(dir),
                    None => {}
                }
            });
            let pts = &whip.rope.pts;
            if self.shown.len() != pts.len() || pts.iter().zip(&self.shown).any(|(a, b)| (a.0 - b.0).hypot(a.1 - b.1) > SHOW_MOVE) {
                self.shown.clone_from(pts);
            }
        }
        let mouse_still = mouse == self.mouse_prev;
        self.mouse_prev = mouse;
        if let Some(dir) = cracked {
            self.crack(false, dir);
        } else if let Some(power) = whoosh {
            self.sound.whoosh(power, params.weight);
        }

        self.fx.step(dt);
        self.heat.step(dt);
        let since = |m: &Mark| now.saturating_duration_since(m.when).as_secs_f32();
        let flash = self.mark.as_ref().filter(|m| m.lit).map_or(0.0, |m| (1.0 - since(m) / FLASH_FOR).max(0.0));
        let vibe = def.vibe;
        let t = now.duration_since(self.epoch).as_secs_f32();
        let (level, offset, edges) = vibe.screen.look(t, self.mark.as_ref().map(|m| (since(m), m.angle)));
        let heat_on = !self.heat.cold();

        if self.whip.is_none() && !self.lingering() {
            self.hide_overlay();
            return;
        }
        if self.canvas.as_ref().is_none_or(|c| c.size() != (w, h)) {
            self.canvas = Canvas::new(w, h);
        }
        // The canvas repaints only what differs from the last frame: a still scene costs nothing.
        let scene = Scene {
            whip: self.whip.as_ref().map(|_| (&self.shown[..], def, self.tip_img.as_ref().map(|tip| tip.frame_at(t)))),
            flash,
            glow: self.heat.glow() * level,
            edges,
            hue: vibe.hue,
            offset,
            scale: self.overlay.scale(),
            meter: (self.cfg.rage && (self.whip.is_some() || heat_on)).then(|| (self.heat.shown(), self.heat.full())),
            clock: t,
            combo: self.combo.visible(now).and_then(|age| {
                let m = self.mark.as_ref()?;
                let (path, width) = m.label.as_ref()?;
                Some(ComboView { path, width: *width, at: m.at, age, count: self.combo.count(), dir: m.dir })
            }),
            burst: Some(&self.fx),
        };
        let mut drew = true;
        if let Some(c) = &mut self.canvas {
            let dirty = c.draw(&scene, self.overlay.work_area());
            drew = !dirty.is_empty();
            self.overlay.present(&c.pm, &dirty);
        }

        if self.whip.as_ref().is_some_and(|w| w.dropping && w.rope.fallen_off(h as f32)) {
            self.hide_whip();
        }
        // Idle frame (nothing drawn, mouse still, whip hanging on the mouse): sleep until input
        // instead of waking for every compositor frame.
        if !drew && mouse_still && self.held() {
            // An animated tip ends the wait at its next frame; a still scene waits the full time.
            let wait = self.tip_img.as_ref().and_then(|tip| tip.next_change(t)).map_or(IDLE_WAIT_MS, |s| ((s * 1000.0).ceil() as u32).clamp(1, IDLE_WAIT_MS));
            unsafe { MsgWaitForMultipleObjects(None, false, wait, QS_ALLINPUT) };
        } else {
            vsync(now);
        }
    }
}
