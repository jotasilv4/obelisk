#!/usr/bin/env python3
"""Gerador dos assets SVG originais do Obelisk.

Tudo aqui é desenho original (nenhum asset da Apple). Rode:
    python3 branding/gen.py
Depois: branding/render.sh para gerar os PNGs.
"""
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))
OUT_SVG = os.path.join(HERE, "svg")
OUT_ICONS = os.path.join(OUT_SVG, "icons")
OUT_WALL = os.path.join(HERE, "wallpapers")
for d in (OUT_SVG, OUT_ICONS, OUT_WALL):
    os.makedirs(d, exist_ok=True)


def squircle(cx, cy, r, n=5.0, steps=240):
    """Superelipse |x/r|^n + |y/r|^n = 1 como path SVG."""
    pts = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        x = cx + r * math.copysign(abs(c) ** (2 / n), c)
        y = cy + r * math.copysign(abs(s) ** (2 / n), s)
        pts.append(f"{x:.2f},{y:.2f}")
    return "M" + " L".join(pts) + " Z"


def write(path, text):
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


# ---------------------------------------------------------------- marca
def _symbol_d():
    import re
    src = open(os.path.join(HERE, "obelisk-symbol.svg"), encoding="utf-8").read()
    return re.search(r' d="([^"]+)"', src).group(1)


def obelisk_glyph(fill_l="#ffffff", fill_r=None, fill_cap_l=None, fill_cap_r=None, base=None, size=360):
    """Símbolo do Obelisk (branding/obelisk-symbol.svg, hexágono de quatro peças) num viewBox 512,
    centrado em (256, 256) com `size` px de altura. Os demais parâmetros ficam por compatibilidade."""
    k = size / 87.2
    return f'''
  <path transform="translate({256 - 50 * k:.2f} {256 - 50 * k:.2f}) scale({k:.4f})" fill-rule="evenodd" fill="{fill_l}" d="{_symbol_d()}"/>
'''


def mark_svg(mono=None):
    if mono:
        g = obelisk_glyph(mono, mono, mono, mono, mono)
        # no mono, separa as faces com um filete (linha central transparente via mask simples)
        g += f'<path d="M256 78 L256 420" stroke="#00000000" />'
        body = f'<g fill="{mono}">{g}</g>'
        bg = ""
    else:
        body = obelisk_glyph()
        bg = ""
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <title>Obelisk — marca</title>
  {bg}{body}
</svg>
"""


def mark_gradient_svg():
    """Marca colorida (para fundo claro): faces em azul/índigo."""
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <title>Obelisk — marca (cor)</title>
  <defs>
    <linearGradient id="l" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#6aa3ff"/><stop offset="1" stop-color="#2a3fa8"/></linearGradient>
    <linearGradient id="r" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3b6fe8"/><stop offset="1" stop-color="#2a3fa8"/></linearGradient>
  </defs>
  {obelisk_glyph("url(#l)")}
</svg>
"""


def app_icon_svg():
    sq = squircle(256, 256, 232)
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <title>Obelisk — ícone do app</title>
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0.4" y2="1"><stop offset="0" stop-color="#4a6df0"/><stop offset="0.55" stop-color="#2d3fb5"/><stop offset="1" stop-color="#171d5c"/></linearGradient>
    <linearGradient id="gloss" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity=".38"/><stop offset="0.5" stop-color="#fff" stop-opacity="0"/></linearGradient>
    <clipPath id="c"><path d="{sq}"/></clipPath>
    <filter id="sh" x="-20%" y="-20%" width="140%" height="150%"><feGaussianBlur stdDeviation="8"/></filter>
  </defs>
  <path d="{sq}" fill="#000" opacity=".35" filter="url(#sh)" transform="translate(0 8)"/>
  <path d="{sq}" fill="url(#bg)"/>
  <g clip-path="url(#c)"><rect x="0" y="0" width="512" height="260" fill="url(#gloss)"/></g>
  <g transform="translate(0 4)">{obelisk_glyph(size=270)}</g>
  <path d="{sq}" fill="none" stroke="#fff" stroke-opacity=".25" stroke-width="2"/>
</svg>
"""


def wordmark_svg(dark=False):
    txt = "#ffffff" if dark else "#14182e"
    glyph = obelisk_glyph("#ffffff", size=320) if dark else \
        obelisk_glyph("#3557d6", size=320)
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1100 512" width="1100" height="512">
  <title>Obelisk — wordmark</title>
  <g transform="translate(-20 0)">{glyph}</g>
  <text x="420" y="322" font-family="Inter, 'Inter Variable', Helvetica, Arial, sans-serif" font-weight="600"
        font-size="188" letter-spacing="-4" fill="{txt}">Obelisk</text>
</svg>
"""


