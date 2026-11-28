//! Lightning: the crack's burst and the mark it leaves.
//! Storm: a scorch

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, screen: V) {
    let st = &mut fx.stamps;
    let to = if at.1 < screen.1 * 0.5 { (at.0 + rnd(-120.0, 120.0), screen.1) } else { (at.0 + rnd(-120.0, 120.0), 0.0) };
    st.push(Stamp { kind: StampKind::Bolt, at, to, age: 0.0, life: 0.3, rgb: 0xFFE680 });
    fx.flash = Some(ScreenFlash { colours: vec![0xFFE08A], alpha: 0.22, age: 0.0, life: 0.18 });
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::TAU;
    let c @ (x, y) = s.at;
    // soot fading outward, a jagged burn, branching fern scars and a glowing ember core
    disc(pm, c, 46.0, 0x2A1A12, a * 0.2);
    disc(pm, c, 36.0, 0x2A1A12, a * 0.3);
    let burn: [V; 12] = std::array::from_fn(|k| {
        let (ang, rad) = (k as f32 * TAU / 12.0, if k % 2 == 0 { r(26.0, 36.0) } else { r(17.0, 23.0) });
        (x + ang.cos() * rad, y + ang.sin() * rad)
    });
    fill_poly(pm, &burn, 0x2B1B14, a * 0.95);
    disc(pm, c, 17.0, 0x120C09, a * 0.9);
    let mut edge = [burn[0]; 13];
    edge[..12].copy_from_slice(&burn);
    stroke_line(pm, &edge, 0xFF9A3C, a * 0.4, 1.3);
    let mut fern = [((0.0f32, 0.0f32), (0.0f32, 0.0f32)); 10];
    for k in 0..5 {
        let (ang, len) = (k as f32 * TAU / 5.0 + r(-0.3, 0.3), r(40.0, 56.0));
        let (p0, p1) = ((x + ang.cos() * 20.0, y + ang.sin() * 20.0), (x + ang.cos() * len, y + ang.sin() * len));
        let fa = ang + r(-0.9, 0.9);
        let pf = (x + ang.cos() * 32.0, y + ang.sin() * 32.0);
        fern[k * 2] = (p0, p1);
        fern[k * 2 + 1] = (pf, (pf.0 + fa.cos() * 10.0, pf.1 + fa.sin() * 10.0));
    }
    segs(pm, fern.iter().copied(), 0x15171C, a * 0.7, 2.0);
    segs(pm, fern.iter().copied(), 0xFF9A3C, a * 0.35, 0.8);
    hoop(pm, c, 12.0, 0xFF9A3C, a * 0.3, 5.0);
    hoop(pm, c, 12.0, 0xFF9A3C, a * 0.75, 2.2);
    disc(pm, c, 6.0, 0xFF9A3C, a * 0.35);
    disc(pm, c, 4.0, 0xFFE680, a * 0.85);
}
