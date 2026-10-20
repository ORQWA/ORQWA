"""Draws the ORQWA wordmark and the lockups (ADR 0026) as SVG.

The letters were measured from the wordmark design (2000 px wide, caps 231 px high) and are
drawn here as plain geometry: caps 100 units high on a baseline at 0, so every size and colour
comes from one source. Rerun after changing the geometry, then `scripts/render-splash.sh`:

    scripts/dev.sh python3 scripts/brand/wordmark.py
"""

import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BRAND = ROOT / "docs" / "brand"
APP_BRAND = ROOT / "app" / "assets" / "brand"

# Light backgrounds take the dark ink, dark ones the light ink, as the mark does.
INK = {"light": "#111418", "dark": "#F2F4F7"}
BLUE = {"light": "#0A9BF5", "dark": "#2EB0FF"}

# Room around the letters in the wordmark files.
MARGIN = 20.0
# Where each letter's box starts, as in the design.
LAYOUT = {"o": 0.0, "r": 151.1, "q": 280.9, "w": 415.6, "a": 585.1}


def num(value):
    return f"{value:.2f}".rstrip("0").rstrip(".")


def rounded_rect(x0, y0, x1, y1, radii):
    """A rectangle with its corners rounded: radii = top-left, top-right, bottom-right, bottom-left."""
    tl, tr, br, bl = radii
    d = [f"M{num(x0 + tl)} {num(y0)}", f"H{num(x1 - tr)}"]
    if tr:
        d.append(f"A{num(tr)} {num(tr)} 0 0 1 {num(x1)} {num(y0 + tr)}")
    d.append(f"V{num(y1 - br)}")
    if br:
        d.append(f"A{num(br)} {num(br)} 0 0 1 {num(x1 - br)} {num(y1)}")
    d.append(f"H{num(x0 + bl)}")
    if bl:
        d.append(f"A{num(bl)} {num(bl)} 0 0 1 {num(x0)} {num(y1 - bl)}")
    d.append(f"V{num(y0 + tl)}")
    if tl:
        d.append(f"A{num(tl)} {num(tl)} 0 0 1 {num(x0 + tl)} {num(y0)}")
    return " ".join(d) + " Z"


def polygon(points, radii=None):
    """A closed polygon, each corner rounded by its radius (0 = sharp)."""
    radii = radii or [0] * len(points)
    parts = []
    for i, (p, r) in enumerate(zip(points, radii)):
        if not r:
            parts.append(("L", p))
            continue
        a, b = points[i - 1], points[(i + 1) % len(points)]
        ux, uy = a[0] - p[0], a[1] - p[1]
        vx, vy = b[0] - p[0], b[1] - p[1]
        lu, lv = math.hypot(ux, uy), math.hypot(vx, vy)
        ux, uy, vx, vy = ux / lu, uy / lu, vx / lv, vy / lv
        reach = r / math.tan(math.acos(max(-1.0, min(1.0, ux * vx + uy * vy))) / 2)
        turn = (p[0] - a[0]) * (b[1] - p[1]) - (p[1] - a[1]) * (b[0] - p[0])
        parts.append(("L", (p[0] + ux * reach, p[1] + uy * reach)))
        parts.append(("A", (p[0] + vx * reach, p[1] + vy * reach), r, 1 if turn > 0 else 0))
    d = []
    for i, part in enumerate(parts):
        if part[0] == "L":
            d.append(("M" if i == 0 else "L") + f"{num(part[1][0])} {num(part[1][1])}")
        else:
            d.append(f"A{num(part[2])} {num(part[2])} 0 0 {part[3]} {num(part[1][0])} {num(part[1][1])}")
    return " ".join(d) + " Z"


def circle(cx, cy, r):
    return (f"M{num(cx - r)} {num(cy)} A{num(r)} {num(r)} 0 1 0 {num(cx + r)} {num(cy)} "
            f"A{num(r)} {num(r)} 0 1 0 {num(cx - r)} {num(cy)} Z")