# ---------------------------------------------------------------- wallpapers
def wallpaper(dark, w=3840, h=2160):
    if dark:
        bg = ("#070a1f", "#141a4a")
        blobs = [(0.18, 0.25, 900, "#3b4de0", .75), (0.78, 0.30, 800, "#8a3ff0", .55),
                 (0.55, 0.85, 1000, "#1f7ae0", .55), (0.10, 0.90, 700, "#c43fb0", .35),
                 (0.92, 0.88, 600, "#2fd0d8", .25)]
        mark_op, mark_fill = .05, "#ffffff"
    else:
        bg = ("#e9efff", "#f6e9f7")
        blobs = [(0.15, 0.20, 900, "#7aa7ff", .85), (0.80, 0.25, 800, "#c9a2ff", .75),
                 (0.55, 0.88, 1000, "#8fe0ff", .70), (0.08, 0.88, 700, "#ffb8d9", .70),
                 (0.93, 0.85, 600, "#ffd9a0", .55)]
        mark_op, mark_fill = .06, "#2f4fc4"
    circles = "\n    ".join(
        f'<circle cx="{int(w*x)}" cy="{int(h*y)}" r="{r}" fill="{c}" opacity="{o}"/>' for x, y, r, c, o in blobs)
    s = h / 512 * 0.95
    mx = w * 0.5 - 256 * s
    my = h * 0.02
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">
  <title>Obelisk — wallpaper {'escuro' if dark else 'claro'}</title>
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{bg[0]}"/><stop offset="1" stop-color="{bg[1]}"/></linearGradient>
    <filter id="blur" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="240"/></filter>
    <filter id="soft" x="-10%" y="-10%" width="120%" height="120%"><feGaussianBlur stdDeviation="3"/></filter>
  </defs>
  <rect width="{w}" height="{h}" fill="url(#bg)"/>
  <g filter="url(#blur)">
    {circles}
  </g>
  <g transform="translate({mx:.1f} {my:.1f}) scale({s:.4f})" opacity="{mark_op}" filter="url(#soft)">
    {obelisk_glyph(mark_fill, mark_fill, mark_fill, mark_fill, mark_fill)}
  </g>
</svg>
"""


def login_svg(w=3840, h=2160):
    s = 1.5
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">
  <title>Obelisk — boot / login</title>
  <defs>
    <radialGradient id="bg" cx="0.5" cy="0.45" r="0.8"><stop offset="0" stop-color="#1b2370"/><stop offset="0.55" stop-color="#0b0f33"/><stop offset="1" stop-color="#04061a"/></radialGradient>
    <filter id="blur" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="200"/></filter>
    <filter id="glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="30"/></filter>
  </defs>
  <rect width="{w}" height="{h}" fill="url(#bg)"/>
  <g filter="url(#blur)" opacity=".7">
    <circle cx="{w*0.25}" cy="{h*0.8}" r="700" fill="#4a3fe0"/>
    <circle cx="{w*0.78}" cy="{h*0.25}" r="650" fill="#8a3ff0" opacity=".7"/>
  </g>
  <g transform="translate({w/2-256*s:.1f} {h/2-250*s:.1f}) scale({s})">
    <g filter="url(#glow)" opacity=".55">{obelisk_glyph("#7aa7ff","#7aa7ff","#7aa7ff","#7aa7ff","#7aa7ff")}</g>
    {obelisk_glyph()}
  </g>
</svg>
"""


# ---------------------------------------------------------------- ícones
SQ = squircle(256, 256, 232)


def icon(name, grad, glyph, title=None, stroke_hi=True, extra_defs=""):
    a, b = grad
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <title>{title or name}</title>
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{a}"/><stop offset="1" stop-color="{b}"/></linearGradient>
    <linearGradient id="gloss" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity=".40"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></linearGradient>
    <clipPath id="c"><path d="{SQ}"/></clipPath>
    <filter id="sh" x="-20%" y="-20%" width="140%" height="150%"><feGaussianBlur stdDeviation="7"/></filter>
    <filter id="gs" x="-20%" y="-20%" width="140%" height="150%"><feGaussianBlur stdDeviation="3"/></filter>
    {extra_defs}
  </defs>
  <path d="{SQ}" fill="#000" opacity=".30" filter="url(#sh)" transform="translate(0 7)"/>
  <path d="{SQ}" fill="url(#bg)"/>
  {glyph}
  <g clip-path="url(#c)"><rect width="512" height="250" fill="url(#gloss)"/></g>
  <path d="{SQ}" fill="none" stroke="#fff" stroke-opacity=".28" stroke-width="2"/>
