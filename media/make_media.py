#!/usr/bin/env python3
"""Rebuilds NJ-Whip's site and README media from the product's own off-screen renderers.

    uv run --with pillow --with playwright python media/make_media.py [--cargo] [--only NAME ...] [--sheet PATH]

Never starts nj-whip.exe (a live overlay that types into other windows). What it uses:
  * the cargo gallery / demo_frames / window-sheet tests (src/render/tests.rs, src/ui/sheets.rs), with --cargo
  * the website's own WebAssembly engine (docs/assets/nj-whip.wasm) in headless Chromium, over a local
    http server it starts and stops, through media/scene.html (hero, og, banner, heat, room)
  * ffmpeg for the mp4 and gif files; Pillow for the contact sheet
Type is Thmanyah, taken from the installed fonts by name; no font file is copied into the repo.
Work files go to target/media (or NJ_WHIP_MEDIA_WORK); the finished files go to docs/assets and media/banner.png.
"""
import argparse, http.server, os, shutil, socketserver, subprocess, sys, threading
from pathlib import Path

os.environ.setdefault("PYTHONUTF8", "1")
ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "docs" / "assets"
WORK = Path(os.environ.get("NJ_WHIP_MEDIA_WORK", ROOT / "target" / "media"))
FFMPEG = shutil.which("ffmpeg") or "ffmpeg"
FFPROBE = shutil.which("ffprobe") or "ffprobe"

HERO_FPS = 20          # the mp4; the gif plays the same frames at 12.5 fps (80 ms)
HERO_WHIPS = ["Classic", "Cowboy", "Chef", "Drill Sergeant"]
BEAT_SECONDS = 3.4
HERO_SEG, HERO_HY, HERO_AMP = 0.42, 0.33, 0.19   # lash scale on the stage, hand height, sweep width
STILL_FRAME = 58       # beat 1, 2.9 s in: three cracks, the label up, the line typed, the marks on the stage


# ---------------------------------------------------------------- server and browser
class Quiet(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=str(ROOT), **k)

    def log_message(self, *a):
        pass


class Site:
    """A local http server on the repo root; each page() is one headless Chromium on media/scene.html."""

    def __init__(self):
        socketserver.TCPServer.allow_reuse_address = True
        self.httpd = socketserver.TCPServer(("127.0.0.1", 0), Quiet)
        self.port = self.httpd.server_address[1]
        threading.Thread(target=self.httpd.serve_forever, daemon=True).start()
        self.pw = None
        self.n = 0
        self.failed = []

    def __enter__(self):
        from playwright.sync_api import sync_playwright
        self.pw = sync_playwright().start()
        return self

    def __exit__(self, *e):
        try:
            if self.pw:
                self.pw.stop()
        finally:
            self.httpd.shutdown()
            self.httpd.server_close()

    def page(self, w, h, scale=1.0):
        self.n += 1
        ctx = self.pw.chromium.launch_persistent_context(
            str(WORK / f"chromium-nj-whip-media-{self.n}"), headless=True,
            viewport={"width": w, "height": h}, device_scale_factor=scale)
        pg = ctx.pages[0] if ctx.pages else ctx.new_page()
        pg.on("requestfailed", lambda r: self.failed.append(r.url))
        pg.on("pageerror", lambda e: print("page error:", e, file=sys.stderr))
        pg.goto(f"http://127.0.0.1:{self.port}/media/scene.html")
        info = pg.evaluate("boot()")
        return ctx, pg, info


def run(cmd, **k):
    r = subprocess.run(cmd, **k)
    if r.returncode:
        raise SystemExit(f"command failed ({r.returncode}): {cmd[0]}")


# ---------------------------------------------------------------- hero, og, stills
def hero_frames(site, lang, out_dir, w=960, h=480, scale=1.0, last=None):
    """Writes the frames of the four-beat hero at HERO_FPS; returns the list of paths."""
    out_dir.mkdir(parents=True, exist_ok=True)
    ctx, pg, _ = site.page(w, h, scale)
    pg.evaluate("o => heroBuild(o)", {"lang": lang, "w": w, "h": h, "whips": HERO_WHIPS, "seg": HERO_SEG, "hy": HERO_HY, "amp": HERO_AMP})
    n = int(round(len(HERO_WHIPS) * BEAT_SECONDS * HERO_FPS)) if last is None else last
    paths = []
    for k in range(n):
        pg.evaluate("([k, f]) => heroFrame(k, f)", [k, HERO_FPS])
        p = out_dir / f"{k:04d}.png"
        pg.screenshot(path=str(p))
        paths.append(p)
    ctx.close()
    return paths


