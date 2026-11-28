"""One-off: flatten the glyph outlines of 'NJ-Whip' (Thmanyah Serif Display Bold) into wordmark.json.

    uv run --with fonttools python assets/brand/extract_wordmark.py <path-to-thmanyahserifdisplay-Bold.otf>

The JSON holds closed polylines (em-units / unitsPerEm, y down), so make_brand.py needs only Pillow.
"""
import json, sys, pathlib
from fontTools.ttLib import TTFont
from fontTools.pens.recordingPen import DecomposingRecordingPen

TEXT = "NJ-Whip"
font = TTFont(sys.argv[1])
upm = font["head"].unitsPerEm
gs, cmap, hmtx = font.getGlyphSet(), font.getBestCmap(), font["hmtx"]

def bez(p0, p1, p2, p3, n=14):
    return [tuple((1-t)**3*a + 3*(1-t)**2*t*b + 3*(1-t)*t*t*c + t**3*d for a, b, c, d in zip(p0, p1, p2, p3))
            for t in [i/n for i in range(1, n+1)]]

def quad(p0, p1, p2, n=10):
    return [tuple((1-t)**2*a + 2*(1-t)*t*b + t*t*c for a, b, c in zip(p0, p1, p2))
            for t in [i/n for i in range(1, n+1)]]

x = 0.0
contours = []
for ch in TEXT:
    gname = cmap[ord(ch)]
    pen = DecomposingRecordingPen(gs)
    gs[gname].draw(pen)
    cur, start = None, None
    pts = []
    for op, args in pen.value:
        if op == "moveTo":
            pts = [args[0]]; cur = start = args[0]
        elif op == "lineTo":
            pts.append(args[0]); cur = args[0]
        elif op == "curveTo":
            pts += bez(cur, *args); cur = args[-1]
        elif op == "qCurveTo":
            # implied on-curve points between consecutive off-curve points
            offs = list(args[:-1]); end = args[-1]
            for i, o in enumerate(offs):
                nxt = end if i == len(offs)-1 else ((o[0]+offs[i+1][0])/2, (o[1]+offs[i+1][1])/2)
                pts += quad(cur, o, nxt); cur = nxt
        elif op in ("closePath", "endPath"):
            contours.append([(round((px + x)/upm, 4), round(-py/upm, 4)) for px, py in pts])
            pts = []
    x += hmtx[gname][0]
asc = font["hhea"].ascent / upm
out = {"text": TEXT, "advance": round(x/upm, 4), "capHeight": round(getattr(font["OS/2"], "sCapHeight", 0)/upm, 4),
       "contours": contours}
p = pathlib.Path(__file__).with_name("wordmark.json")
p.write_text(json.dumps(out, separators=(",", ":")), encoding="utf-8")
print(len(contours), "contours,", p.stat().st_size, "bytes, advance", out["advance"], "cap", out["capHeight"])
