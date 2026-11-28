# Rendering a whip or an effect without a window

The live app is an overlay that types into other windows, so it is never run to look at a whip.
Pictures come from the off-screen renderer: an ignored test that drives the same `Rope`, `Fx` and
`Canvas` the overlay uses and writes PNGs.

## Command

```text
set NJ_WHIP_NAME=ninja            (a whip name, an effect name, or all; default all)
set NJ_WHIP_FRAMES=target/frames  (output folder; default target/frames)
set NJ_WHIP_N=12                  (frames per run; default 12, at least 6)
cargo test --release --locked render::frames::frames -- --ignored --exact --nocapture
```

Output, per whip and per effect: `<folder>/whip-<name>/NNN.png`, `<folder>/effect-<name>/NNN.png`,
a `sheet.png` beside them (the frames at a third of their size, four per row) and `<folder>/index.txt`.
Frames are 960x540 over the Polar Night background. A whip run swings the rope and cracks once a
third of the way in; an effect run shows the burst every 70 ms and the mark it leaves at 1.0, 2.0
and 2.9 s. The seed is fixed, so a run is repeatable. Source: `src/render/frames.rs`.

## Where each piece lives

| Piece | Files |
|---|---|
| Whip lash, shadow, glow, highlight, `draw_whip` | `src/whipdraw/mod.rs` |
| Lash pattern (braid, rings, ribbon, chunks) | `src/whipdraw/pattern.rs` |
| Handle | `src/whipdraw/handle.rs` |
| Tip art, one file per tip | `src/whipdraw/tips/<tip>.rs` (dispatch and shared frames in `tips/mod.rs`) |
| Crack effect, one file per effect: `burst` (particles, stamps, flash) and `mark` (what it leaves) | `src/effects/<effect>.rs` |
| Effect core: types, `Fx` stepping, `ring`, drawing helpers, the dispatch | `src/effects/mod.rs` |
| Stamp drawing (shock, exclaim, ripple, lasso, slash, magic, bolt) | `src/effects/stamps.rs` |
| Particle drawing | `src/effects/particle.rs` |
| Glow at the screen edges, tip flash | `src/render/glow.rs` |
| Heat meter, combo label | `src/render/hud.rs` |
| Layers, `Canvas`, dirty boxes | `src/render/mod.rs` |
| Window tabs and chrome | `src/ui/{chrome,whip_tab,message_tab,options_tab}.rs` |
| Engine: whip state, typing, one frame | `src/app/{whip_state,typing,frame}.rs` |

Each effect file has a `burst` and a `mark` function; the matching `match` arm in
`src/effects/mod.rs` only dispatches. Stamps (`StampKind`) are shared by several effects, so a
change to one stamp is made in `stamps.rs`.

## The catalogue (whip, tip file, effect file, pattern)

| Whip | Tip file | Effect file | Pattern |
|---|---|---|---|
| Classic | `frayed` | `sparks` | braid |
| Neon | `bead` | `flash` | plain |
| Elephant | `tail` | `thump` | rings |
| Cowboy | `frayed` | `lasso` | braid |
| Cheerleader | `pompom` | `confetti` | ribbon |
| Sauce | `drip` | `splash` | chunks |
| Drill Sergeant | `frayed` | `exclaim` | braid |
| Zen | `frayed` | `ripple` | plain |
| Ninja | `kunai` | `slash` | braid |
| Pirate | `frayed` | `spray` | braid |
| Chef | `pan` | `sizzle` | rings |
| Wizard | `bead` | `magic` | plain |
| Vine | `bloom` | `petals` | braid |
| Penguin | `penguin` | `snow` | ribbon |
| Storm | `cloud` | `lightning` | braid |

Names for `NJ_WHIP_NAME`: the whip name in the first column (case does not matter) or an effect name
from the third column.

## Checks that prove a change moved no pixel

`render::tests::golden_write` before the change and `golden_check` after, then `whipdraw::tests::golden_write`
and `golden_compare` (both ignored: `cargo test --release --locked <name> -- --ignored`). A visual
change is expected to fail them; read the sheet instead.

The web build (`python web/build.py`) compiles `effects`, `render` and `whipdraw` through
`#[path]` includes in `web/src/lib.rs`; each points at the module's `mod.rs`.
