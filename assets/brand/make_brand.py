"""NJ-Whip brand set: mark, wordmark lockups, app icon, favicon. Drawn in code, no third-party art.

    uv run --with pillow python assets/brand/make_brand.py            # write everything
    uv run --with pillow python assets/brand/make_brand.py --sheet target/brand-sheet.png

One geometry (512 x 512 design space) feeds two writers: SVG text and a supersampled Pillow raster, so the
masters and the PNG/ICO files cannot drift apart. The wordmark outlines (Thmanyah Serif Display Bold) come from
wordmark.json (see extract_wordmark.py), so no font is needed at build time.

The mark: a white N, and a J whose stem is a whip handle and whose hook is the lash, ending in a crack spark.
"""
import argparse, json, math, pathlib, struct, io, sys
from PIL import Image, ImageChops, ImageDraw, ImageFilter

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent
SS = 4  # supersampling factor for every raster

# ----------------------------------------------------------------------------- palettes
DARK_TILE = dict(
    tile=("#2A3F7E", "#0B1228"), glow="#8FDCFF", glow_a=0.30,
    n=("#FFFFFF", "#BCD0F0"), handle=("#2B448C", "#14234F"), grip="#8FDCFF",
    silver=("#FFFFFF", "#A9B8D6"), lash=("#C4EEFF", "#46ADEB"), lash_hi="#FFFFFF",
    spark="#FFFFFF", spark_glow="#8FDCFF", shadow="#02050F", shadow_a=0.55, text="#F2F7FF",
)
GLYPH_DARK = dict(DARK_TILE, tile=None)
GLYPH_LIGHT = dict(
    tile=None, glow="#8FDCFF", glow_a=0.0,
    n=("#16385F", "#0B2540"), handle=("#16385F", "#0B2540"), grip="#8FDCFF",
    silver=("#5E7EA3", "#0B2540"), lash=("#5CC0F8", "#1E7FD8"), lash_hi="#E6F7FF",
    spark="#1E7FD8", spark_glow="#8FDCFF", shadow="#0B2540", shadow_a=0.28, text="#0B2540",
)


# ----------------------------------------------------------------------------- geometry
def lerp(a, b, t):
    return a + (b - a) * t


def bez(p0, p1, p2, p3, n=36):
    out = []
    for i in range(n + 1):
        t = i / n
        u = 1 - t
        out.append((u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                    u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1]))
    return out


def with_radius(pts, r0, r1, ease=1.0):
    """Attach a radius that tapers r0 -> r1 along the polyline's length."""
    acc = [0.0]
    for a, b in zip(pts, pts[1:]):
        acc.append(acc[-1] + math.dist(a, b))
    L = acc[-1] or 1.0
    return [(x, y, lerp(r0, r1, (d / L) ** ease)) for (x, y), d in zip(pts, acc)]


def capsule_poly(samples, arc=14):
    """Closed outline of a variable-width stroke with round caps. samples = [(x, y, r)]."""
    n = len(samples)
    left, right, ang = [], [], []
    for i, (x, y, r) in enumerate(samples):
        a = samples[max(i - 1, 0)]
        b = samples[min(i + 1, n - 1)]
        tx, ty = b[0] - a[0], b[1] - a[1]
        d = math.hypot(tx, ty) or 1.0
        tx, ty = tx / d, ty / d
        nx, ny = -ty, tx
        left.append((x + nx * r, y + ny * r))
        right.append((x - nx * r, y - ny * r))
        ang.append(math.atan2(ny, nx))
    poly = list(left)
    x, y, r = samples[-1]
    a0 = ang[-1]
    poly += [(x + r * math.cos(a0 - math.pi * k / arc), y + r * math.sin(a0 - math.pi * k / arc)) for k in range(1, arc)]
    poly += right[::-1]
    x, y, r = samples[0]
    a0 = ang[0]
    poly += [(x + r * math.cos(a0 + math.pi - math.pi * k / arc), y + r * math.sin(a0 + math.pi - math.pi * k / arc))
             for k in range(1, arc)]
    return poly


def line_cap(p, q, r):
    return capsule_poly([(p[0], p[1], r), (q[0], q[1], r)])


def circle(cx, cy, r, n=48):
    return [(cx + r * math.cos(2 * math.pi * i / n), cy + r * math.sin(2 * math.pi * i / n)) for i in range(n)]