def letter_o(x):
    """A rounded rectangle around a rounded counter; sides heavier than top and bottom."""
    return [rounded_rect(x, -99, x + 139.8, 0, (42,) * 4) + " "
            + rounded_rect(x + 28.6, -74, x + 111.2, -26, (17,) * 4)]


def letter_r(x):
    """Stem, a bowl rounded on the right, and a leg out of the bowl's bottom: one outline."""
    stem, bowl_right, bowl_bottom, r_top, r_bottom = 26.4, 119.9, -29.0, 34.6, 36.0
    slope, leg_left, leg_right = 1.16, 90.5, 128.0
    # Where the leg's right edge leaves the bowl's lower corner.
    cx, cy = bowl_right - r_bottom, bowl_bottom - r_bottom
    lo, hi = cy, bowl_bottom
    for _ in range(60):
        mid = (lo + hi) / 2
        if (leg_right + slope * mid - cx) ** 2 + (mid - cy) ** 2 < r_bottom**2:
            lo = mid
        else:
            hi = mid
    jy = (lo + hi) / 2
    outline = (
        f"M{num(x)} 0 V-100 H{num(x + bowl_right - r_top)} "
        f"A{num(r_top)} {num(r_top)} 0 0 1 {num(x + bowl_right)} {num(-100 + r_top)} V{num(cy)} "
        f"A{num(r_bottom)} {num(r_bottom)} 0 0 1 {num(x + leg_right + slope * jy)} {num(jy)} "
        f"L{num(x + leg_right)} 0 H{num(x + leg_left)} L{num(x + leg_left + slope * bowl_bottom)} "
        f"{num(bowl_bottom)} H{num(x + stem)} V0 Z")
    counter = rounded_rect(x + stem, -74.5, x + 93.5, -53.2, (0, 10, 10, 0))
    return [outline + " " + counter]


def letter_q(x):
    """The ring, the mark's blue crescent inside it and the tail crossing its lower right.

    Returns the ring, the tail, the band the ring loses beside the tail, the crescent before it
    is cut, the circle cut from it and the area left of the tail it keeps."""
    left = lambda y: x + 100.4 + 0.93 * y  # noqa: E731 the tail's edges
    right = lambda y: x + 131.6 + y  # noqa: E731
    top, bottom, gap = -33.3, 1.0, 6.5
    ring = (rounded_rect(x, -100, x + 136.4, 1, (42,) * 4) + " "
            + rounded_rect(x + 28.6, -75.3, x + 107.8, -17.5, (24, 24, 24, 30)))
    # The tail's foot runs on under the ring's bottom, so the two share one flat edge.
    tail = polygon([(left(top), top), (right(top), top), (right(bottom), bottom), (x + 90, bottom),
                    (x + 90, -8), (left(-8), -8)])
    band = polygon([(right(-40), -40), (right(-40) + gap, -40), (right(10) + gap, 10), (right(10), 10)])
    inset = 5.2
    crescent = rounded_rect(x + 28.6 + inset, -75.3 + inset, x + 107.8 - inset, -17.5 - inset,
                            (24 - inset, 24 - inset, 24 - inset, 30 - inset))
    bite = circle(x + 72.73, -65.33, 34.74)
    keep = polygon([(x - 20, -120), (left(-120) - gap, -120), (left(20) - gap, 20), (x - 20, 20)])
    return ring, tail, band, crescent, bite, keep


def letter_w(x):
    """Four strokes, the middle apex and the two bottoms rounded."""
    points = [(0, -100), (30.5, -100), (59.5, -37.7), (87.9, -100), (109.1, -100), (137.5, -37.7),
              (166.5, -100), (197, -100), (151, 0), (128.2, 0), (98.5, -57.6), (68.8, 0), (46, 0)]
    radii = [0, 0, 0, 4, 4, 0, 0, 0, 9, 9, 0, 9, 9]
    return [polygon([(x + px, py) for px, py in points], radii)]


