//! A crack and the message it types into the last window.

use super::*;

impl App {
    /// One crack: sound, heat, combo, the lash thrown toward `dir` (-1 left, 1 right, 0 = as it
    /// lies), the message typed into the last window, the whip's effect at the tip.
    pub(super) fn crack(&mut self, from_hotkey: bool, dir: i8) {
        if !self.cfg.enabled {
            return;
        }
        let def = whips::def(self.cfg.whip);
        self.sound.crack();
        if self.cfg.rage {
            self.heat.crack();
        }
        let when = Instant::now();
        let count = self.combo.crack(when);
        let label = if self.combo.labelled() { self.glyphs.as_ref().and_then(|g| g.line(&format!("{} ×{count}", def.vibe.shout))) } else { None };
        let lit = self.held();
        if let Some(w) = self.whip.as_mut().filter(|_| lit) {
            w.rope.kick(dir);
        }
        self.type_message(from_hotkey);
        let at = self.start_burst();
        self.mark = Some(Mark { when, at, dir, angle: fastrand::f32() * std::f32::consts::TAU, lit, label });
    }

    /// Type the crack's message: after Esc (the macro as set) or queued (its Esc steps left out),
    /// by the whip's intent, the pack and the "Always press Esc first" switch.
    pub(super) fn type_message(&self, from_hotkey: bool) {
        let steps = if messages::after_esc(whips::def(self.cfg.whip).intent, self.cfg.style()) {
            self.cfg.macro_steps.clone()
        } else {
            queued(&self.cfg.macro_steps)
        };
        self.type_actions(from_hotkey, build_actions(&steps, &self.message()));
    }

    /// Send key actions to the last window on their own thread, so the whip never freezes while
    /// keys go out. Nothing is sent while an earlier message is still being typed.
    pub(super) fn type_actions(&self, from_hotkey: bool, actions: Vec<KeyAction>) {
        if TYPING.swap(true, Ordering::AcqRel) {
            return;
        }
        focus::restore();
        std::thread::spawn(move || {
            // Typing with Ctrl/Alt still held would send chords, so wait for them to go up.
            if from_hotkey && !wait_modifiers_released_real(MODIFIER_WAIT) {
                log_error(&format!("modifier keys were still held after {} s, macro not sent", MODIFIER_WAIT.as_secs()));
            } else {
                let failed = send(&actions);
                if failed > 0 {
                    log_error(&format!("some keystrokes were refused by Windows ({failed} key events)"));
                }
            }
            TYPING.store(false, Ordering::Release);
        });
    }

    pub(super) fn message(&self) -> String {
        let def = whips::def(self.cfg.whip);
        let line = match self.cfg.user_lines() {
            Some(l) => l[fastrand::usize(..l.len())].clone(),
            None => def.lines[fastrand::usize(..def.lines.len())].to_string(),
        };
        messages::compose(&line, def.intent, self.cfg.style())
    }

    pub(super) fn start_burst(&mut self) -> (f32, f32) {
        if self.whip.is_none() {
            // also re-aims a hidden overlay at the cursor's monitor, so the point below is on it
            self.start_ghost();
        }
        let (w, h) = self.overlay.size();
        let at = self.burst_point();
        self.fx.burst(whips::def(self.cfg.whip).effect, at, (w as f32, h as f32));
        self.last_frame = Instant::now();
        at
    }

    /// Reset: two messages into the last window. "normal mode" alone ends sticky and always-on
    /// modes (nj-terse, ponytail); "Continue." then sends Claude back to the work.
    pub(super) fn type_reset(&self) {
        self.type_actions(false, reset_actions(&self.cfg.macro_steps, messages::RESET, messages::RESUME));
    }
}
