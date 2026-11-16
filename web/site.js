/* NJ-Whip site. Source: web/site.js (web/build.py puts it into both pages and renews the CSP hash).
   The whips, their physics, the crack detection, the effects, the heat meter and the glow are the
   app's own engine (assets/nj-whip.wasm). This file only feeds it the pointer, shows its pixels,
   plays the sound files it names and sets the combo label as page text. Without the wasm every
   stage keeps its still picture. */
(() => {
  'use strict';
  const D = document, root = D.documentElement;
  const A = D.currentScript.getAttribute('data-assets') || 'assets/';
  const calm = matchMedia('(prefers-reduced-motion: reduce)');
  const $ = (s, el) => (el || D).querySelector(s), $$ = (s, el) => [...(el || D).querySelectorAll(s)];
  const pick = (a) => a[Math.floor(Math.random() * a.length)];
  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  let X = null, cat = null, booting = null;

  /* ---------- engine ---------- */
  function boot() {
    if (!booting) {
      booting = fetch(A + 'nj-whip.wasm')
        .then((r) => { if (!r.ok) throw new Error('wasm ' + r.status); return r.arrayBuffer(); })
        .then((b) => WebAssembly.instantiate(b, {}))
        .then((m) => {
          X = m.instance.exports;
          X.seed((Math.random() * 2 ** 32) >>> 0, (Math.random() * 2 ** 32) >>> 0);
          const at = X.catalogue(), len = X.catalogue_len(); /* first: making the text may grow the memory */
          cat = JSON.parse(new TextDecoder().decode(new Uint8Array(X.memory.buffer, at, len)));
          root.classList.add('live');
        })
        .catch((e) => { still(); throw e; });
    }
    return booting;
  }
  /* No engine: the stills stay, the live controls go. */
  function still() {
    X = null; runners.length = 0;
    root.classList.remove('live'); root.classList.add('still');
  }

  class Stage {
    constructor(canvas, w, h, seg, opts) {
      this.c = canvas; this.ctx = canvas.getContext('2d');
      this.id = X.stage_new(w, h, seg); X.stage_opts(this.id, opts);
      this.fit(w, h); this.x = w / 2; this.y = h / 2; this.cracks = 0;
    }
    fit(w, h) { this.w = this.c.width = w; this.h = this.c.height = h; this.buf = null; }
    resize(w, h) { X.stage_resize(this.id, w, h); this.fit(w, h); this.x = w / 2; this.y = h / 2; }
    /* Views into the wasm memory; they are made again whenever the memory has grown. */
    get out() {
      if (this.buf !== X.memory.buffer) {
        this.buf = X.memory.buffer;
        this.img = new ImageData(new Uint8ClampedArray(this.buf, X.stage_pixels(this.id), this.w * this.h * 4), this.w, this.h);
        this.o = new Float32Array(this.buf, X.stage_out(this.id), 16);
      }
      return this.o;
    }
    take(i, x, y, settle) {
      this.x = x; this.y = y;
      X.stage_whip(this.id, i, x, y);
      if (settle) X.stage_settle(this.id, settle);
    }
    /* One frame; `all` paints the whole stage (a canvas that has not seen this stage yet). */
    frame(dt, all, ctx) {
      const bits = X.stage_frame(this.id, dt, this.x, this.y), o = this.out;
      if (all) (ctx || this.ctx).putImageData(this.img, 0, 0);
      else if (bits & 1) this.ctx.putImageData(this.img, 0, 0, o[0], o[1], o[2] - o[0], o[3] - o[1]);
      return bits;
    }
    /* True once per crack, whatever caused it (a swing, a click, a button). */
    cracked() { const n = this.out[4]; if (n === this.cracks) return false; this.cracks = n; return true; }
    crack(dir) { X.stage_crack(this.id, dir); }
  }

  /* ---------- one frame loop for everything that moves ---------- */
  const runners = [];
  let raf = 0, last = 0;
  function tick(now) {
    raf = 0;
    const dt = Math.min((now - last) / 1000, 0.1); last = now;
    try {
      for (let i = runners.length; i--;) if (runners[i](dt) === false) runners.splice(i, 1);
    } catch (e) { still(); throw e; }
    kick();
  }
  function kick() { if (!raf && runners.length && !D.hidden) { raf = requestAnimationFrame(tick); } }
  function run(fn) {
    if (!runners.includes(fn)) runners.push(fn);
    if (!raf) last = performance.now();
    kick();
  }
  D.addEventListener('visibilitychange', () => { last = performance.now(); kick(); });

  /* ---------- sound: off until the visitor turns it on or presses a whip ---------- */
  let actx = null, soundOn = false;
  const clips = {}, swish = {};
  function audio() {
    if (!actx) { const C = window.AudioContext || window.webkitAudioContext; if (C) actx = new C(); }
    if (actx && actx.state === 'suspended') actx.resume();
    return actx;
  }
  function clip(file) {
    return clips[file] || (clips[file] = fetch(A + 'sounds/' + file)
      .then((r) => r.arrayBuffer())
      .then((b) => new Promise((ok, no) => audio().decodeAudioData(b, ok, no)))
      .catch(() => null));
  }
  function out(buf, gain) {
    const s = actx.createBufferSource(), g = actx.createGain();
    s.buffer = buf; g.gain.value = gain; s.connect(g); g.connect(actx.destination); s.start();
  }
  function play(file, gain) { if (soundOn && audio()) clip(file).then((b) => b && soundOn && out(b, gain)); }
  /* A crack as the app plays it (src/sound.rs): one of the whip's cracks at random, its own clips
     on top. Those clips then rest until they have ended plus 1 s (EXTRAS_REST); `always` is the
     app's preview (a whip was picked), which plays them anyway. */
  let rest = 0;
  function crackSound(w, always) {
    play(pick(w.cracks), 0.85);
    if (!w.sounds.length || !soundOn || !audio() || (!always && actx.currentTime < rest)) return;
    rest = Infinity;
    Promise.all(w.sounds.map(clip)).then((bs) => {
      let long = 0;
      bs.forEach((b) => { if (b && soundOn) { out(b, 0.6); long = Math.max(long, b.duration); } });
      rest = actx.currentTime + long + 1;
    });
  }
  function preload(w) { if (soundOn && audio()) [...w.cracks, ...w.sounds].forEach(clip); }
  /* The app's whoosh (src/sound.rs): noise through a low-pass that opens and closes with the swing. */
  function whoosh(power, weight) {
    if (!soundOn || !audio()) return;
    weight = clamp(weight, 0.5, 2);
    const key = weight.toFixed(1);
    if (!swish[key]) {
      const n = Math.floor(actx.sampleRate * (0.14 + 0.1 * weight)), b = actx.createBuffer(1, n, actx.sampleRate), d = b.getChannelData(0);
      let lp = 0, lp2 = 0;
      for (let i = 0; i < n; i++) {
        const env = Math.sin(Math.PI * i / n) ** 1.6, cut = (0.03 + 0.2 * env) / Math.sqrt(weight);
        lp += (Math.random() * 2 - 1 - lp) * cut; lp2 += (lp - lp2) * cut;
        d[i] = (lp * 0.5 + (lp - lp2) * 1.1) * env;
      }
      swish[key] = b;
    }
    out(swish[key], 0.85 * (0.2 + 0.5 * clamp(power, 0, 1)));
  }
  function setSound(on) {
    soundOn = on && !!audio();
    $$('[data-sound]').forEach((b) => {
      b.setAttribute('aria-pressed', soundOn);
      $('[data-label]', b).textContent = b.getAttribute(soundOn ? 'data-on' : 'data-off');
    });
  }
  $$('[data-sound]').forEach((b) => b.addEventListener('click', () => setSound(!soundOn)));

  /* ---------- the combo label: page text over the stage, in the web font ---------- */
  function shout(el, st, w) {
    const o = st.out, n = o[6];
    if (n < 2) return;
    const k = el.parentNode.clientWidth / st.w, s = el.style;
    el.textContent = w.shout + ' ×' + n;
    s.fontSize = ((30 + 3 * Math.min(n, 12)) * k).toFixed(1) + 'px';
    s.left = clamp((o[8] + o[10] * 24) / st.w * 100, 14, 86) + '%';
    s.top = clamp((o[9] - 90) / st.h * 100, 24, 92) + '%';
    s.setProperty('--tilt', (o[10] > 0 ? -6 : -3) + 'deg');
    s.setProperty('--hue', w.hue);
    el.classList.toggle('five', n % 5 === 0);
    el.classList.remove('on'); void el.offsetWidth; el.classList.add('on');
    clearTimeout(el.t); el.t = setTimeout(() => el.classList.remove('on'), 1500);
  }

  /* ---------- the gallery: a whip leans toward the pointer; a press cracks it with its sound ---------- */
  const GW = 480, GH = 320, GSEG = 0.6, REST = [118, 150];
  function gallery(cards) {
    const brush = new Stage(D.createElement('canvas'), GW, GH, GSEG, 0);
    cards.forEach((card, i) => {
      const cv = $('canvas', card);
      cv.width = GW; cv.height = GH;
      brush.take(i, REST[0], REST[1], 240);
      brush.frame(0, true, cv.getContext('2d'));
      let st = null, aim = null, idle = 0, nod = 0;
      const live = () => {
        if (!st) { st = new Stage(cv, GW, GH, GSEG, 0); st.take(i, REST[0], REST[1], 240); st.frame(0, true); }
        idle = 0; run(step);
      };
      function step(dt) {
        nod = Math.max(0, nod - dt);
        const to = aim || [REST[0] + 46 * Math.sin(nod * 14) * nod, REST[1]], f = 1 - Math.exp(-dt * 9);
        st.x += (to[0] - st.x) * f; st.y += (to[1] - st.y) * f;
        idle = st.frame(dt) & 1 || aim || nod ? 0 : idle + 1;
        return idle < 45;
      }
      card.addEventListener('pointermove', (e) => {
        if (e.pointerType === 'touch' || calm.matches) return;
        const r = cv.getBoundingClientRect();
        aim = [REST[0] + ((e.clientX - r.left) / r.width - 0.5) * 150, REST[1] + ((e.clientY - r.top) / r.height - 0.5) * 70];
        live();
      });
      card.addEventListener('pointerleave', () => { aim = null; });
      card.addEventListener('focus', () => { if (!calm.matches && card.matches(':focus-visible')) { nod = 0.7; live(); } });
      card.addEventListener('click', () => {
        const w = cat.whips[i];
        setSound(true); crackSound(w, true);
        live(); st.crack(0);
      });
    });
  }

  /* ---------- heat and streaks: the picked whip, cracked on a beat until the meter is full ---------- */
  const HW = 960, HH = 600, HSEG = 0.9, BEAT = 0.7, LOOP = 9;
  function heat(el) {
    const cv = $('canvas', el), lab = $('.shout', el), st = new Stage(cv, HW, HH, HSEG, 2);
    let i = 0, t = 0, beat = 0, seen = false, auto = !calm.matches, idle = 0;
    const hand = (time) => {
      const amp = 0.17 * HW * clamp((5.2 - time) / 0.7, 0, 1);
      return [HW * 0.42 + amp * Math.sin(Math.PI * time / BEAT), HH * 0.58 + 10 * Math.sin(time * 2.2)];
    };
    function take() {
      X.stage_reset(st.id);
      t = 0; beat = 0; lab.classList.remove('on');
      st.take(i, ...hand(0), 200); st.frame(0, true);
      el.style.setProperty('--hue', cat.whips[i].hue);
      if (auto) wake(); else pose();
    }
    /* Reduced motion: one still frame at full heat, no loop. */
    function pose() {
      for (let k = 0; k < 4; k++) st.crack(k % 2 ? -1 : 1);
      for (let k = 0; k < 66; k++) X.stage_frame(st.id, 1 / 60, st.x, st.y);
      st.frame(0, true); shout(lab, st, cat.whips[i]); clearTimeout(lab.t);
    }
    function step(dt) {
      if (!seen) return false;
      if (auto) {
        t += dt;
        if (t > LOOP) { t = 0; beat = 0; }
        [st.x, st.y] = hand(t);
        const due = Math.min(6, Math.floor(t / BEAT));
        if (due > beat) { beat = due; st.crack(beat % 2 ? -1 : 1); }
      }
      idle = st.frame(dt) & 1 ? 0 : idle + 1;
      if (st.cracked()) shout(lab, st, cat.whips[i]);
      return auto || idle < 60;
    }
    const wake = () => { idle = 0; run(step); };
    $$('input[name=heat-whip]', el).forEach((r, k) => r.addEventListener('change', () => { i = k; take(); }));
    $('[data-crack]', el).addEventListener('click', () => {
      /* A press takes over from the demo: every crack from here on is the visitor's. */
      if (auto) { auto = false; X.stage_reset(st.id); }
      st.crack(st.cracks % 2 ? -1 : 1); wake();
    });
    new IntersectionObserver((es) => { seen = es[0].isIntersecting; if (seen) wake(); }).observe(el);
    calm.addEventListener('change', () => { auto = !calm.matches; take(); });
    take();
  }

  /* ---------- the room: the visitor's pointer holds the whip ---------- */
  function room(el) {
    const cv = $('canvas', el), frame = cv.parentNode, lab = $('.shout', el);
    const typed = $('[data-typed]', el), tag = $('[data-tag]', el), say = $('[data-say]', el), packs = $('select', el);
    const names = el.getAttribute('data-intents').split('|');
    let st = null, i = 0, idle = 0, seen = false, job = 0, press = null;
    const world = () => {
      const r = frame.getBoundingClientRect(), w = Math.round(clamp(r.width * Math.min(devicePixelRatio || 1, 1.5), 720, 1280));
      return [w, Math.round(w * r.height / r.width)];
    };
    const [w0, h0] = world();
    st = new Stage(cv, w0, h0, 1, 3);
    const home = () => { st.take(i, st.w * 0.3, st.h * 0.5, 200); st.frame(0, true); };
    home();

    function step(dt) {
      if (!seen) return false;
      const bits = st.frame(dt);
      if (bits & 4) whoosh(st.out[5], cat.whips[i].weight);
      if (st.cracked()) onCrack();
      idle = bits & 1 ? 0 : idle + 1;
      return idle < 90;
    }
    const wake = () => { idle = 0; run(step); };

    function onCrack() {
      const w = cat.whips[i], n = cat.intents.indexOf(w.intent), row = cat.packs[packs.selectedIndex].rows[n];
      const text = pick(w.lines).trim() + ' ' + row.light;
      crackSound(w); shout(lab, st, w);
      el.classList.add('cracked');
      tag.textContent = names[n] + ' · ' + el.getAttribute(row.esc ? 'data-esc' : 'data-queued');
      tag.setAttribute('data-how', row.esc ? 'esc' : 'queued');
      say.textContent = w.shout + '. ' + text;
      const mine = ++job;
      if (calm.matches) { typed.textContent = text; return; }
      let at = 0;
      typed.textContent = '';
      run(() => {
        if (mine !== job) return false;
        at = Math.min(text.length, at + 3);
        typed.textContent = text.slice(0, at);
        return at < text.length;
      });
    }

    const to = (e) => {
      const r = cv.getBoundingClientRect();
      st.x = (e.clientX - r.left) * st.w / r.width; st.y = (e.clientY - r.top) * st.h / r.height;
      wake();
    };
    cv.addEventListener('pointermove', to);
    cv.addEventListener('pointerdown', (e) => {
      to(e);
      if (e.pointerType === 'mouse') { if (e.button === 0) st.crack(0); return; }
      press = [e.clientX, e.clientY, e.timeStamp];
      try { cv.setPointerCapture(e.pointerId); } catch (_) { /* the pointer is already gone */ }
    });
    cv.addEventListener('pointerup', (e) => {
      /* touch: a tap cracks, a drag swings */
      if (press && e.timeStamp - press[2] < 300 && Math.hypot(e.clientX - press[0], e.clientY - press[1]) < 12) { st.crack(0); wake(); }
      press = null;
    });
    $$('input[name=room-whip]', el).forEach((r, k) => r.addEventListener('change', () => {
      i = k;
      X.stage_whip(st.id, i, st.x, st.y);
      el.style.setProperty('--hue', cat.whips[i].hue);
      preload(cat.whips[i]);
      if (soundOn) crackSound(cat.whips[i], true);
      wake();
    }));
    $('[data-crack]', el).addEventListener('click', () => { st.crack(0); wake(); });
    $('[data-sound]', el).addEventListener('click', () => preload(cat.whips[i]));
    let fit = 0;
    new ResizeObserver(() => {
      clearTimeout(fit);
      fit = setTimeout(() => {
        const [w, h] = world();
        if (Math.abs(w - st.w) < 24 && Math.abs(h - st.h) < 24) return;
        st.resize(w, h); home(); wake();
      }, 160);
    }).observe(frame);
    new IntersectionObserver((es) => { seen = es[0].isIntersecting; if (seen) wake(); }).observe(el);
    el.style.setProperty('--hue', cat.whips[0].hue);
  }

  /* ---------- start: the engine loads when a stage comes near the screen ---------- */
  const stages = $$('[data-stage]');
  if (stages.length && 'WebAssembly' in window && 'IntersectionObserver' in window && 'ResizeObserver' in window) {
    const io = new IntersectionObserver((es) => {
      if (!es.some((e) => e.isIntersecting)) return;
      io.disconnect();
      boot().then(() => {
        try {
          gallery($$('.whip'));
          heat($('#heat-stage'));
          room($('#room-stage'));
        } catch (e) { still(); throw e; }
      }).catch((e) => console.warn('NJ-Whip: the live whips did not start, the pictures stay.', e));
    }, { rootMargin: '900px 0px' });
    stages.forEach((s) => io.observe(s));
  } else if (stages.length) {
    still();
  }
})();