def letter_a(x):
    """Two strokes under a rounded top, no crossbar."""
    width, slope, top, stroke = 155.4, 0.641, 27.2, 32.0
    notch = (width / 2 - stroke) / slope
    points = [(0, 0), (slope * 100, -100), (slope * 100 + top, -100), (width, 0), (width - stroke, 0),
              (width / 2, -notch), (stroke, 0)]
    return [polygon([(x + px, py) for px, py in points], [0, 11, 11, 0, 0, 2, 0])]


WIDTH = LAYOUT["a"] + 155.4


def wordmark(theme, prefix):
    """The wordmark's clip paths and drawing for `theme`, in its own units; `prefix` keeps the
    clip paths' ids apart where several drawings share a page."""
    ring, tail, band, crescent, bite, keep = letter_q(LAYOUT["q"])
    room = "M-100 -200 H900 V100 H-100 Z"
    defs = (f'<clipPath id="{prefix}-q-gap"><path clip-rule="evenodd" d="{room} {band}"/></clipPath>'
            f'<clipPath id="{prefix}-q-bite"><path clip-rule="evenodd" d="{room} {bite}"/></clipPath>'
            f'<clipPath id="{prefix}-q-keep"><path d="{keep}"/></clipPath>')
    shapes = letter_o(LAYOUT["o"]) + letter_r(LAYOUT["r"])
    body = (f'<g fill="{INK[theme]}" fill-rule="evenodd">'
            + "".join(f'<path d="{d}"/>' for d in shapes)
            + f'<path clip-path="url(#{prefix}-q-gap)" d="{ring}"/><path d="{tail}"/>'
            + "".join(f'<path d="{d}"/>' for d in letter_w(LAYOUT["w"]) + letter_a(LAYOUT["a"]))
            + f'</g><g clip-path="url(#{prefix}-q-keep)"><path fill="{BLUE[theme]}" '
            f'clip-path="url(#{prefix}-q-bite)" d="{crescent}"/></g>')
    return defs, body


def wordmark_svg(theme):
    left, top, bottom = -MARGIN, -100 - MARGIN, 1 + MARGIN
    defs, body = wordmark(theme, "wordmark")
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{num(left)} {num(top)} '
            f'{num(WIDTH + 2 * MARGIN)} {num(bottom - top)}" role="img" aria-labelledby="title">\n'
            f'  <title id="title">ORQWA</title>\n  <defs>{defs}</defs>\n  {body}\n</svg>\n')


def lockup_svg(theme):
    """The mark (from its own file) above the wordmark, which takes the mark's width."""
    logo = (BRAND / f"orqwa-logo-{theme}.svg").read_text()
    mark_defs = re.search(r"<defs>(.*?)</defs>", logo, re.S).group(1)
    mark = logo[logo.index("</defs>") + len("</defs>"):logo.rindex("</svg>")]
    scale = 560 / WIDTH
    baseline = 700 + 100 * scale
    bottom = baseline + scale + 36
    defs, body = wordmark(theme, "lockup")
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="74 64 612 {num(bottom - 64)}" role="img" '
            f'aria-labelledby="title">\n  <title id="title">ORQWA</title>\n  <defs>{mark_defs}  {defs}\n'
            f'  </defs>{mark.rstrip()}\n  <g transform="translate({num(380 - WIDTH * scale / 2)} '
            f'{num(baseline)}) scale({scale:.4f})">{body}</g>\n</svg>\n')


def main():
    for theme in ("light", "dark"):
        (BRAND / f"orqwa-wordmark-{theme}.svg").write_text(wordmark_svg(theme))
        (BRAND / f"orqwa-lockup-{theme}.svg").write_text(lockup_svg(theme))
    (APP_BRAND / "orqwa-wordmark-dark.svg").write_text(wordmark_svg("dark"))


if __name__ == "__main__":
    main()