def rrect(x, y, w, h, r, n=24):
    pts = []
    for cx, cy, a0 in ((x + w - r, y + r, -90), (x + w - r, y + h - r, 0), (x + r, y + h - r, 90), (x + r, y + r, 180)):
        for k in range(n + 1):
            a = math.radians(a0 + 90 * k / n)
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


def star(cx, cy, ro, ri, rot=0.0):
    pts = []
    for i in range(8):
        a = rot + i * math.pi / 4 - math.pi / 2
        r = ro if i % 2 == 0 else ri
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


# ----------------------------------------------------------------------------- layers
def solid(c, a=1.0):
    return dict(t="solid", c=c, a=a)


def lin(p, stops):
    return dict(t="lin", p=p, stops=stops)  # stops: [(offset, colour, alpha)]


def rad(cx, cy, r, c, a):
    return dict(t="rad", c=(cx, cy, r), color=c, a=a)


def layer(polys, fill, op=1.0, shadow=None, blur=None, clip=False, rule="nonzero"):
    if polys and isinstance(polys[0], tuple):
        polys = [polys]
    return dict(polys=polys, fill=fill, op=op, shadow=shadow, blur=blur, clip=clip, rule=rule)


def mark_layers(pal, simple=False, radius=116):
    """The NJ-Whip mark in 512 space. simple=True drops grips, highlights and soft shadows (<= 48 px sizes)."""
    L = []
    sh = lambda dy, s, a=1.0: None if simple else (0, dy, s, pal["shadow_a"] * a, pal["shadow"])
    if pal["tile"]:
        tile = rrect(0, 0, 512, 512, radius) if radius > 0 else [(0, 0), (512, 0), (512, 512), (0, 512)]
        L.append(dict(layer(tile, lin((0, 0, 0, 512), [(0, pal["tile"][0], 1), (1, pal["tile"][1], 1)])), role="tile"))
        L.append(layer(circle(330, 340, 300), rad(330, 340, 300, pal["glow"], pal["glow_a"]), clip=True))
        L.append(layer(rrect(0, 0, 512, 250, 0), lin((0, 0, 0, 250), [(0, "#FFFFFF", 0.13), (1, "#FFFFFF", 0.0)]), clip=True))

    # N: three round-capped strokes, white with a soft contact shadow
    w = 29
    x0, x1, ya, yb = 92, 222, 116, 318
    nfill = lin((0, ya, 0, yb), [(0, pal["n"][0], 1), (1, pal["n"][1], 1)])
    L.append(layer([line_cap((x0, ya), (x0, yb), w), line_cap((x0, ya), (x1, yb), w), line_cap((x1, ya), (x1, yb), w)],
                   nfill, shadow=sh(10, 9)))
    if not simple:  # edge light on the left stem
        L.append(layer(line_cap((x0 - 12, ya + 6), (x0 - 12, yb - 6), 3.2), solid("#FFFFFF", 0.55)))

    # J stem = handle
    hx = 338
    handle = line_cap((hx, 108), (hx, 236), 25)
    L.append(layer(handle, lin((hx - 25, 0, hx + 25, 0), [(0, pal["handle"][0], 1), (1, pal["handle"][1], 1)]),
                   shadow=sh(9, 8)))
    if not simple:
        for k in range(6):
            y = 134 + k * 17
            L.append(layer(line_cap((hx - 14, y + 5), (hx + 14, y - 5), 3.4), solid(pal["grip"], 0.9)))
    L.append(layer(circle(hx, 96, 29), lin((hx, 68, hx, 124), [(0, pal["silver"][0], 1), (1, pal["silver"][1], 1)]),
                   shadow=sh(7, 6)))
    if not simple:
        L.append(layer(circle(hx - 8, 87, 8), solid("#FFFFFF", 0.65), blur=2.5))
    L.append(layer(rrect(hx - 33, 232, 66, 18, 8, 8), lin((hx, 232, hx, 250), [(0, pal["silver"][0], 1), (1, pal["silver"][1], 1)]),
                   shadow=sh(6, 5)))

    # lash: tapering curve down the stem, round the corner, flick up at the tip
    path = bez((hx, 246), (hx, 350), (326, 430), (252, 430))[:-1] + bez((252, 430), (222, 430), (198, 424), (170, 398))
    samples = with_radius(path, 22 if simple else 21, 5.5, ease=0.8)
    lash_fill = lin((hx, 246, 150, 430), [(0, pal["lash"][0], 1), (1, pal["lash"][1], 1)])
    L.append(layer(capsule_poly(samples), lash_fill, shadow=sh(11, 9)))
    if not simple:  # a thinner, brighter ribbon riding the light side of the lash
        hi = []
        for i, (x, y, r) in enumerate(samples):
            a = samples[max(i - 1, 0)]
            b = samples[min(i + 1, len(samples) - 1)]
            tx, ty = b[0] - a[0], b[1] - a[1]
            d = math.hypot(tx, ty) or 1.0
            nx, ny = -ty / d, tx / d
            if nx + ny > 0:
                nx, ny = -nx, -ny  # toward the light (up-left)
            hi.append((x + nx * r * 0.42, y + ny * r * 0.42, max(r * 0.22, 1.2)))
        L.append(layer(capsule_poly(hi[2:-6]), solid(pal["lash_hi"], 0.85)))

    # crack spark at the tip
    sx, sy = 158, 388
    L.append(layer(circle(sx, sy, 46), rad(sx, sy, 46, pal["spark_glow"], 0.85 if pal["tile"] else 0.55)))
    L.append(layer(star(sx, sy, 32, 5.5), solid(pal["spark"]), shadow=None if simple else (0, 0, 5, 0.9, pal["spark_glow"])))
    if not simple:
        L.append(layer(star(sx, sy, 17, 4.2, math.pi / 4), solid(pal["spark"], 0.95)))
        for (dx, dy, r) in ((-46, 14, 4), (30, -44, 3.2), (-18, -48, 2.6)):
            L.append(layer(circle(sx + dx, sy + dy, r), solid(pal["spark"], 0.9)))
    return L