</svg>
"""


def gear(cx, cy, r_out, r_in, teeth=8, hole=48):
    pts = []
    for i in range(teeth):
        a0 = 2 * math.pi * i / teeth
        step = 2 * math.pi / teeth
        for frac, rad in ((0.0, r_in), (0.18, r_out), (0.50, r_out), (0.68, r_in)):
            a = a0 + step * frac
            pts.append(f"{cx + rad*math.cos(a):.1f},{cy + rad*math.sin(a):.1f}")
    return (f'<path fill-rule="evenodd" d="M{" L".join(pts)} Z '
            f'M{cx+hole},{cy} A{hole},{hole} 0 1 0 {cx-hole},{cy} A{hole},{hole} 0 1 0 {cx+hole},{cy} Z"/>')


def build_icons():
    icons = {}

    # Files — pasta
    icons["files"] = icon("Arquivos", ("#6cb4ff", "#2c6be0"), """
  <g filter="url(#gs)" opacity=".25" transform="translate(0 8)"><path d="M110 190 h110 l30 32 h152 a22 22 0 0 1 22 22 v118 a22 22 0 0 1 -22 22 h-292 a22 22 0 0 1 -22 -22 v-150 a22 22 0 0 1 22 -22z" fill="#000"/></g>
  <path d="M108 168 h112 l32 34 h152 a24 24 0 0 1 24 24 v26 h-344 v-60 a24 24 0 0 1 24 -24z" fill="#cfe4ff"/>
  <path d="M84 218 h344 a24 24 0 0 1 24 24 v110 a24 24 0 0 1 -24 24 h-344 a24 24 0 0 1 -24 -24 v-110 a24 24 0 0 1 24 -24z" fill="#fff"/>
""")

    # Terminal
    icons["terminal"] = icon("Terminal", ("#3a3f55", "#0d0f1a"), """
  <path d="M130 188 L228 256 L130 324" fill="none" stroke="#7dffb0" stroke-width="34" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M262 332 H380" fill="none" stroke="#fff" stroke-width="34" stroke-linecap="round"/>
""")

    # Browser — globo
    icons["browser"] = icon("Navegador", ("#5fd4ff", "#2467e8"), """
  <g fill="none" stroke="#fff" stroke-width="22" stroke-linecap="round">
    <circle cx="256" cy="256" r="142"/>
    <ellipse cx="256" cy="256" rx="62" ry="142"/>
    <path d="M114 256 H398"/><path d="M134 188 H378" stroke-width="14"/><path d="M134 324 H378" stroke-width="14"/>
  </g>
""")

    # Settings — engrenagem
    icons["settings"] = icon("Ajustes", ("#b9bfd0", "#6b7186"), f"""
  <g fill="#fff" transform="translate(0 6)" opacity=".25" filter="url(#gs)">{gear(256,256,150,118)}</g>
  <g fill="#fff">{gear(256,256,150,118)}</g>
""")

    # Launcher — grade 3x3
    cells = "".join(
        f'<rect x="{110+i*100}" y="{110+j*100}" width="76" height="76" rx="22" fill="#fff" opacity="{0.95 if (i+j)%2==0 else 0.75}"/>'
        for i in range(3) for j in range(3))
    icons["launcher"] = icon("Launcher", ("#a07bff", "#4a2fd0"), cells)

    # Trash — lixeira
    icons["trash"] = icon("Lixeira", ("#e3e8f2", "#9aa3b8"), """
  <g fill="#5b6479">
    <path d="M168 190 h176 l-14 170 a24 24 0 0 1 -24 22 h-100 a24 24 0 0 1 -24 -22z"/>
    <rect x="146" y="150" width="220" height="26" rx="13"/>
    <path d="M214 138 a14 14 0 0 1 14 -14 h56 a14 14 0 0 1 14 14 v16 h-84z"/>
  </g>
  <g stroke="#e3e8f2" stroke-width="14" stroke-linecap="round"><path d="M222 220 v120"/><path d="M256 220 v120"/><path d="M290 220 v120"/></g>
