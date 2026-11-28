//! Whip and overlay state: show, spawn, hide, toggle.

use super::*;

impl App {
    pub(super) fn show_whip(&mut self) {
        self.ghost = false;
        self.overlay.show();
        self.spawn_whip();
    }

    pub(super) fn spawn_whip(&mut self) {
        self.mouse_prev = self.overlay.cursor_local();
        let rope = Rope::spawn(&whips::def(self.cfg.whip).physics, self.mouse_prev);
        self.shown.clone_from(&rope.pts);
        self.whip = Some(Whip { rope, dropping: false, spawned: Instant::now() });
        self.swing = Swing::default();
        self.last_frame = Instant::now();
        self.acc = 0.0;
    }

    pub(super) fn hide_whip(&mut self) {
        self.whip = None;
        if self.lingering() {
            self.start_ghost();
        } else {
            self.hide_overlay();
        }
    }

    pub(super) fn hide_overlay(&mut self) {
        self.ghost = false;
        self.canvas = None;
        self.overlay.hide();
    }

    /// Something is still playing that outlives the whip: effects, meter, glow, combo label.
    pub(super) fn lingering(&self) -> bool {
        self.fx.alive() || self.combo.visible(Instant::now()).is_some() || !self.heat.cold()
    }

    /// No whip on the mouse: keep the overlay up, click-through, for what is still playing.
    pub(super) fn start_ghost(&mut self) {
        self.overlay.show_passive();
        self.ghost = true;
    }

    /// The loop animates (instead of blocking) while the whip or the ghost overlay is up.
    pub(super) fn active(&self) -> bool {
        self.whip.is_some() || self.ghost
    }

    /// The whip is on the mouse: shown and not being dropped.
    pub(super) fn held(&self) -> bool {
        self.whip.as_ref().is_some_and(|w| !w.dropping)
    }

    pub(super) fn burst_point(&self) -> (f32, f32) {
        self.whip.as_ref().and_then(|w| w.rope.pts.last().copied()).unwrap_or_else(|| self.overlay.cursor_local())
    }

    pub(super) fn toggle(&mut self) {
        match &mut self.whip {
            Some(w) if !w.dropping => w.dropping = true,
            _ if self.cfg.enabled => self.show_whip(),
            _ => {}
        }
        focus::restore();
    }
}