_WM = None


def wordmark_layers(pal, x, base, em):
    """'NJ-Whip' outlines at origin x, baseline `base`, font size `em`."""
    global _WM
    if _WM is None:
        _WM = json.loads((HERE / "wordmark.json").read_text(encoding="utf-8"))
    polys = [[(x + px * em, base + py * em) for px, py in c] for c in _WM["contours"]]
    return [layer(polys, solid(pal["text"]), rule="evenodd")], _WM["advance"] * em


def xf(layers, s=1.0, tx=0.0, ty=0.0):
    def p(pt):
        return (pt[0] * s + tx, pt[1] * s + ty)
    out = []
    for l in layers:
        n = dict(l)
        n["polys"] = [[p(q) for q in poly] for poly in l["polys"]]
        f = dict(l["fill"])
        if f["t"] == "lin":
            x0, y0, x1, y1 = f["p"]
            f["p"] = (*p((x0, y0)), *p((x1, y1)))
        elif f["t"] == "rad":
            cx, cy, r = f["c"]
            f["c"] = (*p((cx, cy)), r * s)
        n["fill"] = f
        if l["shadow"]:
            dx, dy, sg, a, c = l["shadow"]
            n["shadow"] = (dx * s, dy * s, sg * s, a, c)
        if l["blur"]:
            n["blur"] = l["blur"] * s
        out.append(n)
    return out


def lockup_layers(pal_mark, pal_text, h=512):
    """Mark tile at left, wordmark right. Canvas is h tall; returns (layers, width)."""
    ml = mark_layers(pal_mark)
    em = h * 0.56
    wl, adv = wordmark_layers(pal_text, h + h * 0.06, h * 0.5 + 0.7 * em / 2, em)
    return ml + wl, int(h + h * 0.06 + adv + h * 0.04)


# ----------------------------------------------------------------------------- SVG writer
def _f(v):
    return f"{v:.1f}".rstrip("0").rstrip(".")


