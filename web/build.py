"""Builds everything the website takes from the app. Run from anywhere:

    python web/build.py            (add --offline to build without the network)

1. Builds the engine to WebAssembly (release) and copies it to docs/assets/nj-whip.wasm.
2. Copies every sound file named in src/clips.rs from assets/sounds/ to docs/assets/sounds/.
3. Puts web/site.js into both pages as their inline script and renews its hash in the
   Content-Security-Policy (the pages allow inline scripts by hash only).
4. Writes the sound credits table of both pages from CREDITS.md.
5. Runs web/check.mjs on the result when node is installed.

It keeps no list of its own: whips, clips and credits are read from the app's files.
"""
import base64
import hashlib
import html
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WEB, DOCS = ROOT / "web", ROOT / "docs"
PAGES = [DOCS / "index.html", DOCS / "ar" / "index.html"]


def build_wasm():
    cmd = ["cargo", "build", "--release", "--locked"] + (["--offline"] if "--offline" in sys.argv else [])
    subprocess.run(cmd, cwd=WEB, check=True)
    out = WEB / "target" / "wasm32-unknown-unknown" / "release" / "nj_whip_web.wasm"
    shutil.copyfile(out, DOCS / "assets" / "nj-whip.wasm")
    print(f"wasm    {out.stat().st_size / 1024:.0f} KB -> docs/assets/nj-whip.wasm")


def copy_sounds():
    names = re.findall(r'clip!\(\s*\w+\s*,\s*"([^"]+)"\s*\)', (ROOT / "src" / "clips.rs").read_text(encoding="utf-8"))
    if not names:
        sys.exit("src/clips.rs names no clips: its clip!(NAME, \"file\") lines were not found")
    dest = DOCS / "assets" / "sounds"
    dest.mkdir(parents=True, exist_ok=True)
    for n in names:
        shutil.copyfile(ROOT / "assets" / "sounds" / n, dest / n)
    stale = sorted(p.name for p in dest.iterdir() if p.name not in names)
    print(f"sounds  {len(names)} clips -> docs/assets/sounds/" + (f"  (no longer named in clips.rs, remove by hand: {', '.join(stale)})" if stale else ""))


def credits_rows():
    """CREDITS.md table rows as (file, source url, author, licence)."""
    rows = []
    for line in (ROOT / "CREDITS.md").read_text(encoding="utf-8").splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) == 4 and cells[0].startswith("assets/sounds/"):
            licence = re.sub(r"\*\*|\s*\(https?://[^)]*\)", "", cells[3]).split(" — ")[0].split(" - ")[0].strip()
            rows.append((cells[0].rsplit("/", 1)[1], cells[1], cells[2], licence))
    if not rows:
        sys.exit("CREDITS.md: no sound rows found")
    return rows


def credits_html(rows):
    e = html.escape
    return "\n".join(
        f'            <tr><th scope="row"><bdi>{e(f)}</bdi></th><td><a href="{e(src)}" rel="noopener"><bdi>{e(author)}</bdi></a></td><td><bdi>{e(lic)}</bdi></td></tr>'
        for f, src, author, lic in rows
    )


def between(text, name, body, page):
    """Replaces what sits between <!--name--> and <!--/name-->."""
    pat = re.compile(rf"(<!--{name}-->)(.*?)(<!--/{name}-->)", re.S)
    if not pat.search(text):
        sys.exit(f"{page}: the markers <!--{name}--> ... <!--/{name}--> are missing")
    return pat.sub(lambda m: m.group(1) + "\n" + body + "\n" + m.group(3), text, count=1)


def patch_pages():
    js = (WEB / "site.js").read_text(encoding="utf-8").replace("\r\n", "\n").strip()
    if "</script" in js.lower():
        sys.exit("web/site.js holds '</script', which would end the inline script")
    digest = "sha256-" + base64.b64encode(hashlib.sha256(js.encode("utf-8")).digest()).decode()
    rows = credits_html(credits_rows())
    for page in PAGES:
        text = page.read_text(encoding="utf-8")
        script = re.compile(r"(<script data-site[^>]*>)(.*?)(</script>)", re.S)
        if not script.search(text):
            sys.exit(f"{page}: <script data-site> not found")
        text = script.sub(lambda m: m.group(1) + js + m.group(3), text, count=1)
        text, n = re.subn(r"(script-src[^;]*?)'sha256-[A-Za-z0-9+/=]+'", lambda m: m.group(1) + f"'{digest}'", text, count=1)
        if n != 1:
            sys.exit(f"{page}: no script hash found in the Content-Security-Policy")
        text = between(text, "credits", rows, page)
        page.write_text(text, encoding="utf-8", newline="\n")
    print(f"pages   site.js inlined in {len(PAGES)} pages, CSP hash {digest}")


def check():
    node = shutil.which("node")
    if not node:
        print("check   skipped: node is not installed")
        return
    subprocess.run([node, str(WEB / "check.mjs")], check=True)


if __name__ == "__main__":
    build_wasm()
    copy_sounds()
    patch_pages()
    check()