def encode(frames_dir, mp4, gif, fps=HERO_FPS, gif_fps=12.5, gif_colors=96):
    """mp4 from every frame (h264, yuv420p); gif from the same frames at 12.5 fps."""
    pattern = str(frames_dir / "%04d.png")
    run([FFMPEG, "-y", "-v", "error", "-framerate", str(fps), "-i", pattern, "-c:v", "libx264", "-preset", "slow",
         "-crf", "24", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(mp4)])
    pal = frames_dir / "palette.png"
    sel = f"fps={gif_fps}"
    run([FFMPEG, "-y", "-v", "error", "-framerate", str(fps), "-i", pattern, "-vf", f"{sel},palettegen=max_colors={gif_colors}:stats_mode=full", str(pal)])
    run([FFMPEG, "-y", "-v", "error", "-framerate", str(fps), "-i", pattern, "-i", str(pal), "-lavfi",
         f"{sel}[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle", "-loop", "0", str(gif)])


def png_copy(src, dst):
    shutil.copyfile(src, dst)
    print("wrote", dst.relative_to(ROOT))


def build_hero(site, lang, mp4, gif, still):
    d = WORK / f"hero-{lang}"
    shutil.rmtree(d, ignore_errors=True)
    hero_frames(site, lang, d)
    encode(d, ASSETS / mp4, ASSETS / gif)
    png_copy(d / f"{STILL_FRAME:04d}.png", ASSETS / still)


def build_og(site):
    d = WORK / "og"
    shutil.rmtree(d, ignore_errors=True)
    ps = hero_frames(site, "en", d, 960, 504, 1.25, last=STILL_FRAME + 1)
    png_copy(ps[STILL_FRAME], ASSETS / "og.png")


def shot(site, w, h, js, arg, dst):
    ctx, pg, _ = site.page(w, h)
    res = pg.evaluate(js, arg)
    pg.screenshot(path=str(dst))
    ctx.close()
    print("wrote", dst.relative_to(ROOT), res)


def build_banner(site):
    shot(site, 1280, 640, "o => banner(o)", {"whip": "Classic", "cracks": 3, "seg": 0.5, "hx": 0.42, "hy": 0.22, "amp": 0.7, "frames": 10,
                                           "window": "docs/assets/settings.png", "seed": 11}, ROOT / "media" / "banner.png")


def build_heat(site):
    shot(site, 586, 368, "o => pose(o)", {"whip": "Neon", "w": 586, "h": 368, "seg": 0.55, "cracks": 5, "frames": 8, "hy": 0.30}, ASSETS / "heat.png")


def build_room(site):
    shot(site, 1070, 603, "o => pose(o)", {"whip": "Sauce", "w": 1070, "h": 603, "seg": 0.8, "cracks": 2, "frames": 40, "hy": 0.2, "hx": 0.5}, ASSETS / "room.png")


def build_demo():
    src = ROOT / "target" / "demo"
    if not (src / "0000.png").exists():
        raise SystemExit("no target/demo frames: run with --cargo")
    mp4, gif = ASSETS / "demo.mp4", ASSETS / "demo.gif"
    run([FFMPEG, "-y", "-v", "error", "-framerate", "60", "-i", str(src / "%04d.png"), "-c:v", "libx264", "-preset", "slow",
         "-crf", "27", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(mp4)])
    pal = WORK / "demo-palette.png"
    vf = "fps=10,scale=720:405:flags=lanczos"
    run([FFMPEG, "-y", "-v", "error", "-framerate", "60", "-i", str(src / "%04d.png"), "-vf", vf + ",palettegen=max_colors=128:stats_mode=diff", str(pal)])
    run([FFMPEG, "-y", "-v", "error", "-framerate", "60", "-i", str(src / "%04d.png"), "-i", str(pal), "-lavfi",
         vf + "[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle", "-loop", "0", str(gif)])
    print("wrote", mp4.relative_to(ROOT), gif.relative_to(ROOT))


def cargo(*args, **env):
    e = dict(os.environ, PYTHONUTF8="1", **env)
    run(["cargo", "test", "--release", "--locked", *args, "--", "--ignored", "--nocapture"], cwd=ROOT, env=e)


def build_cargo(ui_dir):
    """The off-screen cargo renders. The window is 560x760 logical; 1.25x makes the 700x950 the pages expect."""
    cargo("render::tests::gallery")
    cargo("render::tests::demo_frames")
    ui_dir.mkdir(parents=True, exist_ok=True)
    cargo("ui::", NJ_WHIP_UI_OUT=str(ui_dir), NJ_WHIP_PPP="1.25", NJ_WHIP_TAB_ONLY="whip")


# ---------------------------------------------------------------- the contact sheet and the size gate
OUTPUTS = [
    ("docs/assets/gallery.png", (2000, 810)), ("docs/assets/hero-still.png", (960, 480)), ("docs/assets/hero-still-ar.png", (960, 480)),
    ("docs/assets/og.png", (1200, 630)), ("media/banner.png", (1280, 640)), ("docs/assets/settings.png", (700, 950)),
    ("docs/assets/settings-ar.png", (700, 950)), ("docs/assets/heat.png", (586, 368)), ("docs/assets/room.png", (1070, 603)),
]