def to_svg(layers, w, h, title, vb=None):
    defs, body = [], []
    uid = [0]

    def paint(f):
        if f["t"] == "solid":
            return f'fill="{f["c"]}"' + (f' fill-opacity="{f["a"]}"' if f["a"] < 1 else "")
        uid[0] += 1
        gid = f"g{uid[0]}"
        if f["t"] == "lin":
            x0, y0, x1, y1 = f["p"]
            st = "".join(f'<stop offset="{o}" stop-color="{c}"' + (f' stop-opacity="{a}"' if a < 1 else "") + "/>"
                         for o, c, a in f["stops"])
            defs.append(f'<linearGradient id="{gid}" gradientUnits="userSpaceOnUse" x1="{_f(x0)}" y1="{_f(y0)}" x2="{_f(x1)}" y2="{_f(y1)}">{st}</linearGradient>')
        else:
            cx, cy, r = f["c"]
            defs.append(f'<radialGradient id="{gid}" gradientUnits="userSpaceOnUse" cx="{_f(cx)}" cy="{_f(cy)}" r="{_f(r)}">'
                        f'<stop offset="0" stop-color="{f["color"]}" stop-opacity="{f["a"]}"/>'
                        f'<stop offset="1" stop-color="{f["color"]}" stop-opacity="0"/></radialGradient>')
        return f'fill="url(#{gid})"'

    clip_d = None
    for l in layers:
        if l.get("role") == "tile":
            clip_d = "".join("M" + "L".join(f"{_f(x)} {_f(y)}" for x, y in p) + "Z" for p in l["polys"])
    if clip_d:
        defs.append(f'<clipPath id="tile"><path d="{clip_d}"/></clipPath>')
    for i, l in enumerate(layers):
        d = "".join("M" + "L".join(f"{_f(x)} {_f(y)}" for x, y in p) + "Z" for p in l["polys"])
        attrs = [paint(l["fill"])]
        if l["rule"] == "evenodd":
            attrs.append('fill-rule="evenodd"')
        if l["clip"] and clip_d:
            attrs.append('clip-path="url(#tile)"')
        if l["shadow"]:
            dx, dy, sg, a, c = l["shadow"]
            fid = f"s{i}"
            defs.append(f'<filter id="{fid}" x="-30%" y="-30%" width="160%" height="170%" color-interpolation-filters="sRGB">'
                        f'<feDropShadow dx="{_f(dx)}" dy="{_f(dy)}" stdDeviation="{_f(sg)}" flood-color="{c}" flood-opacity="{a:.2f}"/></filter>')
            attrs.append(f'filter="url(#{fid})"')
        elif l["blur"]:
            fid = f"b{i}"
            defs.append(f'<filter id="{fid}" x="-60%" y="-60%" width="220%" height="220%"><feGaussianBlur stdDeviation="{_f(l["blur"])}"/></filter>')
            attrs.append(f'filter="url(#{fid})"')
        body.append(f'<path {" ".join(attrs)} d="{d}"/>')
    vbx = vb or f"0 0 {w} {h}"
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vbx}" width="{w}" height="{h}" role="img" aria-label="{title}">\n'
            f'<title>{title}</title>\n<defs>{"".join(defs)}</defs>\n' + "\n".join(body) + "\n</svg>\n")


# ----------------------------------------------------------------------------- raster writer
def _hex(c):
    c = c.lstrip("#")
    return tuple(int(c[i:i + 2], 16) for i in (0, 2, 4))


def _mask(layer_, size, k):
    m = None
    for poly in layer_["polys"]:
        one = Image.new("L", size, 0)
        ImageDraw.Draw(one).polygon([(x * k, y * k) for x, y in poly], fill=255)
        if m is None:
            m = one
        elif layer_["rule"] == "evenodd":
            m = ImageChops.difference(m, one)
        else:
            m = ImageChops.lighter(m, one)
    return m


def _gradient_t(size, p, k):
    x0, y0, x1, y1 = (v * k for v in p)
    dx, dy = x1 - x0, y1 - y0
    L2 = dx * dx + dy * dy or 1.0
    src = Image.new("L", (1, 1024), 0)
    col = [0] * 384 + [round(255 * i / 255) for i in range(256)] + [255] * 384
    src.putdata(col)
    a, b = 0, 0
    d, e = 256 * dx / L2, 256 * dy / L2
    f = 384 - 256 * (x0 * dx + y0 * dy) / L2
    return src.transform(size, Image.AFFINE, (a, b, 0, d, e, f), resample=Image.BILINEAR)


