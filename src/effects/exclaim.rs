//! Exclaim: the crack's burst and the mark it leaves.
//! Drill Sergeant: a bullet hole

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    let st = &mut fx.stamps;
    let stamp = |kind, to: V, life: f32, rgb: u32| Stamp { kind, at, to, age: 0.0, life, rgb };
    st.push(stamp(StampKind::Exclaim, at, 0.5, 0xE03A3A));
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::TAU;
    let c @ (x, y) = s.at;
    // scorch and dust round the hole, then splintered cracks with a fork each
    disc(pm, c, 30.0, 0x000000, a * 0.12);
    disc(pm, c, 21.0, 0x000000, a * 0.16);
    let mut cracks = [((0.0f32, 0.0f32), (0.0f32, 0.0f32)); 18];
    for k in 0..6 {
        let (ang, len) = (r(0.0, TAU), r(20.0, 36.0));
        let at = |d: f32, side: f32| (x + ang.cos() * d - ang.sin() * side, y + ang.sin() * d + ang.cos() * side);
        let (p0, pk, p1) = (at(11.0, 0.0), at(len * 0.55, r(-4.5, 4.5)), at(len, r(-3.0, 3.0)));
        let (fa, fl) = (ang + r(-0.9, 0.9), r(6.0, 11.0));
        cracks[k * 3] = (p0, pk);
        cracks[k * 3 + 1] = (pk, p1);
        cracks[k * 3 + 2] = (pk, (pk.0 + fa.cos() * fl, pk.1 + fa.sin() * fl));
    }
    segs(pm, cracks.iter().copied(), 0x9AA0A8, a * 0.5, 3.0);
    segs(pm, cracks.iter().copied(), 0x1E1F24, a * 0.9, 1.5);
    // a bevelled steel lip around a dark hole, light catching its lower right
    hoop(pm, c, 11.5, 0x5C626A, a * 0.9, 3.4);
    hoop(pm, c, 11.0, 0xC8CCD2, a * 0.8, 1.6);
    disc(pm, c, 9.5, 0x0B0C0E, a * 0.95);
    stroke_line(pm, &arc(c, 7.6, 1.0, 0.2, 1.4, 6), 0xFFFFFF, a * 0.3, 1.4);
}
