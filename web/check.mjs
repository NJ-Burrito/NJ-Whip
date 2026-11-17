// Checks the wasm the site ships (docs/assets/nj-whip.wasm): node web/check.mjs
// A snap cracks once, a lazy sweep never does, every whip is in the catalogue with sound files
// that exist, every crack leaves its landing mark, and stages are freed and capped.
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
  const n = out(id)[4];
  X.stage_free(id);
  return n;
}
const lazy = (t) => 640 + 300 * Math.cos(t * 2 * Math.PI);
const snap = (t) => { const s = t * 60; return s <= 10 ? 1000 - 25 * s : 750 + 50 * Math.min(s - 10, 12); };
cat.whips.forEach((w, i) => {
  assert.equal(cracks(i, lazy, 4), 0, `${w.name}: a lazy sweep only whooshes`);
  assert.equal(cracks(i, snap, 1), 1, `${w.name}: a snap cracks once`);
});

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
  X.stage_free(m);
});

/* Stages do not pile up. Made and freed over and over, the wasm memory stays where it was (each
   1280x720 stage holds 7 MB; 500 kept would be 3.5 GB). Made and never freed, they are refused
   (u32::MAX) long before the 4 GiB wasm limit, where the old engine trapped. */
const MB = () => X.memory.buffer.byteLength / 2 ** 20, base = MB();
const NONE = -1; // u32::MAX as wasm hands an i32 back
for (let k = 0; k < 500; k++) {
  const s = X.stage_new(1280, 720, 1);
  assert.notEqual(s, NONE, `stage ${k} made`);
  X.stage_frame(s, DT, 640, 360);
  X.stage_free(s);
}
assert.ok(MB() <= base + 16, `500 stages made and freed: memory ${base} -> ${MB()} MB`);
const kept = [];
for (let s; (s = X.stage_new(1280, 720, 1)) !== NONE;) {
  kept.push(s);
  assert.ok(kept.length < 100, 'never-freed stages are refused before the memory runs out');
}
assert.ok(kept.length >= 16 && MB() < 512, `${kept.length} stages kept, then refused at ${MB()} MB`);
kept.forEach(X.stage_free);
assert.notEqual(X.stage_new(1280, 720, 1), NONE, 'freed stages make room again');

console.log(`PASS  ${cat.whips.length} whips, ${cat.packs.length} packs and landing marks behave as in the app; stages are freed and capped`);