def _fill_image(f, size, k):
    if f["t"] == "solid":
        r, g, b = _hex(f["c"])
        return Image.new("RGBA", size, (r, g, b, round(255 * f["a"])))
    if f["t"] == "lin":
        t = _gradient_t(size, f["p"], k)
        chans = []
        for ci in range(4):
            lut = []
            for v in range(256):
                u = v / 255
                st = f["stops"]
                if u <= st[0][0]:
                    c0 = c1 = st[0]
                    q = 0
                elif u >= st[-1][0]:
                    c0 = c1 = st[-1]
                    q = 0
                else:
                    for s0, s1 in zip(st, st[1:]):
                        if s0[0] <= u <= s1[0]:
                            c0, c1 = s0, s1
                            q = (u - s0[0]) / ((s1[0] - s0[0]) or 1)
                            break
                if ci < 3:
                    lut.append(round(lerp(_hex(c0[1])[ci], _hex(c1[1])[ci], q)))
                else:
                    lut.append(round(255 * lerp(c0[2], c1[2], q)))
            chans.append(t.point(lut))
        return Image.merge("RGBA", chans)
    cx, cy, r = f["c"]
    d = max(2, round(2 * r * k))
    g = Image.radial_gradient("L").resize((d, d), Image.BILINEAR)
    a = g.point([round(f["a"] * max(0.0, 255 - v * math.sqrt(2))) for v in range(256)])
    rr, gg, bb = _hex(f["color"])
    patch = Image.new("RGBA", (d, d), (rr, gg, bb, 0))
    patch.putalpha(a)
    full = Image.new("RGBA", size, (rr, gg, bb, 0))
    full.paste(patch, (round((cx - r) * k), round((cy - r) * k)))
    return full


def raster(layers, w, h, k=SS):
    size = (round(w * k), round(h * k))
    canvas = Image.new("RGBA", size, (0, 0, 0, 0))
    tile_mask = None
    for l in layers:
        if l.get("role") == "tile":
            tile_mask = _mask(l, size, k)
    for l in layers:
        m = _mask(l, size, k)
        if l["clip"] and tile_mask is not None:
            m = ImageChops.multiply(m, tile_mask)
        if l["blur"]:
            m = m.filter(ImageFilter.GaussianBlur(l["blur"] * k))
        if l["shadow"]:
            dx, dy, sg, a, c = l["shadow"]
            sm = ImageChops.offset(m, round(dx * k), round(dy * k)).filter(ImageFilter.GaussianBlur(max(sg * k, 0.1)))
            sm = sm.point([round(a * v) for v in range(256)])
            sh = Image.new("RGBA", size, _hex(c) + (0,))
            sh.putalpha(sm)
            canvas = Image.alpha_composite(canvas, sh)
        img = _fill_image(l["fill"], size, k)
        img.putalpha(ImageChops.multiply(img.getchannel("A"), m.point([round(l["op"] * v) for v in range(256)])))
        canvas = Image.alpha_composite(canvas, img)
    return canvas.resize((round(w), round(h)), Image.LANCZOS) if k != 1 else canvas


def render_icon(px, pal=DARK_TILE, radius=116):
    """Square icon at px, drawn at px*SS from the 512 design space (simple variant at small sizes)."""
    layers = mark_layers(pal, simple=px <= 48, radius=radius)
    layers = xf(layers, px / 512)
    return raster(layers, px, px, SS if px >= 64 else 8)


# ----------------------------------------------------------------------------- outputs
def write_ico(path, sizes):
    imgs = [render_icon(s) for s in sizes]
    blobs = []
    for im in imgs:
        b = io.BytesIO()
        im.save(b, "PNG")
        blobs.append(b.getvalue())
    head = struct.pack("<HHH", 0, 1, len(sizes))
    off = 6 + 16 * len(sizes)
    ent = b""
    for s, blob in zip(sizes, blobs):
        ent += struct.pack("<BBBBHHII", s if s < 256 else 0, s if s < 256 else 0, 0, 0, 1, 32, len(blob), off)
        off += len(blob)
    pathlib.Path(path).write_bytes(head + ent + b"".join(blobs))