def contact_sheet(path):
    from PIL import Image, ImageDraw
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    cell_w, cell_h = 640, 400
    tiles = [(f, Image.open(ROOT / f).convert("RGB")) for f, _ in OUTPUTS]
    for name, frames in (("hero.gif", (0, 30, 60, 100, 130, 160)), ("hero-ar.gif", (0, 30, 60, 100, 130, 160)), ("demo.gif", (10, 30, 50, 70))):
        im = Image.open(ASSETS / name)
        for k in frames:
            im.seek(min(k, im.n_frames - 1))
            tiles.append((f"{name} frame {k}", im.convert("RGB")))
    cols = 4
    rows = (len(tiles) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * cell_w, rows * cell_h), (30, 30, 36))
    dr = ImageDraw.Draw(sheet)
    for i, (label, im) in enumerate(tiles):
        size = im.size
        im.thumbnail((cell_w - 12, cell_h - 30))
        x, y = (i % cols) * cell_w, (i // cols) * cell_h
        sheet.paste(im, (x + 6, y + 24))
        dr.text((x + 8, y + 6), f"{label} {size}", fill=(255, 255, 255))
    sheet.save(path)
    print("sheet", path)


def nine_hero_frames(path, lang="en"):
    from PIL import Image
    d = WORK / f"hero-{lang}"
    ks = [4, 20, 40, 58, 80, 110, 150, 200, 250]
    ims = [Image.open(d / f"{k:04d}.png").convert("RGB") for k in ks]
    w, h = ims[0].size
    sheet = Image.new("RGB", (w * 3, h * 3))
    for i, im in enumerate(ims):
        sheet.paste(im, ((i % 3) * w, (i // 3) * h))
    sheet.save(path)
    print("frames", path)


def check_sizes():
    from PIL import Image
    ok = True
    for f, size in OUTPUTS:
        got = Image.open(ROOT / f).size
        ok &= got == size
        print(("ok  " if got == size else "BAD ") + f, got, "want", size)
    for f, size in (("docs/assets/hero.gif", (960, 480)), ("docs/assets/hero-ar.gif", (960, 480)), ("docs/assets/demo.gif", (720, 405))):
        got = Image.open(ROOT / f).size
        mb = (ROOT / f).stat().st_size / 1e6
        good = got == size and mb <= 4
        ok &= good
        print(("ok  " if good else "BAD ") + f, got, f"{mb:.2f} MB")
    for f, size in (("docs/assets/hero.mp4", "960x480"), ("docs/assets/hero-ar.mp4", "960x480"), ("docs/assets/demo.mp4", "1280x720")):
        r = subprocess.run([FFPROBE, "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height",
                            "-of", "csv=p=0:s=x", str(ROOT / f)], capture_output=True, text=True)
        good = r.stdout.strip() == size
        ok &= good
        print(("ok  " if good else "BAD ") + f, r.stdout.strip())
    b = (ROOT / "media" / "banner.png").stat().st_size
    ok &= b < 1_000_000
    print(("ok  " if b < 1_000_000 else "BAD ") + "media/banner.png", b, "bytes")
    return ok


ALL = ["settings", "gallery", "demo", "hero", "hero-ar", "og", "heat", "room", "banner"]

if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*", help="any of: " + " ".join(ALL))
    ap.add_argument("--cargo", action="store_true", help="run the cargo renders first (gallery, demo frames, window sheets)")
    ap.add_argument("--ui-dir", help="folder with whip-en-dark.png and whip-ar-dark.png (default work/ui)")
    ap.add_argument("--sheet", help="write one contact sheet of every output here, and nine hero frames beside it")
    ap.add_argument("--probe", type=int, help="render N hero frames only, into work/probe")
    ap.add_argument("--lang", default="en")
    a = ap.parse_args()
    WORK.mkdir(parents=True, exist_ok=True)
    todo = a.only or ALL
    ui_dir = Path(a.ui_dir) if a.ui_dir else WORK / "ui"
    if a.cargo:
        build_cargo(ui_dir)
    with Site() as site:
        if a.probe:
            print(len(hero_frames(site, a.lang, WORK / "probe", last=a.probe)), "frames")
        else:
            if "settings" in todo:
                png_copy(ui_dir / "whip-en-dark.png", ASSETS / "settings.png")
                png_copy(ui_dir / "whip-ar-dark.png", ASSETS / "settings-ar.png")
            if "gallery" in todo:
                png_copy(ROOT / "target" / "gallery.png", ASSETS / "gallery.png")
            if "demo" in todo:
                build_demo()
            if "hero" in todo:
                build_hero(site, "en", "hero.mp4", "hero.gif", "hero-still.png")
            if "hero-ar" in todo:
                build_hero(site, "ar", "hero-ar.mp4", "hero-ar.gif", "hero-still-ar.png")
            if "og" in todo:
                build_og(site)
            if "heat" in todo:
                build_heat(site)
            if "room" in todo:
                build_room(site)
            if "banner" in todo:
                build_banner(site)
        print("failed requests:", site.failed)
    if a.sheet and not a.probe:
        contact_sheet(a.sheet)
        nine_hero_frames(Path(a.sheet).with_name("hero-frames.png"))
        if not check_sizes():
            raise SystemExit("size gate failed")
