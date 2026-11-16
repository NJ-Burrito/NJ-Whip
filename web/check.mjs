// Checks the wasm the site ships (docs/assets/nj-whip.wasm): node web/check.mjs
// A snap cracks once, a lazy sweep never does, every whip is in the catalogue with sound files
// that exist, and the Sauce trail is gone after its life.
import { readFileSync, existsSync } from 'node:fs';
import assert from 'node:assert/strict';

const docs = new URL('../docs/assets/', import.meta.url);
const { instance } = await WebAssembly.instantiate(readFileSync(new URL('nj-whip.wasm', docs)), {});
const X = instance.exports;
X.seed(2026, 11);
const [at, len] = [X.catalogue(), X.catalogue_len()]; // first: making the text may grow the memory
const cat = JSON.parse(new TextDecoder().decode(new Uint8Array(X.memory.buffer, at, len)));

assert.equal(cat.whips.length, 15, 'fifteen whips');
assert.equal(cat.packs.length, 6, 'six packs');
for (const w of cat.whips) {
  assert.ok(w.cracks.length && w.lines.length >= 5, w.name);
  for (const f of [...w.cracks, ...w.sounds]) assert.ok(existsSync(new URL('sounds/' + f, docs)), `${w.name}: docs/assets/sounds/${f} is missing`);
}

const DT = 1 / 60;
const out = (id) => new Float32Array(X.memory.buffer, X.stage_out(id), 16);
/** Cracks of whip `i` when the pointer follows `x(t)` for `secs`. */
function cracks(i, x, secs) {
  const id = X.stage_new(1280, 720, 1);
  X.stage_opts(id, 3);
  X.stage_whip(id, i, x(0), 400);
  for (let t = 0; t < 0.5; t += DT) X.stage_frame(id, DT, x(0), 400); // past the first-crack grace
  for (let t = 0; t < secs; t += DT) X.stage_frame(id, DT, x(t), 400);
  return out(id)[4];
}
const lazy = (t) => 640 + 300 * Math.cos(t * 2 * Math.PI);
const snap = (t) => { const s = t * 60; return s <= 10 ? 1000 - 25 * s : 750 + 50 * Math.min(s - 10, 12); };
cat.whips.forEach((w, i) => {
  assert.equal(cracks(i, lazy, 4), 0, `${w.name}: a lazy sweep only whooshes`);
  assert.equal(cracks(i, snap, 1), 1, `${w.name}: a snap cracks once`);
});

const sauce = cat.whips.findIndex((w) => w.trail);
assert.ok(sauce >= 0, 'a whip leaves a trail');
const id = X.stage_new(1280, 720, 1);
X.stage_whip(id, sauce, 200, 300);
for (let f = 0; f < 40; f++) X.stage_frame(id, DT, 200 + 22 * f, 300);
let drew = 0, last = 0;
for (let f = 0; f < (cat.trailLife + 4) * 60; f++) if (X.stage_frame(id, DT, 1080, 300) & 1) { drew++; last = f; }
assert.ok(drew > cat.trailLife * 30, 'the trail keeps fading on screen');
assert.ok(last / 60 < cat.trailLife + 2.5, `the trail is gone ${(last / 60).toFixed(1)} s after the whip stops (life ${cat.trailLife} s)`);

/* Every crack leaves its landing mark: on screen 1.5 s later, gone after MARK_LIFE. The whip is
   then taken to the far left, so only the mark can be near the crack point. */
cat.whips.forEach((w, i) => {
  const m = X.stage_new(1280, 720, 1);
  X.stage_whip(m, i, 640, 300); X.stage_settle(m, 200);
  X.stage_crack(m, 1); X.stage_frame(m, DT, 640, 300);
  const o = new Float32Array(X.memory.buffer, X.stage_out(m), 16), [mx, my] = [o[8], o[9]];
  X.stage_whip(m, i, 60, 40); X.stage_settle(m, 200);
  const lit = () => {
    const px = new Uint8Array(X.memory.buffer, X.stage_pixels(m), 1280 * 720 * 4);
    let n = 0;
    for (let y = Math.max(0, my - 90 | 0); y < Math.min(720, my + 90); y++)
      for (let x = Math.max(300, mx - 90 | 0); x < Math.min(1280, mx + 90); x++) {
        const k = (y * 1280 + x) * 4;
        if (Math.abs(px[k] - 0x0A) + Math.abs(px[k + 1] - 0x10) + Math.abs(px[k + 2] - 0x24) > 12) n++;
      }
    return n;
  };
  const run = (s) => { for (let f = 0; f < s * 60; f++) X.stage_frame(m, DT, 60, 40); };
  run(1.5); const mid = lit();
  run(cat.markLife + 0.5 - 1.5); const after = lit();
  assert.ok(mid > 20, `${w.name}: the landing mark shows 1.5 s after the crack (${mid} px)`);
  assert.ok(after <= 2, `${w.name}: the landing mark is gone after ${cat.markLife} s (${after} px left)`);
});

console.log(`PASS  ${cat.whips.length} whips, ${cat.packs.length} packs, swing, trail and landing marks behave as in the app`);