def build_all():
    out = HERE
    (out / "svg").mkdir(exist_ok=True)
    (out / "png").mkdir(exist_ok=True)
    docs = ROOT / "docs" / "assets"
    mark_l = mark_layers(DARK_TILE)
    (out / "svg" / "mark.svg").write_text(to_svg(mark_l, 512, 512, "NJ-Whip"), encoding="utf-8")
    (out / "svg" / "mark-glyph-dark.svg").write_text(to_svg(mark_layers(GLYPH_DARK), 512, 512, "NJ-Whip"), encoding="utf-8")
    (out / "svg" / "mark-glyph-light.svg").write_text(to_svg(mark_layers(GLYPH_LIGHT), 512, 512, "NJ-Whip"), encoding="utf-8")
    for name, pal in (("dark", DARK_TILE), ("light", GLYPH_LIGHT)):
        ll, lw = lockup_layers(DARK_TILE, pal)
        (out / "svg" / f"lockup-{name}.svg").write_text(to_svg(ll, lw, 512, "NJ-Whip"), encoding="utf-8")
    fav = to_svg(mark_layers(DARK_TILE, simple=True, radius=112), 512, 512, "NJ-Whip")
    (docs / "favicon.svg").write_text(fav, encoding="utf-8")
    (out / "svg" / "favicon.svg").write_text(fav, encoding="utf-8")
    for s in (32, 180, 192, 512):
        render_icon(s, radius=0 if s == 180 else 116).save(out / "png" / f"mark-{s}.png")
    render_icon(512).save(docs / "logo.png")
    render_icon(512).save(docs / "icon-512.png")
    render_icon(180, radius=0).save(docs / "apple-touch-icon.png")
    write_ico(ROOT / "assets" / "icon.ico", [16, 24, 32, 48, 64, 128, 256])
    write_ico(docs / "favicon.ico", [16, 32, 48])
    for name, pal in (("dark", DARK_TILE), ("light", GLYPH_LIGHT)):
        ll, lw = lockup_layers(DARK_TILE, pal)
        raster(ll, lw, 512, SS).resize((lw // 2, 256), Image.LANCZOS).save(out / "png" / f"lockup-{name}.png")
    print("written: svg x6, png x6, icon.ico, docs/assets logo/favicon/apple-touch/icon-512")


def sheet(path):
    W, H = 2000, 1300
    sh = Image.new("RGBA", (W, H), "#0A1024")
    light = Image.new("RGBA", (W, 560), "#EEF6FB")
    sh.paste(light, (0, 740))
    d = ImageDraw.Draw(sh)

    def put(im, x, y):
        sh.alpha_composite(im, (x, y))

    # row 1 (dark): tile 440, glyph, lockup
    put(render_icon(440), 40, 40)
    g = raster(xf(mark_layers(GLYPH_DARK), 440 / 512), 440, 440, SS)
    put(g, 520, 40)
    ll, lw = lockup_layers(DARK_TILE, DARK_TILE)
    lk = raster(ll, lw, 512, SS).resize((int(lw * 0.5), 256), Image.LANCZOS)
    put(lk, 1000, 60)
    # size ladder on dark, 1x and 6x nearest for 16/32
    x = 1000
    for s in (16, 24, 32, 48, 64, 128):
        im = render_icon(s)
        put(im, x, 360)
        x += s + 24
    x = 1000
    for s in (16, 32):
        im = render_icon(s).resize((s * 6, s * 6), Image.NEAREST)
        put(im, x, 480 if s == 16 else 480)
        x += s * 6 + 30
    # row 2 (light)
    put(render_icon(440), 40, 780)
    g2 = raster(xf(mark_layers(GLYPH_LIGHT), 440 / 512), 440, 440, SS)
    put(g2, 520, 780)
    ll2, lw2 = lockup_layers(DARK_TILE, GLYPH_LIGHT)
    lk2 = raster(ll2, lw2, 512, SS).resize((int(lw2 * 0.5), 256), Image.LANCZOS)
    put(lk2, 1000, 800)
    x = 1000
    for s in (16, 32, 48, 64, 128, 192):
        put(render_icon(s), x, 1100)
        x += s + 24
    ap = render_icon(180, radius=0)
    put(ap, 1000 + 16 + 32 + 48 + 64 + 128 + 192 + 6 * 24 + 20, 1100 - 0) if False else None
    d.text((40, 1260), "tile / glyph / lockup on night and on paper; ladder 16-192; 6x zoom of 16 and 32", fill="#8FA3C8")
    pathlib.Path(path).parent.mkdir(parents=True, exist_ok=True)
    sh.convert("RGB").save(path)
    print("sheet:", path)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--sheet", help="write a review contact sheet to this PNG and nothing else")
    a = ap.parse_args()
    if a.sheet:
        sheet(a.sheet)
    else:
        build_all()