""")

    # Notes
    lines = "".join(f'<path d="M150 {236+i*48} H362" stroke="#d8dce8" stroke-width="10" stroke-linecap="round"/>' for i in range(4))
    icons["notes"] = icon("Notas", ("#fff3a0", "#ffd23a"), f"""
  <rect x="102" y="96" width="308" height="320" rx="34" fill="#fff"/>
  <path d="M102 130 a34 34 0 0 1 34 -34 h240 a34 34 0 0 1 34 34 v34 h-308z" fill="#ffc933"/>
  {lines}
""")

    # Music — colcheia
    icons["music"] = icon("Música", ("#ff7a9c", "#e0245e"), """
  <g fill="#fff">
    <path d="M214 134 L378 104 V318 a52 46 0 1 1 -28 -41 V188 L242 208 V360 a52 46 0 1 1 -28 -41z"/>
  </g>
""")

    # Photos — paisagem (sol + montanhas), original
    icons["photos"] = icon("Fotos", ("#8fd3ff", "#4a8cf0"), """
  <g clip-path="url(#c)">
    <circle cx="342" cy="170" r="48" fill="#fff6c2"/>
    <path d="M20 400 L170 220 L260 330 L330 260 L492 430 V492 H20 Z" fill="#2f4fc4" opacity=".55"/>
    <path d="M20 440 L150 300 L240 400 L330 320 L492 470 V492 H20 Z" fill="#1c2f8f"/>
  </g>
""")

    # Calendar — folha com grade de pontos (sem texto)
    dots = "".join(
        f'<circle cx="{138+i*60}" cy="{250+j*52}" r="{9}" fill="{"#ff3b4a" if (i,j)==(3,1) else "#8a93a8"}"/>'
        for i in range(6) for j in range(3))
    icons["calendar"] = icon("Calendário", ("#ffffff", "#eef0f6"), f"""
  <path d="M60 140 a24 24 0 0 1 24 -24 h344 a24 24 0 0 1 24 24 v34 h-392z" fill="#ff4458"/>
  <rect x="148" y="84" width="22" height="56" rx="11" fill="#c9283b"/><rect x="342" y="84" width="22" height="56" rx="11" fill="#c9283b"/>
  {dots}
""")

    # Calculator
    keys = ""
    for j in range(3):
        for i in range(4):
            col = "#ff9f2e" if i == 3 else ("#6a7088" if j == 0 else "#4a4f64")
            keys += f'<rect x="{104+i*80}" y="{212+j*74}" width="64" height="58" rx="18" fill="{col}"/>'
    icons["calculator"] = icon("Calculadora", ("#3a3e52", "#14161f"), f"""
  <rect x="104" y="100" width="304" height="86" rx="20" fill="#0a0b12"/>
  <path d="M330 156 h56" stroke="#fff" stroke-width="12" stroke-linecap="round"/>
  {keys}
""")

    # Store — sacola
    icons["store"] = icon("Loja", ("#52e0c6", "#1b7fe0"), """
  <path d="M168 196 h176 a20 20 0 0 1 20 19 l12 150 a26 26 0 0 1 -26 28 h-188 a26 26 0 0 1 -26 -28 l12 -150 a20 20 0 0 1 20 -19z" fill="#fff"/>
  <path d="M200 214 v-24 a56 56 0 0 1 112 0 v24" fill="none" stroke="#fff" stroke-width="22" stroke-linecap="round" transform="translate(0 -8)"/>
  <path d="M212 296 a44 44 0 0 0 88 0" fill="none" stroke="#2a6fe0" stroke-width="20" stroke-linecap="round"/>
""")
    return icons


def main():
    write(os.path.join(OUT_SVG, "obelisk-mark.svg"), mark_svg())
    write(os.path.join(OUT_SVG, "obelisk-mark-color.svg"), mark_gradient_svg())
    write(os.path.join(OUT_SVG, "obelisk-mark-mono-black.svg"),
          mark_svg(mono="#000000").replace("#00000000", "none"))
    write(os.path.join(OUT_SVG, "obelisk-app-icon.svg"), app_icon_svg())
    write(os.path.join(OUT_SVG, "obelisk-wordmark-light.svg"), wordmark_svg(False))
    write(os.path.join(OUT_SVG, "obelisk-wordmark-dark.svg"), wordmark_svg(True))
    write(os.path.join(OUT_WALL, "obelisk-light.svg"), wallpaper(False))
    write(os.path.join(OUT_WALL, "obelisk-dark.svg"), wallpaper(True))
    write(os.path.join(OUT_WALL, "obelisk-login.svg"), login_svg())
    for name, svg in build_icons().items():
        write(os.path.join(OUT_ICONS, f"{name}.svg"), svg)
    print("ok")


if __name__ == "__main__":
    main()
