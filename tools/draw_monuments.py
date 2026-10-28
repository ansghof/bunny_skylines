"""Zeichnet die Denkmäler, die sich die Familie mit Errungenschaften verdient, im selben Stil wie
`draw_art.py`: weich schattiert, ohne dicke Umrisse, Licht von links oben. Alle stehen aufrecht auf
der Wiese wie Bäume; unten in der Mitte ist der Boden.

Jedes Denkmal gibt es in drei Stufen, gleich groß und an derselben Stelle: Stufe 1 ist schlicht,
Stufe 2 geschmückt mit Blumen und Bändern, Stufe 3 prächtig mit Gold, Schein und Funkeln.

Aufruf aus dem Projektordner: `python tools/draw_monuments.py .`, danach `cargo run -p art_export`."""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from draw_art import Svg, dark, f, flower, foliage, glow, light, mix, ring  # noqa: E402

STONE = '#b2ad9f'
GOLD = '#e0b440'
FLOWERS = ['#ffffff', '#f2b8b0', '#f4e7a0', '#b98ad9', '#ffffff']
GRASS = ['#8ab858', '#5e8a34', '#a8cc70']


def shadow(s, cx, cy, rx, ry, opacity=0.4):
    s.add(f'<ellipse cx="{f(cx)}" cy="{f(cy)}" rx="{f(rx)}" ry="{f(ry)}" fill="#141008" opacity="{opacity}" {s.blur(max(2, ry * 0.6))}/>')


def lit(s, box, base, at=(0.3, 0.25), spread=0.9, rim=0.45, shine=0.35):
    """Kugeliges Licht für eine Form in `box`, von links oben."""
    x0, y0, x1, y1 = box
    w, h = x1 - x0, y1 - y0
    return s.radial(x0 + w * at[0], y0 + h * at[1], max(w, h) * spread,
                    [(0, light(base, shine)), (0.5, base), (1, dark(base, rim))])


def stone(s, d, base, box, rough=1.8, specks=True, at=(0.3, 0.25)):
    """Ein Stück Stein: schattiert, mit leicht unregelmäßigem Rand und Sprenkeln."""
    s.add(f'<g {s.rough(rough, 0.15)}><path d="{d}" fill="{lit(s, box, base, at)}"/></g>')
    if specks:
        x0, y0, x1, y1 = box
        n = int((x1 - x0) * (y1 - y0) / 45)
        dots = ''.join(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(s.rng.uniform(0.4, 1.2))}" '
                       f'fill="{s.rng.choice([dark(base, 0.35), light(base, 0.4), dark(base, 0.2)])}" opacity="0.6"/>'
                       for x, y in s.scatter(n, x0, y0, x1, y1))
        s.add(f'<g {s.clip_d(d)}>{dots}</g>')


def block(x0, y0, x1, y1, r=4):
    """Ein Quader mit leicht gerundeten Ecken als Pfad."""
    return (f'M{f(x0 + r)} {f(y0)} H{f(x1 - r)} Q{f(x1)} {f(y0)} {f(x1)} {f(y0 + r)} V{f(y1 - r)} '
            f'Q{f(x1)} {f(y1)} {f(x1 - r)} {f(y1)} H{f(x0 + r)} Q{f(x0)} {f(y1)} {f(x0)} {f(y1 - r)} '
            f'V{f(y0 + r)} Q{f(x0)} {f(y0)} {f(x0 + r)} {f(y0)} Z')


def plinth(s, x0, y0, x1, y1, base=STONE, cap=10):
    """Ein Sockel aus Stein: eine Deckplatte, darunter der Block, die rechte Seite im Schatten."""
    body = block(x0 + 6, y0 + cap - 2, x1 - 6, y1, 3)
    stone(s, body, base, (x0, y0, x1, y1), 1.4)
    s.add(f'<g {s.clip_d(body)}><rect x="{f(x1 - 6 - (x1 - x0) * 0.28)}" y="{f(y0)}" width="{f((x1 - x0) * 0.3)}" '
          f'height="{f(y1 - y0)}" fill="#1e1a12" opacity="0.25" {s.blur(4)}/>'
          f'<rect x="{f(x0)}" y="{f(y0 + cap)}" width="{f(x1 - x0)}" height="5" fill="#1e1a12" opacity="0.3" {s.blur(2)}/></g>')
    stone(s, block(x0, y0, x1, y0 + cap, 3), light(base, 0.08), (x0, y0, x1, y0 + cap + 20), 1.2)


def moss(s, cx, cy, rx, ry):
    s.add(f'<g {s.rough(4, 0.35)}><ellipse cx="{f(cx)}" cy="{f(cy)}" rx="{f(rx)}" ry="{f(ry)}" '
          f'fill="{s.radial(cx - rx * 0.3, cy - ry * 0.4, rx * 1.2, [(0, "#a6c86a"), (0.6, "#6a9a3c"), (1, "#3e6a24")])}"/></g>')


def tufts(s, x0, x1, y, n=14, length=10):
    pts = [(s.rng.uniform(x0, x1), y + s.rng.uniform(-2, 2)) for _ in range(n)]
    s.add(s.strokes(pts, -90, length, 1.4, GRASS, 0.9, 0.5, 0.2))


def sparkle(s, x, y, r, color='#ffffff', opacity=0.95):
    """Ein Glanzpunkt: ein vierzackiger Stern mit weichem Schein."""
    d = (f'M{f(x)} {f(y - r)} Q{f(x + r * 0.15)} {f(y - r * 0.15)} {f(x + r)} {f(y)} '
         f'Q{f(x + r * 0.15)} {f(y + r * 0.15)} {f(x)} {f(y + r)} Q{f(x - r * 0.15)} {f(y + r * 0.15)} {f(x - r)} {f(y)} '
         f'Q{f(x - r * 0.15)} {f(y - r * 0.15)} {f(x)} {f(y - r)} Z')
    s.add(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(r * 0.7)}" fill="{color}" opacity="0.35" {s.blur(r * 0.4)}/>'
          f'<path d="{d}" fill="{color}" opacity="{opacity}"/>')


def ribbon_bow(s, x, y, color, size=1.0):
    """Eine Schleife mit zwei hängenden Enden."""
    k = size
    g = s.linear(x - 12 * k, y, x + 12 * k, y, [(0, light(color, 0.25)), (1, dark(color, 0.3))])
    s.add(f'<path d="M{f(x)} {f(y)} C{f(x - 6 * k)} {f(y - 9 * k)} {f(x - 15 * k)} {f(y - 7 * k)} {f(x - 13 * k)} {f(y + 1 * k)} '
          f'C{f(x - 11 * k)} {f(y + 7 * k)} {f(x - 5 * k)} {f(y + 4 * k)} {f(x)} {f(y)} Z '
          f'M{f(x)} {f(y)} C{f(x + 6 * k)} {f(y - 9 * k)} {f(x + 15 * k)} {f(y - 7 * k)} {f(x + 13 * k)} {f(y + 1 * k)} '
          f'C{f(x + 11 * k)} {f(y + 7 * k)} {f(x + 5 * k)} {f(y + 4 * k)} {f(x)} {f(y)} Z" fill="{g}"/>'
          f'<path d="M{f(x - 1 * k)} {f(y + 1 * k)} L{f(x - 7 * k)} {f(y + 16 * k)} L{f(x - 3 * k)} {f(y + 14 * k)} L{f(x - 1 * k)} {f(y + 18 * k)} Z '
          f'M{f(x + 1 * k)} {f(y + 1 * k)} L{f(x + 8 * k)} {f(y + 15 * k)} L{f(x + 4 * k)} {f(y + 14 * k)} L{f(x + 3 * k)} {f(y + 18 * k)} Z" '
          f'fill="{dark(color, 0.15)}"/>'
          f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(3 * k)}" fill="{dark(color, 0.25)}"/>')


def earth_mound(s, d, box):
    s.add(f'<g {s.rough(3, 0.35)}><path d="{d}" fill="{lit(s, box, "#6a4a2c", (0.35, 0.2), 1.0, 0.45, 0.25)}"/></g>')
    x0, y0, x1, y1 = box
    crumbs = ''.join(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(s.rng.uniform(0.7, 2.2))}" '
                     f'fill="{s.rng.choice(["#a07a50", "#2e1e10", "#7a5a3a", "#b89068"])}" opacity="0.85"/>'
                     for x, y in s.scatter(int((x1 - x0) * (y1 - y0) / 18), x0, y0, x1, y1))
    s.add(f'<g {s.clip_d(d)}>{crumbs}</g>')


def halo(s, cx, cy, rx, ry, color='#fff0a0'):
    """Stufe 3: ein warmer Schein hinter dem Denkmal. Er verläuft weich und bleibt im Bild, damit
    man keine Kante sieht."""
    rx = min(rx, cx - 1, s.w - cx - 1)
    ry = min(ry, cy - 1, s.h - cy - 1)
    name = s.id('h')
    s.defs.append(f'<radialGradient id="{name}"><stop offset="0" stop-color="{color}" stop-opacity="0.7"/>'
                  f'<stop offset="0.55" stop-color="{color}" stop-opacity="0.35"/>'
                  f'<stop offset="1" stop-color="{color}" stop-opacity="0"/></radialGradient>')
    s.add(f'<ellipse cx="{f(cx)}" cy="{f(cy)}" rx="{f(rx)}" ry="{f(ry)}" fill="url(#{name})"/>')


def glitter(s, points, color='#fffbe0'):
    for x, y, r in points:
        sparkle(s, x, y, r, color)


def flower_row(s, x0, x1, y, n):
    """Blumen am Fuß, etwas unregelmäßig verteilt."""
    for i in range(n):
        x = x0 + (x1 - x0) * (i + 0.5) / n + s.rng.uniform(-3, 3)
        s.add(flower(s, x, y + s.rng.uniform(-2, 2), s.rng.uniform(3.5, 5), s.rng.choice(FLOWERS)))


def garland(s, x0, x1, y, sag, n):
    """Eine Blumengirlande, die zwischen zwei Punkten durchhängt."""
    s.add(f'<path d="M{f(x0)} {f(y)} Q{f((x0 + x1) / 2)} {f(y + sag * 2)} {f(x1)} {f(y)}" stroke="#3e7a2a" '
          f'stroke-width="5" fill="none" stroke-linecap="round"/>'
          f'<path d="M{f(x0)} {f(y - 1)} Q{f((x0 + x1) / 2)} {f(y + sag * 2 - 1)} {f(x1)} {f(y - 1)}" stroke="#6aa848" '
          f'stroke-width="2" fill="none" stroke-linecap="round"/>')
    for i in range(n + 1):
        t = i / n
        x = x0 + (x1 - x0) * t
        yy = y + sag * 4 * t * (1 - t)
        s.add(flower(s, x, yy, 4.5, FLOWERS[i % len(FLOWERS)] if i % 2 else '#f2b8b0'))


def bunting(s, x0, y0, x1, y1, sag, n, colors=('#d8503a', '#f0c040', '#5a8ad0', '#5a9a48')):
    """Eine Wimpelkette: eine Schnur mit kleinen Dreiecken."""
    mx, my = (x0 + x1) / 2, (y0 + y1) / 2 + sag * 2
    s.add(f'<path d="M{f(x0)} {f(y0)} Q{f(mx)} {f(my)} {f(x1)} {f(y1)}" stroke="#6e5a3a" stroke-width="1.2" fill="none"/>')
    for i in range(n):
        t = (i + 0.5) / n
        x = (1 - t) ** 2 * x0 + 2 * (1 - t) * t * mx + t ** 2 * x1
        y = (1 - t) ** 2 * y0 + 2 * (1 - t) * t * my + t ** 2 * y1
        w = abs(x1 - x0) / n * 0.42
        c = colors[i % len(colors)]
        s.add(f'<path d="M{f(x - w)} {f(y)} L{f(x + w)} {f(y)} L{f(x)} {f(y + w * 2.2)} Z" fill="{c}"/>'
              f'<path d="M{f(x - w)} {f(y)} L{f(x)} {f(y)} L{f(x)} {f(y + w * 2.2)} Z" fill="{light(c, 0.25)}" opacity="0.6"/>')


def laurel(s, cx, cy, r, a0, a1, n, color=GOLD, leaf=6.0):
    """Ein Lorbeerzweig aus goldenen Blättern auf einem Bogen von `a0` bis `a1` (Grad)."""
    for i in range(n):
        a = math.radians(a0 + (a1 - a0) * i / (n - 1))
        x, y = cx + math.cos(a) * r, cy + math.sin(a) * r
        for side in (-1, 1):
            tilt = math.degrees(a) + 90 + side * 35
            lx = x + math.cos(math.radians(tilt)) * leaf * 0.6
            ly = y + math.sin(math.radians(tilt)) * leaf * 0.6
            s.add(f'<ellipse cx="{f(lx)}" cy="{f(ly)}" rx="{f(leaf)}" ry="{f(leaf * 0.42)}" '
                  f'transform="rotate({f(tilt)} {f(lx)} {f(ly)})" '
                  f'fill="{s.radial(lx - 2, ly - 2, leaf * 1.4, [(0, light(color, 0.45)), (1, dark(color, 0.3))])}"/>')


def medal(s, x, y, r, color=GOLD):
    """Eine Rosette mit zwei Bändern: ein Ehrenpreis."""
    for dx, c in ((-0.5, '#d8503a'), (0.5, '#5a8ad0')):
        s.add(f'<path d="M{f(x + dx * r * 0.4)} {f(y)} L{f(x + dx * r * 1.4)} {f(y + r * 2.2)} L{f(x + dx * r * 0.6)} {f(y + r * 1.8)} '
              f'L{f(x)} {f(y + r * 2.3)} Z" fill="{c}"/>')
    petals = ''.join(f'<circle cx="{f(x + math.cos(i * math.tau / 12) * r * 0.9)}" cy="{f(y + math.sin(i * math.tau / 12) * r * 0.9)}" '
                     f'r="{f(r * 0.32)}"/>' for i in range(12))
    s.add(f'<g fill="{dark(color, 0.1)}">{petals}</g>'
          f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(r * 0.85)}" fill="{s.radial(x - r * 0.3, y - r * 0.3, r, [(0, light(color, 0.5)), (1, dark(color, 0.3))])}"/>'
          f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(r * 0.55)}" fill="none" stroke="{light(color, 0.4)}" stroke-width="1"/>')


def name(monument, tier):
    return f'einrichtung/denkmal_{monument}_{tier}.svg'


# ---------------------------------------------------------------------------------------------

def statue(tier):
    s = Svg(240, 340, 'Kaninchenstatue: ein steinernes Kaninchen macht Männchen auf einem hohen Sockel, mit Moos und Gras am Fuß', 201)
    shadow(s, 124, 330, 104, 9)
    if tier >= 3:
        halo(s, 122, 140, 110, 130)
    plinth(s, 46, 236, 194, 330)
    # Eine Tafel vorn auf dem Sockel, mit eingeritzter Pfote.
    s.add(f'<path d="{block(84, 266, 156, 306, 3)}" fill="{s.linear(0, 266, 0, 306, [(0, "#8e897c"), (1, "#a8a396")])}" opacity="0.8"/>')
    for dx, dy, r in ((-10, -6, 4), (-3, -11, 4), (5, -11, 4), (12, -6, 4)):
        s.add(f'<ellipse cx="{120 + dx}" cy="{290 + dy}" rx="{r * 0.8}" ry="{r}" fill="#6e6a5e" opacity="0.8"/>')
    s.add('<ellipse cx="120" cy="294" rx="10" ry="8" fill="#6e6a5e" opacity="0.8"/>')
    if tier >= 3:
        # Ein goldener Rahmen um die Tafel.
        s.add(f'<path d="{block(84, 266, 156, 306, 3)}" fill="none" stroke="{GOLD}" stroke-width="3"/>')
    if tier == 1:
        moss(s, 64, 322, 16, 7)
        moss(s, 182, 326, 12, 5)
        moss(s, 96, 240, 14, 4)

    # Das Kaninchen: eine einzige Form aus Teilen, die alle dasselbe Licht bekommen; ganz oben aus Gold.
    box = (58, 20, 186, 240)
    body = '#d8a838' if tier >= 3 else STONE
    g = lit(s, box, body, (0.3, 0.3), 0.8, 0.5, 0.5 if tier >= 3 else 0.35)
    parts = [
        '<ellipse cx="122" cy="206" rx="54" ry="34"/>',               # Keulen
        '<ellipse cx="124" cy="160" rx="38" ry="56"/>',               # Rumpf, aufgerichtet
        '<ellipse cx="132" cy="96" rx="31" ry="27"/>',                # Kopf
        '<ellipse cx="118" cy="44" rx="11" ry="38" transform="rotate(-12 118 44)"/>',   # Ohr hinten
        '<ellipse cx="142" cy="42" rx="12" ry="40" transform="rotate(10 142 42)"/>',    # Ohr vorn
        '<ellipse cx="78" cy="214" rx="15" ry="14"/>',                # Schwanz
        '<ellipse cx="96" cy="232" rx="30" ry="9"/>',                 # Hinterpfoten
        '<ellipse cx="158" cy="232" rx="26" ry="8"/>',
    ]
    s.add(f'<g {s.rough(1.2, 0.2)} fill="{g}">' + ''.join(parts) + '</g>')
    # Vorderpfoten vor der Brust, die Innenseite der Ohren und die Wangen etwas tiefer gemeißelt.
    s.add(f'<g fill="{lit(s, (130, 130, 170, 170), body, (0.3, 0.2), 1.0, 0.4)}">'
          '<ellipse cx="146" cy="146" rx="11" ry="16" transform="rotate(-20 146 146)"/>'
          '<ellipse cx="160" cy="150" rx="10" ry="15" transform="rotate(-25 160 150)"/></g>')
    inner = '#a07418' if tier >= 3 else '#8a8578'
    s.add(f'<ellipse cx="143" cy="46" rx="5" ry="30" transform="rotate(10 143 46)" fill="{inner}" opacity="0.6"/>'
          f'<ellipse cx="119" cy="48" rx="4" ry="26" transform="rotate(-12 119 48)" fill="{inner}" opacity="0.45"/>')
    # Schatten unter Kopf und Brust, Licht an der linken Kante.
    s.add(f'<ellipse cx="128" cy="124" rx="28" ry="8" fill="#2a261c" opacity="0.3" {s.blur(4)}/>'
          f'<ellipse cx="122" cy="186" rx="40" ry="8" fill="#2a261c" opacity="0.25" {s.blur(5)}/>'
          f'<path d="M92 120 C84 150 84 180 74 200" stroke="#efeadc" stroke-width="4" fill="none" opacity="0.5" {s.blur(2)}/>')
    # Gesicht: Auge, Nase, Schnurrhaarlöcher.
    s.add('<ellipse cx="146" cy="90" rx="5" ry="6" fill="#5a564c"/><ellipse cx="144.5" cy="88" rx="1.6" ry="2" fill="#d8d4c8"/>'
          '<path d="M160 104 q3 2 1 5" stroke="#5a564c" stroke-width="2" fill="none"/>'
          '<circle cx="154" cy="110" r="0.9" fill="#6e6a5e"/><circle cx="150" cy="112" r="0.9" fill="#6e6a5e"/>')
    if tier == 1:
        # Ein feiner Riss und Flechten: Der Stein ist alt und noch nicht gepflegt.
        s.add('<path d="M102 176 l6 10 l-3 9 l7 12" stroke="#6e6a5e" stroke-width="1.2" fill="none" opacity="0.7"/>')
        for x, y in ((100, 214), (150, 214), (112, 140), (128, 74)):
            s.add(f'<circle cx="{x}" cy="{y}" r="{f(s.rng.uniform(3, 5))}" fill="#c8c48a" opacity="0.45" {s.blur(1)}/>')
    tufts(s, 40, 200, 330, 22)
    if tier >= 2:
        garland(s, 50, 190, 244, 10, 10)
        flower_row(s, 40, 200, 326, 9)
    if tier >= 3:
        # Ein Lorbeerkranz zwischen den Ohren und Funkeln auf dem Gold.
        laurel(s, 130, 84, 34, 200, 340, 7, '#f0c850', 5.5)
        glitter(s, [(96, 150, 7), (150, 60, 6), (168, 196, 5), (110, 100, 4), (186, 120, 5), (60, 90, 4)])
    s.save(name('statue', tier))


def shovel(tier):
    s = Svg(128, 184, 'Schaufelmonument: eine große Schaufel steckt in einem Erdhaufen, mit roter Ehrenschleife am Stiel', 202)
    shadow(s, 66, 178, 58, 6)
    if tier >= 3:
        halo(s, 66, 90, 60, 80)
    # Der Stiel ragt schräg aus dem Haufen; die Klinge steckt halb in der Erde.
    handle = 'M58 112 L72 24 L80 25 L67 113 Z'
    s.add(f'<path d="{handle}" fill="{s.linear(58, 0, 80, 0, [(0, "#d8b07a"), (0.45, "#b08350"), (1, "#6e4c2c")])}"/>')
    s.add(f'<g {s.clip_d(handle)}>' + s.strokes(s.scatter(24, 58, 24, 80, 112), 98, 10, 0.8, ['#7a5634', '#e8c890'], 0.6, 0.08) + '</g>')
    # Griff oben: ein Querholz.
    s.add(f'<path d="M60 22 C62 14 88 14 92 22 C90 28 62 28 60 22 Z" fill="{s.linear(0, 14, 0, 28, [(0, "#e0bc88"), (1, "#7a5634")])}"/>'
          '<path d="M66 22 L86 22" stroke="#5a3c22" stroke-width="1" opacity="0.5"/>')
    # Die Klinge, Metall mit Glanzkante und etwas Rost.
    blade = 'M40 150 C38 128 46 110 64 108 C80 108 90 124 88 148 L82 156 L46 156 Z'
    metal = GOLD if tier >= 3 else '#9aa2a6'
    s.add(f'<path d="{blade}" fill="{lit(s, (38, 104, 92, 156), metal, (0.25, 0.2), 0.9, 0.5, 0.5)}"/>')
    rust = f'<g {s.clip_d(blade)}><ellipse cx="80" cy="140" rx="10" ry="16" fill="#8a5a2c" opacity="0.35" {s.blur(4)}/></g>' if tier == 1 else ''
    s.add(f'<path d="M46 132 C48 118 56 112 64 112" stroke="#f4f8fa" stroke-width="3" fill="none" opacity="0.7" {s.blur(1)}/>'
          f'{rust}<path d="M60 110 L64 108 L68 110 L66 124 L62 124 Z" fill="{dark(metal, 0.3)}"/>')
    # Der Erdhaufen davor verdeckt die Spitze der Klinge.
    mound = 'M6 178 C10 156 28 140 52 142 C60 146 70 146 80 142 C102 140 120 158 124 178 Z'
    earth_mound(s, mound, (6, 140, 124, 178))
    s.add(f'<path d="M40 148 C50 152 76 152 88 146" stroke="#2e1e10" stroke-width="3" fill="none" opacity="0.45" {s.blur(1.5)}/>')
    tufts(s, 8, 122, 177, 14, 8)
    if tier >= 2:
        ribbon_bow(s, 70, 64, '#d0402e', 1.0)
        flower_row(s, 14, 116, 170, 6)
    if tier >= 3:
        medal(s, 74, 84, 8)
        glitter(s, [(52, 128, 6), (84, 30, 5), (30, 100, 4), (100, 70, 4)])
    s.save(name('schaufel', tier))


def fox_stone(tier):
    s = Svg(260, 210, 'Fuchsschreck: ein mächtiger Findling mit tiefen Kratzspuren, Moos obenauf und ein Büschel rotes Fuchsfell', 203)
    shadow(s, 134, 198, 120, 10, 0.45)
    if tier >= 3:
        halo(s, 132, 110, 130, 90)
    d = ('M18 198 C10 160 22 104 58 74 C90 46 140 34 182 50 C222 66 248 112 246 158 C246 178 240 192 232 198 Z')
    stone(s, d, '#a39e90', (10, 34, 248, 198), 2.6, True, (0.28, 0.18))
    # Kanten und Flächen: eine hellere Oberseite, dunklere rechte Flanke.
    s.add(f'<g {s.clip_d(d)}><path d="M40 96 C80 56 150 44 200 66" stroke="#e4e0d2" stroke-width="10" fill="none" opacity="0.35" {s.blur(5)}/>'
          f'<ellipse cx="226" cy="150" rx="40" ry="70" fill="#2a261c" opacity="0.3" {s.blur(12)}/>'
          f'<ellipse cx="130" cy="200" rx="120" ry="20" fill="#2a261c" opacity="0.35" {s.blur(8)}/></g>')
    # Drei Kratzspuren schräg über den Stein: tiefe Rillen mit heller Kante.
    # Ganz oben sind die Rillen mit Gold ausgelegt.
    groove, edge = (GOLD, '#fff4c0') if tier >= 3 else ('#4a463c', '#e8e4d6')
    for i in range(3):
        x = 104 + i * 22
        s.add(f'<path d="M{x} 92 C{x + 8} 112 {x + 14} 134 {x + 16} 160" stroke="{groove}" stroke-width="5" fill="none" stroke-linecap="round"/>'
              f'<path d="M{x - 2} 93 C{x + 6} 113 {x + 12} 135 {x + 14} 160" stroke="{edge}" stroke-width="1.5" fill="none" opacity="0.7" stroke-linecap="round"/>')
    # Ein Riss und Flechten.
    s.add('<path d="M60 120 l10 14 l-4 12 l9 16" stroke="#5e5a4e" stroke-width="1.4" fill="none" opacity="0.7"/>')
    for x, y in ((70, 150), (190, 110), (160, 170), (52, 100)):
        s.add(f'<circle cx="{x}" cy="{y}" r="{f(s.rng.uniform(4, 7))}" fill="#c8c48a" opacity="0.4" {s.blur(1.2)}/>')
    moss(s, 130, 50, 34, 9)
    moss(s, 80, 70, 18, 6)
    # Ein Büschel Fuchsfell, das an der rauen Kante hängen geblieben ist.
    s.add(s.strokes([(202 + s.rng.uniform(-4, 4), 82 + s.rng.uniform(-3, 3)) for _ in range(16)], 60, 12, 1.6,
                    ['#d8742c', '#f0a050', '#a84e1c', '#ffffff'], 0.9, 0.6, 0.3))
    # Kleine Steine und Gras am Fuß.
    for x, y, r in ((26, 196, 9), (238, 196, 7), (204, 200, 5)):
        s.add(f'<ellipse cx="{x}" cy="{y}" rx="{r}" ry="{r * 0.7}" fill="{s.radial(x - 3, y - 3, r * 1.3, [(0, "#d8d4c8"), (1, "#6e6a5e")])}"/>')
    tufts(s, 14, 246, 198, 30, 11)
    if tier >= 2:
        # Ein Laubkranz auf dem Moos und Blumen rundherum.
        foliage(s, [(104 + i * 14, 46 - math.sin(i / 4 * math.pi) * 6, 8) for i in range(5)], '#4a8a34', 4, box=(90, 32, 172, 56))
        for i, x in enumerate((112, 130, 148)):
            s.add(flower(s, x, 42 - (i == 1) * 6, 5, FLOWERS[i + 1]))
        flower_row(s, 30, 230, 196, 12)
    if tier >= 3:
        glitter(s, [(126, 110, 7), (170, 140, 6), (104, 150, 5), (60, 90, 5), (210, 120, 4)])
    s.save(name('fuchsschreck', tier))


def feather_mast(tier):
    s = Svg(104, 260, 'Federmast: ein Holzpfahl mit einem Fächer gestreifter Bussardfedern und bunten Bändern, unten von Steinen gehalten', 204)
    shadow(s, 54, 252, 40, 6)
    if tier >= 3:
        halo(s, 53, 70, 46, 60)
    pole = 'M47 250 L49 36 L57 36 L59 250 Z'
    s.add(f'<path d="{pole}" fill="{s.linear(47, 0, 59, 0, [(0, "#c49a64"), (0.4, "#8a643c"), (1, "#4a321c")])}"/>')
    s.add(f'<g {s.clip_d(pole)}>' + s.strokes(s.scatter(40, 47, 36, 59, 250), 90, 12, 0.8, ['#3e2814', '#d8b07a'], 0.5, 0.05) + '</g>')
    if tier >= 3:
        # Goldene Ringe um den Pfahl.
        for y in (110, 150, 190):
            s.add(f'<path d="M47 {y} L59 {y - 2} L59 {y + 4} L47 {y + 6} Z" fill="{s.linear(47, 0, 59, 0, [(0, "#fff0b0"), (0.5, GOLD), (1, "#8a6414")])}"/>')
    s.add(f'<ellipse cx="53" cy="34" rx="7" ry="4" fill="{s.radial(51, 33, 7, [(0, "#e8c890"), (1, "#8a643c")])}"/>')
    # Die Federn: ein Fächer, unten zusammengebunden, mit hellen und dunklen Querbändern. Erst drei, dann sechs.
    fan = (-30, 6, 42) if tier == 1 else (-48, -30, -12, 6, 24, 42)
    for i, a in enumerate(fan):
        ang = math.radians(a - 90)
        L = 58 - abs(a) * 0.25
        bx, by = 53, 70
        tx, ty = bx + math.cos(ang) * L, by + math.sin(ang) * L
        nx, ny = -math.sin(ang) * 8, math.cos(ang) * 8
        d = (f'M{f(bx)} {f(by)} Q{f(bx + (tx - bx) * 0.5 + nx)} {f(by + (ty - by) * 0.5 + ny)} {f(tx)} {f(ty)} '
             f'Q{f(bx + (tx - bx) * 0.5 - nx)} {f(by + (ty - by) * 0.5 - ny)} {f(bx)} {f(by)} Z')
        base = ['#8a5a32', '#a06c3c', '#7a4e2a'][i % 3]
        s.add(f'<path d="{d}" fill="{s.linear(bx, by, tx, ty, [(0, "#f0e4cc"), (0.25, base), (1, dark(base, 0.3))])}"/>')
        bands = ''.join(f'<path d="M{f(bx + (tx - bx) * k - nx)} {f(by + (ty - by) * k - ny)} L{f(bx + (tx - bx) * k + nx)} {f(by + (ty - by) * k + ny)}" '
                        f'stroke="{"#3a2210" if j % 2 else "#f4ead4"}" stroke-width="3" opacity="0.55"/>'
                        for j, k in enumerate((0.35, 0.5, 0.65, 0.8)))
        s.add(f'<g {s.clip_d(d)}>{bands}</g>')
        s.add(f'<path d="M{f(bx)} {f(by)} L{f(tx)} {f(ty)}" stroke="#f4ead4" stroke-width="0.8" opacity="0.7"/>')
    # Gebunden mit Bast und Bändern, die im Wind wehen.
    s.add('<path d="M45 70 C50 66 58 66 62 70 L60 78 C55 75 50 75 46 78 Z" fill="#d8c08a"/>')
    if tier >= 2:
        for color, dx, sway in (('#d0402e', -4, -14), ('#4a7ad0', 2, 10), ('#f0c040', 6, 18)):
            s.add(f'<path d="M{53 + dx} 76 C{48 + dx + sway} 96 {58 + dx - sway} 112 {50 + dx + sway} 132" stroke="{color}" '
                  f'stroke-width="3.5" fill="none" stroke-linecap="round"/>')
    # Steine halten den Pfahl.
    for x, y, r in ((36, 246, 11), (70, 246, 12), (53, 240, 10), (46, 252, 9), (64, 252, 9)):
        s.add(f'<ellipse cx="{x}" cy="{y}" rx="{r}" ry="{r * 0.72}" fill="{s.radial(x - 3, y - 3, r * 1.3, [(0, "#d8d4c8"), (0.6, "#a8a496"), (1, "#6a665a")])}"/>')
    tufts(s, 16, 90, 254, 12, 9)
    if tier >= 2:
        flower_row(s, 12, 94, 250, 5)
    if tier >= 3:
        # Eine goldene Kugel auf der Spitze.
        s.add(f'<circle cx="53" cy="28" r="7" fill="{s.radial(51, 26, 8, [(0, "#fff4c0"), (0.5, GOLD), (1, "#8a6414")])}"/>')
        glitter(s, [(50, 26, 5), (30, 44, 4), (78, 50, 5), (60, 160, 4)])
    s.save(name('federmast', tier))


def snow_rabbit(tier):
    s = Svg(132, 168, 'Winterkaninchen: ein Kaninchen aus Schnee und Eis auf einem flachen Stein, mit Eiszapfen und Glitzern', 205)
    shadow(s, 68, 160, 56, 6)
    if tier >= 3:
        halo(s, 68, 80, 66, 70, '#d8ecff')
    stone(s, block(16, 132, 116, 162, 8), '#9a9688', (16, 132, 116, 162), 1.6)
    s.add('<path d="M22 136 C40 130 92 130 110 136" stroke="#f4f8ff" stroke-width="6" fill="none" stroke-linecap="round"/>')
    for x in ((30, 102) if tier == 1 else (30, 48, 86, 102)):
        L = s.rng.uniform(6, 12)
        s.add(f'<path d="M{x - 3} 138 L{x} {138 + L} L{x + 3} 138 Z" fill="{s.linear(0, 138, 0, 138 + L, [(0, "#e8f4ff"), (1, "#9ac8f0", 0.6)])}"/>')
    # Ein hockendes Kaninchen aus Schnee: kugelig, bläulich im Schatten.
    box = (24, 24, 112, 136)
    g = s.radial(46, 60, 96, [(0, '#ffffff'), (0.45, '#eef6ff'), (0.8, '#c4dcf2'), (1, '#8ab0d8')])
    parts = [
        '<ellipse cx="70" cy="108" rx="40" ry="28"/>',
        '<ellipse cx="80" cy="74" rx="24" ry="22"/>',
        '<ellipse cx="70" cy="34" rx="8" ry="24" transform="rotate(-14 70 34)"/>',
        '<ellipse cx="88" cy="34" rx="8" ry="25" transform="rotate(12 88 34)"/>',
        '<circle cx="32" cy="112" r="11"/>',
    ]
    s.add(f'<g {s.rough(1.6, 0.3)} fill="{g}">' + ''.join(parts) + '</g>')
    s.add(f'<ellipse cx="72" cy="128" rx="36" ry="6" fill="#7aa0c8" opacity="0.35" {s.blur(3)}/>'
          '<ellipse cx="89" cy="36" rx="3.5" ry="18" transform="rotate(12 89 36)" fill="#b8d4f0" opacity="0.7"/>'
          '<ellipse cx="71" cy="36" rx="3" ry="16" transform="rotate(-14 71 36)" fill="#b8d4f0" opacity="0.5"/>')
    # Auge aus einem Kieselstein, Nase aus einer Beere.
    s.add('<circle cx="90" cy="70" r="3.4" fill="#2a2a30"/><circle cx="89" cy="69" r="1" fill="#ffffff"/>'
          '<circle cx="102" cy="80" r="2.4" fill="#c8302a"/>')
    if tier >= 2:
        # Ein gestrickter roter Schal mit weißen Streifen und einem flatternden Ende.
        scarf = 'M58 90 C66 98 92 98 102 88 L104 96 C94 106 64 106 56 98 Z'
        s.add(f'<path d="{scarf}" fill="{s.linear(56, 0, 104, 0, [(0, "#e05040"), (1, "#a82a20")])}"/>'
              '<path d="M66 96 L70 112 L80 110 L76 98 Z" fill="#c8382c"/>'
              '<path d="M68 101 L77 100 M69 106 L78 105" stroke="#ffffff" stroke-width="1.6" opacity="0.8"/>')
        for x in (64, 74, 84, 94):
            s.add(f'<path d="M{x} 95 L{x + 2} 101" stroke="#ffffff" stroke-width="1.4" opacity="0.6"/>')
    if tier >= 3:
        # Eine Krone aus Eiskristallen zwischen den Ohren.
        for x, h in ((70, 12), (79, 18), (88, 12)):
            s.add(f'<path d="M{x - 4} 58 L{x} {58 - h} L{x + 4} 58 Z" fill="{s.linear(0, 58 - h, 0, 58, [(0, "#ffffff"), (1, "#7ab4f0")])}"/>')
        s.add('<path d="M64 58 L94 58 L92 62 L66 62 Z" fill="#a8d0f8"/>')
    stars = ((44, 92, 6), (96, 56, 5), (60, 118, 4), (108, 100, 3.5), (30, 60, 4))
    if tier >= 3:
        stars += ((20, 30, 5), (114, 30, 4), (14, 90, 4), (120, 130, 4))
    glitter(s, stars[:2] if tier == 1 else stars, '#ffffff')
    s.save(name('winterkaninchen', tier))


def frond(s, x, y, angle, length, base):
    """Ein Möhrenblatt: ein gebogener Stiel mit fein gefiederten Blättchen links und rechts."""
    a = math.radians(angle - 90)
    bend = math.radians(angle * 0.4)
    out = ''
    points = []
    for i in range(9):
        t = i / 8
        ang = a + bend * t
        points.append((x + math.cos(ang) * length * t, y + math.sin(ang) * length * t))
    path = 'M' + ' L'.join(f'{f(px)} {f(py)}' for px, py in points)
    out += f'<path d="{path}" stroke="{dark(base, 0.25)}" stroke-width="3.2" fill="none" stroke-linecap="round"/>'
    for i in range(2, 9):
        px, py = points[i]
        size = 16 * (1.15 - i / 10)
        for side in (-1, 1):
            la = a + side * math.radians(55)
            lx, ly = px + math.cos(la) * size * 0.6, py + math.sin(la) * size * 0.6
            c = light(base, 0.25) if (side < 0) else dark(base, 0.12)
            out += (f'<ellipse cx="{f(lx)}" cy="{f(ly)}" rx="{f(size * 0.62)}" ry="{f(size * 0.3)}" '
                    f'transform="rotate({f(math.degrees(la))} {f(lx)} {f(ly)})" fill="{c}"/>')
    tip = points[-1]
    out += f'<ellipse cx="{f(tip[0])}" cy="{f(tip[1])}" rx="4.5" ry="3" transform="rotate({f(angle)} {f(tip[0])} {f(tip[1])})" fill="{light(base, 0.2)}"/>'
    return f'<g {s.rough(1.6, 0.5)}>{out}</g>'


def giant_carrot(tier):
    s = Svg(220, 320, 'Riesenmöhre: eine aus Holz geschnitzte, bemalte Möhre steckt mit der Spitze in der Erde, oben ein Schopf gefiederter Blätter', 206)
    shadow(s, 112, 310, 92, 8)
    if tier >= 3:
        halo(s, 110, 150, 110, 140)
    # Das Laub zuerst: Es wächst hinter der Schnittkante aus der Möhre.
    for angle, length, base in ((-62, 58, '#5aa83a'), (60, 60, '#5aa83a'), (-48, 76, '#3e8a2c'), (46, 78, '#3e8a2c'),
                                (-34, 92, '#4e9a32'), (32, 94, '#4e9a32'), (-18, 104, '#3e8a2c'), (16, 102, '#3e8a2c'),
                                (-4, 110, '#5aa83a'), (8, 96, '#4e9a32')):
        s.add(frond(s, 111 + angle * 0.3, 112, angle, length, base))
    # Die Möhre steht kopfüber: dick oben, spitz unten in der Erde.
    body = 'M60 118 C60 98 160 98 162 118 C164 170 130 250 114 296 C110 300 106 300 104 296 C88 250 58 170 60 118 Z'
    s.add(f'<path d="{body}" fill="{lit(s, (58, 98, 164, 300), "#e8782a", (0.3, 0.2), 0.9, 0.45, 0.3)}"/>')
    # Kurze, unregelmäßige Querrillen wie bei einer echten Möhre.
    grooves = ''
    for y in range(128, 286, 11):
        half = 50 * (1 - (y - 118) / 190)
        x = 111 + s.rng.uniform(-half * 0.6, half * 0.4)
        w = s.rng.uniform(half * 0.3, half * 0.7)
        grooves += (f'<path d="M{f(x)} {y} q{f(w / 2)} {f(2.5)} {f(w)} 0" stroke="#a8461c" stroke-width="1.8" fill="none" opacity="0.5"/>'
                    f'<path d="M{f(x)} {y + 1.5} q{f(w / 2)} {f(2.5)} {f(w)} 0" stroke="#ffb27a" stroke-width="1" fill="none" opacity="0.5"/>')
    s.add(f'<g {s.clip_d(body)}>{grooves}'
          f'<ellipse cx="150" cy="180" rx="16" ry="90" fill="#7a2e10" opacity="0.3" {s.blur(8)}/>'
          f'<path d="M78 120 C76 160 90 210 104 260" stroke="#ffc08a" stroke-width="6" fill="none" opacity="0.45" {s.blur(3)}/></g>')
    # Die Schnittfläche oben, mit dem grünen Ansatz des Laubs in der Mitte.
    s.add(f'<ellipse cx="111" cy="112" rx="51" ry="12" fill="{s.radial(100, 108, 52, [(0, "#ffb070"), (0.6, "#f09040"), (1, "#c45a1c")])}"/>'
          '<ellipse cx="111" cy="112" rx="24" ry="5.5" fill="#f8c890" opacity="0.6"/>'
          '<ellipse cx="111" cy="111" rx="10" ry="3" fill="#3e7a24"/>')
    # Erde um die Spitze.
    mound = 'M40 312 C48 292 78 282 108 282 C140 282 170 292 178 312 Z'
    earth_mound(s, mound, (40, 280, 178, 312))
    tufts(s, 30, 190, 312, 22, 10)
    if tier >= 2:
        ribbon_bow(s, 64, 138, '#f0c040', 1.1)
        flower_row(s, 34, 186, 306, 9)
    if tier >= 3:
        medal(s, 132, 156, 13)
        glitter(s, [(84, 190, 7), (150, 120, 5), (40, 60, 5), (184, 70, 5), (120, 240, 4)])
    s.save(name('riesenmoehre', tier))


def ancestor_stone(tier):
    s = Svg(128, 168, 'Ahnenstein: ein aufrechter Stein mit eingeritzten Pfotenabdrücken, einer Reihe pro Generation, Moos und Blumen am Fuß', 207)
    shadow(s, 66, 162, 52, 6)
    if tier >= 3:
        halo(s, 66, 80, 64, 76)
    d = 'M26 162 C22 120 22 62 32 36 C42 14 86 12 98 34 C108 58 108 120 104 162 Z'
    stone(s, d, '#a8a496', (22, 12, 108, 162), 2.0)
    s.add(f'<g {s.clip_d(d)}><ellipse cx="102" cy="100" rx="16" ry="70" fill="#2a261c" opacity="0.3" {s.blur(8)}/>'
          f'<path d="M34 50 C40 26 70 18 88 30" stroke="#ece8da" stroke-width="6" fill="none" opacity="0.4" {s.blur(3)}/></g>')

    # Ganz oben sind die Pfoten mit Gold ausgelegt.
    ink = '#c8962c' if tier >= 3 else '#5e5a4e'

    def paw(x, y, r):
        out = f'<ellipse cx="{f(x)}" cy="{f(y + r * 0.4)}" rx="{f(r)}" ry="{f(r * 0.8)}" fill="{ink}"/>'
        for dx, dy in ((-1.0, -0.9), (-0.35, -1.35), (0.35, -1.35), (1.0, -0.9)):
            out += f'<ellipse cx="{f(x + dx * r)}" cy="{f(y + dy * r)}" rx="{f(r * 0.34)}" ry="{f(r * 0.42)}" fill="{ink}"/>'
        return f'<g opacity="0.9">{out}</g><g opacity="0.45" transform="translate(-0.8 -0.8)">{out.replace(ink, "#fff4d0" if tier >= 3 else "#e8e4d6")}</g>'
    # Ganz oben die Gründer, darunter immer mehr Nachkommen; erst drei Generationen, dann fünf.
    rows = [(1, 46), (2, 70), (3, 94), (4, 118), (5, 140)][:3 if tier == 1 else 5]
    for n, y in rows:
        r = 4.2 - n * 0.35
        for i in range(n):
            x = 65 + (i - (n - 1) / 2) * (r * 4.2)
            s.add(paw(x, y, r))
    moss(s, 36, 156, 14, 6)
    if tier == 1:
        moss(s, 96, 52, 8, 5)
    tufts(s, 18, 112, 162, 14, 9)
    if tier >= 2:
        flower_row(s, 16, 112, 158, 6)
        # Ein Blütenkranz oben auf dem Stein.
        garland(s, 34, 96, 30, -6, 6)
    if tier >= 3:
        glitter(s, [(48, 60, 5), (88, 90, 4), (40, 120, 4), (96, 136, 4)])
    s.save(name('ahnenstein', tier))


def obelisk(tier):
    s = Svg(200, 360, 'Kristallobelisk: ein hoher, dunkler Steinpfeiler aus der Tiefe, mit leuchtenden Kristallen am Fuß und einem eingelassenen Kristall', 208)
    shadow(s, 104, 350, 86, 8)
    s.add(f'<ellipse cx="100" cy="320" rx="80" ry="30" fill="#b48cf0" opacity="{0.15 + 0.1 * tier}" {s.blur(16)}/>')
    if tier >= 3:
        halo(s, 100, 120, 90, 120, '#e8d8ff')
    # Der Pfeiler: links im Licht, rechts im Schatten, oben eine kleine Pyramide.
    left = 'M64 330 L78 70 L100 46 L100 330 Z'
    right = 'M100 330 L100 46 L122 70 L136 330 Z'
    s.add(f'<path d="{left}" fill="{s.linear(64, 0, 100, 0, [(0, "#7a7488"), (1, "#5a5466")])}"/>')
    s.add(f'<path d="{right}" fill="{s.linear(100, 0, 136, 0, [(0, "#3e384a"), (1, "#26222e")])}"/>')
    s.add('<path d="M78 70 L100 46 L100 70 Z" fill="#8e88a0"/><path d="M100 46 L122 70 L100 70 Z" fill="#4a4456"/>'
          '<path d="M78 70 L122 70" stroke="#2a2632" stroke-width="1.2" opacity="0.6"/>')
    tex = ''.join(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(s.rng.uniform(0.4, 1.2))}" fill="{s.rng.choice(["#9a94ac", "#2a2632", "#b0a8c4"])}" opacity="0.6"/>'
                  for x, y in s.scatter(260, 64, 46, 136, 330))
    s.add(f'<g {s.clip_d("M64 330 L78 70 L100 46 L122 70 L136 330 Z")}>{tex}'
          '<path d="M86 140 l6 12 l-4 10 l6 14" stroke="#2a2632" stroke-width="1.2" fill="none" opacity="0.7"/></g>')
    # Ein eingelassener, leuchtender Kristall in halber Höhe; ganz oben auch einer auf der Spitze.
    gems = [(100, 160, 1.0)] if tier == 2 else [(100, 160, 1.0), (100, 30, 1.2)] if tier >= 3 else []
    for gx, gy, k in gems:
        s.add(glow(s, gx, gy, 26 * k, 30 * k, '#c8a8ff', 0.55, 10))
        top, mid, bot, half = gy - 24 * k, gy - 4 * k, gy + 26 * k, 12 * k
        s.add(f'<path d="M{gx} {f(top)} L{f(gx + half)} {f(mid)} L{gx} {f(bot)} L{f(gx - half)} {f(mid)} Z" fill="#b890f8"/>'
              f'<path d="M{gx} {f(top)} L{f(gx + half)} {f(mid)} L{gx} {f(mid + 6 * k)} Z" fill="#e8d8ff"/>'
              f'<path d="M{gx} {f(top)} L{f(gx - half)} {f(mid)} L{gx} {f(mid + 6 * k)} Z" fill="#cbb0ff"/>'
              f'<path d="M{f(gx - half)} {f(mid)} L{gx} {f(mid + 6 * k)} L{gx} {f(bot)} Z" fill="#8a60d8"/>')
        sparkle(s, gx - 4, top + 10 * k, 6 * k)

    def crystal(x, y, h, w, tilt, color):
        a = math.radians(tilt)
        ux, uy = math.sin(a), -math.cos(a)
        px, py = math.cos(a), math.sin(a)
        tip = (x + ux * h, y + uy * h)
        bl, br = (x - px * w, y - py * w), (x + px * w, y + py * w)
        sl, sr = (bl[0] + ux * h * 0.72, bl[1] + uy * h * 0.72), (br[0] + ux * h * 0.72, br[1] + uy * h * 0.72)
        mid = (x + ux * h * 0.72, y + uy * h * 0.72)
        pts = lambda *ps: ' '.join(f'{f(p[0])} {f(p[1])}' for p in ps)
        return (f'<path d="M{pts(bl, sl, tip, mid)} L{pts((x, y))} Z" fill="{light(color, 0.35)}"/>'
                f'<path d="M{pts((x, y), mid, tip, sr, br)} Z" fill="{dark(color, 0.25)}"/>'
                f'<path d="M{pts(sl, tip, mid)} Z" fill="{light(color, 0.6)}" opacity="0.8"/>')
    # Fels am Fuß und eine Gruppe Kristalle davor.
    stone(s, 'M30 352 C30 330 52 318 80 320 L124 320 C150 318 172 330 172 352 Z', '#6a6478', (30, 316, 172, 352), 2.4)
    s.add(glow(s, 100, 334, 70, 16, '#d8c0ff', 0.4, 8))
    crystals = [(58, 348, 34, 7, -24, '#9a70e8'), (78, 350, 52, 9, -8, '#b890f8'), (96, 352, 40, 8, 6, '#8a60d8'),
                (122, 350, 58, 10, 14, '#a880f0'), (142, 348, 30, 7, 30, '#9a70e8'), (110, 352, 24, 6, -16, '#c8a8ff')]
    if tier == 1:
        crystals = crystals[1:4]
    if tier >= 3:
        crystals += [(40, 350, 26, 6, -36, '#c8a8ff'), (160, 350, 28, 6, 38, '#b890f8'), (66, 338, 70, 11, -14, '#a880f0'),
                     (134, 338, 72, 11, 12, '#9a70e8')]
        crystals.sort(key=lambda c: c[1])
    for x, y, h, w, t, c in crystals:
        s.add(crystal(x, y, h, w, t, c))
    stars = [(74, 306, 5), (128, 300, 6), (148, 330, 4), (60, 326, 3.5)]
    if tier >= 3:
        stars += [(40, 260, 5), (166, 230, 5), (72, 90, 4), (132, 110, 4)]
    glitter(s, stars[:1] if tier == 1 else stars, '#f4ecff')
    s.save(name('obelisk', tier))


def cat_basket(tier):
    s = Svg(152, 112, 'Katzenkorb: ein runder, geflochtener Weidenkorb mit rot-weiß kariertem Kissen und einem Wollknäuel davor', 209)
    shadow(s, 78, 104, 66, 7)
    if tier >= 3:
        halo(s, 76, 56, 76, 50)
    # Rückwand innen, dann das Kissen, dann die Vorderwand.
    s.add(f'<ellipse cx="76" cy="50" rx="62" ry="20" fill="{s.linear(0, 30, 0, 70, [(0, "#6e4a24"), (1, "#a87a40")])}"/>')
    cushion = 'M24 56 C24 40 128 40 128 56 C128 66 24 66 24 56 Z'
    s.add(f'<path d="{cushion}" fill="{"#c8b8a0" if tier == 1 else "#e8e0d4"}"/>')
    # Erst ein schlichtes Kissen, dann rot-weiß kariert, zuletzt königsblau mit Gold.
    check = {1: '#a89878', 2: '#c8302a', 3: '#3a5ab0'}[tier]
    checks = ''.join(f'<rect x="{x}" y="38" width="8" height="30" fill="{check}" opacity="0.75"/>' for x in range(24, 130, 16))
    checks += ''.join(f'<rect x="20" y="{y}" width="112" height="4" fill="{check}" opacity="0.45"/>' for y in (46, 54, 62))
    if tier == 1:
        checks = ''.join(f'<path d="M{x} 44 l4 3 M{x + 6} 56 l-3 4" stroke="{check}" stroke-width="1" opacity="0.5"/>' for x in range(30, 124, 14))
    if tier >= 3:
        checks += ''.join(f'<rect x="{x + 7}" y="38" width="1.5" height="30" fill="{GOLD}" opacity="0.9"/>' for x in range(24, 130, 16))
    s.add(f'<g {s.clip_d(cushion)}>{checks}<ellipse cx="60" cy="48" rx="40" ry="8" fill="#ffffff" opacity="0.35" {s.blur(4)}/>'
          f'<ellipse cx="76" cy="62" rx="56" ry="6" fill="#3a1a10" opacity="0.3" {s.blur(3)}/></g>')
    front = 'M14 52 C14 96 138 96 138 52 C138 72 14 72 14 52 Z'
    s.add(f'<path d="{front}" fill="{lit(s, (14, 52, 138, 96), "#b08048", (0.3, 0.3), 0.9, 0.45, 0.3)}"/>')
    # Das Geflecht: Weidenruten waagerecht, Staken senkrecht.
    weave = ''
    for k in range(5):
        y = 64 + k * 6
        weave += f'<path d="M14 {y - 10 + k * 1.5} C40 {y + 8 + k} 112 {y + 8 + k} 138 {y - 10 + k * 1.5}" stroke="#6e4a24" stroke-width="1.6" fill="none" opacity="0.6"/>'
    for x in range(22, 136, 10):
        weave += f'<path d="M{x} 58 L{x + (x - 76) * 0.08} 92" stroke="#e0b880" stroke-width="2.2" opacity="0.5"/>'
    s.add(f'<g {s.clip_d(front)}>{weave}</g>')
    s.add(f'<path d="M14 52 C14 72 138 72 138 52" stroke="{s.linear(14, 0, 138, 0, [(0, "#e8c890"), (1, "#8a5a2c")])}" stroke-width="7" fill="none" stroke-linecap="round"/>')
    if tier >= 2:
        # Ein Wollknäuel mit losem Faden.
        s.add(f'<circle cx="134" cy="96" r="10" fill="{s.radial(130, 92, 12, [(0, "#9ac8f0"), (1, "#3a6ab0")])}"/>'
              '<path d="M126 92 C132 88 140 92 142 98 M128 100 C132 94 140 96 140 102" stroke="#2a5090" stroke-width="1" fill="none" opacity="0.7"/>'
              '<path d="M124 100 C114 108 100 100 92 106" stroke="#5a8ad0" stroke-width="1.4" fill="none"/>')
        for x in (10, 22):
            s.add(flower(s, x, 100, 4.5, FLOWERS[x % 4]))
    if tier >= 3:
        # Ein goldenes Glöckchen an rotem Band vorn am Rand und ein Fisch zum Spielen.
        s.add('<path d="M60 72 C68 78 84 78 92 72" stroke="#c8302a" stroke-width="3" fill="none"/>')
        s.add(f'<path d="M70 76 C70 68 82 68 82 76 L84 82 L68 82 Z" fill="{s.radial(73, 72, 10, [(0, "#fff4c0"), (0.5, GOLD), (1, "#8a6414")])}"/>'
              '<circle cx="76" cy="83" r="1.8" fill="#6e4a14"/>')
        s.add(f'<path d="M30 96 C36 88 50 88 56 96 C50 104 36 104 30 96 Z M30 96 L22 90 L22 102 Z" fill="{s.linear(22, 0, 56, 0, [(0, "#9ab0c0"), (1, "#e8f0f8")])}"/>'
              '<circle cx="50" cy="95" r="1.4" fill="#2a3440"/>')
        glitter(s, [(76, 74, 5), (40, 30, 4), (120, 26, 5), (146, 60, 3.5)])
    s.save(name('katzenkorb', tier))


def maypole(tier):
    s = Svg(250, 420, 'Maibaum: ein hoher, geschälter Stamm mit Laubkranz und Krone, bunte Bänder laufen von oben zum Boden, Blumen am Fuß', 210)
    shadow(s, 128, 410, 110, 9)
    if tier >= 3:
        halo(s, 125, 70, 90, 70)
    top = (125, 72)
    colors = ['#d8503a', '#f0c040', '#5a9a48', '#5a8ad0', '#d86a90', '#f08a3a']
    # Erst zwei Bänder, dann sechs.
    feet = [52, 198] if tier == 1 else [22, 52, 84, 166, 198, 228]
    # Hintere Bänder zuerst, dann der Stamm, dann die vorderen.
    def band(i, x):
        c = colors[i % len(colors)]
        sway = (x - 125) * 0.25
        return (f'<path d="M{top[0]} {top[1]} C{f(top[0] + (x - top[0]) * 0.3 + sway)} 180 {f(x - sway * 0.5)} 300 {x} 404" '
                f'stroke="{c}" stroke-width="5" fill="none" stroke-linecap="round" opacity="0.95"/>')
    s.add(''.join(band(i, x) for i, x in enumerate(feet) if i % 2))
    pole = 'M119 408 L121 64 L129 64 L131 408 Z'
    s.add(f'<path d="{pole}" fill="{s.linear(119, 0, 131, 0, [(0, "#f4ecd8"), (0.4, "#d8c8a4"), (1, "#8a7a5a")])}"/>')
    # Ein Band windet sich spiralig um den Stamm.
    if tier >= 2:
        spiral = ''.join(f'<path d="M119 {y} L131 {y - 10}" stroke="{GOLD if tier >= 3 else colors[(y // 20) % 6]}" stroke-width="4"/>'
                         for y in range(100, 400, 20))
        s.add(f'<g {s.clip_d(pole)}>{spiral}</g>')
    s.add(''.join(band(i, x) for i, x in enumerate(feet) if i % 2 == 0))
    # Der Kranz am Ring, darüber eine kleine Krone aus Zweigen mit einem Wimpel.
    wreath = [(125 + math.cos(a) * 30, 74 + math.sin(a) * 10, 9) for a in [i * math.tau / 16 for i in range(16)]]
    foliage(s, wreath, '#4a8a34', 4, box=(90, 56, 160, 92))
    if tier >= 2:
        for i, a in enumerate([i * math.tau / 8 + 0.2 for i in range(8)]):
            x, y = 125 + math.cos(a) * 30, 76 + math.sin(a) * 10
            if math.sin(a) > -0.3:
                s.add(flower(s, x, y, 5, ['#ffffff', '#f2b8b0', '#f4e7a0', '#b98ad9'][i % 4]))
    if tier < 3:
        foliage(s, [(125, 34, 14), (112, 44, 11), (138, 44, 11), (125, 50, 12)], '#3e7a2a', 4, box=(100, 18, 150, 62))
    else:
        # Eine goldene Krone mit fünf Zacken und Kugeln darauf.
        crown = 'M100 62 L104 30 L114 46 L125 22 L136 46 L146 30 L150 62 Z'
        s.add(f'<path d="{crown}" fill="{lit(s, (100, 22, 150, 62), GOLD, (0.3, 0.3), 0.9, 0.45, 0.5)}"/>'
              f'<path d="M100 56 L150 56 L150 62 L100 62 Z" fill="{dark(GOLD, 0.25)}"/>')
        for x, y in ((104, 30), (125, 22), (146, 30)):
            s.add(f'<circle cx="{x}" cy="{y}" r="4" fill="{s.radial(x - 1, y - 1, 5, [(0, "#fff4c0"), (1, "#a87a1c")])}"/>')
        for x, c in ((114, '#d8503a'), (125, '#5a8ad0'), (136, '#5a9a48')):
            s.add(f'<circle cx="{x}" cy="51" r="3" fill="{c}"/>')
    if tier >= 2:
        s.add('<path d="M125 22 L125 6" stroke="#8a7a5a" stroke-width="2"/><path d="M125 6 L146 12 L125 18 Z" fill="#d8503a"/>')
    # Blumen und Gras am Fuß, wo die Bänder an Pflöcken festgemacht sind.
    for x in feet:
        s.add(f'<path d="M{x} 400 L{x} 412" stroke="#6e4c2c" stroke-width="3"/>')
    for _ in range(4 if tier == 1 else 18):
        x = s.rng.uniform(16, 234)
        s.add(flower(s, x, s.rng.uniform(400, 414), s.rng.uniform(3.5, 5), s.rng.choice(['#ffffff', '#ffffff', '#f2b8b0', '#b98ad9', '#f4e7a0'])))
    tufts(s, 10, 240, 412, 30, 11)
    if tier >= 3:
        bunting(s, 20, 150, 125, 110, 14, 7)
        bunting(s, 125, 110, 230, 150, 14, 7)
        glitter(s, [(104, 30, 6), (146, 34, 5), (125, 10, 5), (70, 100, 4), (190, 96, 4)])
    s.save(name('maibaum', tier))


def mole(tier):
    s = Svg(128, 152, 'Maulwurfsdenkmal: ein Maulwurf aus dunklem Holz schaut aus seinem Hügel, mit rosa Nase und großen rosa Grabhänden auf dem Rand', 211)
    shadow(s, 66, 146, 56, 6)
    if tier >= 3:
        halo(s, 64, 60, 62, 56)
    # Hinter dem Hügelrand: der samtschwarze Körper, nach oben gereckt, die spitze Schnauze schräg nach vorn.
    fur = '#3e3a40'
    g = lit(s, (30, 20, 100, 110), fur, (0.3, 0.25), 0.8, 0.5, 0.35)
    s.add(f'<g fill="{g}"><ellipse cx="62" cy="78" rx="30" ry="36"/><ellipse cx="70" cy="44" rx="22" ry="20"/>'
          '<path d="M82 34 C94 38 104 46 108 54 C100 60 88 58 80 52 Z"/></g>')
    s.add(s.strokes(s.scatter(60, 36, 26, 96, 110), 70, 5, 0.7, ['#6e6a74', '#1e1c22'], 0.5, 0.4))
    s.add(f'<ellipse cx="54" cy="52" rx="12" ry="9" fill="#a8a2b0" opacity="0.35" {s.blur(4)}/>')
    # Die rosa Rüsselnase und ein winziges, halb im Fell verstecktes Auge.
    s.add(f'<ellipse cx="108" cy="54" rx="6" ry="5" fill="{s.radial(106, 52, 7, [(0, "#ffd0d8"), (1, "#d07888")])}"/>'
          '<circle cx="107" cy="55" r="0.9" fill="#7a3040"/><circle cx="110" cy="54" r="0.9" fill="#7a3040"/>'
          '<ellipse cx="80" cy="42" rx="1.8" ry="1.2" fill="#0a080c"/>')
    s.add(s.strokes([(100, 50), (100, 56), (98, 53)], -20, 9, 0.6, ['#e8e0e8'], 0.7, 0.6))
    # Der Hügel davor verdeckt den Bauch.
    mound = 'M4 148 C8 112 34 96 64 96 C94 96 120 112 124 148 Z'
    earth_mound(s, mound, (4, 94, 124, 148))
    s.add(f'<path d="M24 104 C40 94 88 94 104 104" stroke="#2e1e10" stroke-width="4" fill="none" opacity="0.4" {s.blur(2)}/>')
    # Die breiten Grabhände liegen auf dem Rand, nach außen gedreht, mit hellen Krallen.
    for x, flip in ((30, -1), (94, 1)):
        hand = (f'M{x - 14 * flip} 104 C{x - 16 * flip} 94 {x + 4 * flip} 88 {x + 14 * flip} 94 '
                f'C{x + 18 * flip} 100 {x + 12 * flip} 110 {x} 110 C{x - 8 * flip} 110 {x - 12 * flip} 108 {x - 14 * flip} 104 Z')
        s.add(f'<path d="{hand}" fill="{s.radial(x - 4, 94, 18, [(0, "#ffd8de"), (0.6, "#e8a0ae"), (1, "#b86a7a")])}"/>')
        for i in range(5):
            cx = x - 12 * flip + i * 6 * flip
            s.add(f'<path d="M{cx} 108 l{flip * 1.5} 6" stroke="#f8f0e0" stroke-width="2.2" stroke-linecap="round"/>')
    tufts(s, 8, 120, 147, 12, 8)
    if tier >= 2:
        ribbon_bow(s, 62, 120, '#d0402e', 0.8)
        flower_row(s, 10, 118, 142, 5)
    if tier >= 3:
        # Ein goldener Grubenhelm mit leuchtender Lampe.
        helmet = 'M46 34 C46 14 92 12 94 32 L98 36 L42 38 Z'
        s.add(f'<path d="{helmet}" fill="{lit(s, (42, 12, 98, 38), GOLD, (0.3, 0.25), 0.9, 0.45, 0.5)}"/>'
              f'<path d="M42 36 L98 34 L98 39 L42 41 Z" fill="{dark(GOLD, 0.3)}"/>')
        s.add(glow(s, 70, 20, 16, 12, '#fff8c0', 0.7, 5))
        s.add('<circle cx="70" cy="22" r="6" fill="#8a6414"/><circle cx="70" cy="22" r="4.2" fill="#fffbe0"/>')
        glitter(s, [(56, 22, 4), (104, 76, 4), (20, 70, 4), (90, 14, 3.5)])
    s.save(name('maulwurf', tier))


def builder_arch(tier):
    s = Svg(288, 248, 'Baumeister-Bogen: ein Torbogen aus Stampferde und Steinen, über den sich dicke Wurzeln winden, darin ein dunkler Gang', 212)
    shadow(s, 148, 238, 132, 9)
    if tier >= 3:
        halo(s, 144, 110, 150, 110)
    # Der Bogen aus festgestampfter Erde mit eingesetzten Steinen.
    outer = 'M24 240 L24 120 C24 52 80 18 144 18 C208 18 264 52 264 120 L264 240 Z'
    inner = 'M84 240 L84 132 C84 98 110 76 144 76 C178 76 204 98 204 132 L204 240 Z'
    s.add(f'<path d="{outer} {inner}" fill-rule="evenodd" fill="{lit(s, (24, 18, 264, 240), "#8a6440", (0.3, 0.2), 0.9, 0.5, 0.3)}"/>')
    arch_clip = s.clip(f'<path d="{outer} {inner}" clip-rule="evenodd" fill-rule="evenodd"/>')
    crumbs = ''.join(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(s.rng.uniform(0.6, 2))}" fill="{s.rng.choice(["#b89068", "#3e2814", "#a07a50"])}" opacity="0.8"/>'
                     for x, y in s.scatter(500, 24, 18, 264, 240))
    layers = ''.join(f'<path d="M20 {y} H268" stroke="#5e4228" stroke-width="1.5" opacity="0.35"/>' for y in range(40, 240, 22))
    stones = ''
    for a in [math.pi * (0.06 + i * 0.0985) for i in range(10)]:
        x, y = 144 - math.cos(a) * 90, 128 - math.sin(a) * 82
        r = s.rng.uniform(9, 13)
        stones += (f'<ellipse cx="{f(x)}" cy="{f(y)}" rx="{f(r)}" ry="{f(r * 0.75)}" transform="rotate({f(math.degrees(a) - 90)} {f(x)} {f(y)})" '
                   f'fill="{s.radial(x - 3, y - 3, r * 1.3, [(0, "#d8d4c8"), (0.6, "#a8a496"), (1, "#6a665a")])}"/>')
    s.add(f'<g {arch_clip}>{layers}{crumbs}{stones}<ellipse cx="250" cy="150" rx="30" ry="110" fill="#1e140a" opacity="0.3" {s.blur(10)}/></g>')
    # Der Gang dahinter: dunkel, zur Tiefe hin schwarz.
    s.add(f'<path d="{inner}" fill="{s.radial(144, 210, 90, [(0, "#000000"), (0.6, "#140c06"), (1, "#3a2614")])}"/>'
          f'<path d="M92 150 C96 110 120 86 144 84" stroke="#6e4c2c" stroke-width="5" fill="none" opacity="0.5" {s.blur(2)}/>')
    # Wurzeln winden sich über den Bogen und an den Seiten herab.
    for k, (w, c) in enumerate(((9, '#6e4a2a'), (7, '#8a6440'), (5, '#a07a50'))):
        d = (f'M{14 + k * 8} 236 C{30 + k * 4} 170 {12 + k * 10} 110 {54 + k * 6} {62 + k * 4} '
             f'C{90 + k * 4} {26 + k * 6} {200 - k * 6} {22 + k * 6} {232 - k * 6} {60 + k * 6} '
             f'C{270 - k * 8} {104 + k * 4} {252 - k * 4} {176 - k * 2} {272 - k * 8} 236')
        s.add(f'<path d="{d}" stroke="{dark(c, 0.35)}" stroke-width="{w + 2}" fill="none" stroke-linecap="round" opacity="0.6"/>'
              f'<path d="{d}" stroke="{c}" stroke-width="{w}" fill="none" stroke-linecap="round"/>'
              f'<path d="{d}" stroke="{light(c, 0.35)}" stroke-width="{w * 0.3}" fill="none" stroke-linecap="round" opacity="0.6" transform="translate(-1.5 -1.5)"/>')
    s.add(s.strokes([(144 + s.rng.uniform(-60, 60), 80 + s.rng.uniform(-4, 6)) for _ in range(12)], 92, 14, 1.2, ['#c4a074', '#8a6a48'], 0.8, 0.3, 0.4))
    if tier >= 2:
        foliage(s, [(60, 50, 12), (76, 38, 10), (214, 42, 11), (230, 56, 12), (144, 20, 10)], '#4a8a34', 4, box=(40, 8, 250, 70))
        for x, y in ((64, 44), (220, 48), (150, 16), (80, 34)):
            s.add(flower(s, x, y, 4.5, FLOWERS[x % 5]))
        bunting(s, 92, 112, 196, 112, 12, 7)
    tufts(s, 16, 272, 240, 26, 11)
    if tier >= 2:
        flower_row(s, 20, 80, 236, 4)
        flower_row(s, 208, 270, 236, 4)
    if tier >= 3:
        # Ein goldener Schlussstein oben im Bogen und zwei Laternen an den Seiten.
        key = 'M132 14 L156 14 L152 46 L136 46 Z'
        s.add(f'<path d="{key}" fill="{lit(s, (132, 14, 156, 46), GOLD, (0.3, 0.25), 0.9, 0.45, 0.5)}"/>'
              '<path d="M140 24 l8 0 l-1 14 l-6 0 Z" fill="#a87a1c" opacity="0.6"/>')
        for x in (70, 218):
            s.add(f'<path d="M{x} 132 L{x} 146" stroke="#3e2814" stroke-width="2"/>')
            s.add(glow(s, x, 160, 16, 18, '#ffe08a', 0.7, 6))
            s.add(f'<path d="M{x - 8} 148 L{x + 8} 148 L{x + 6} 172 L{x - 6} 172 Z" fill="#fff0b0" stroke="{dark(GOLD, 0.3)}" stroke-width="2"/>'
                  f'<path d="M{x - 10} 146 L{x + 10} 146 L{x} 140 Z" fill="{GOLD}"/>')
        glitter(s, [(144, 10, 6), (40, 100, 5), (250, 96, 5), (100, 60, 4), (190, 60, 4)])
    s.save(name('bogen', tier))


def golden_clover(tier):
    s = Svg(220, 320, 'Goldenes Kleeblatt: ein glänzendes, vierblättriges Kleeblatt aus Gold auf einem Steinsockel, mit Lichtschein und Funkeln', 213)
    shadow(s, 114, 310, 94, 8)
    plinth(s, 50, 212, 170, 310)
    s.add(f'<path d="{block(80, 244, 140, 282, 3)}" fill="#968f80" opacity="0.6"/>')
    s.add('<path d="M110 276 C104 268 96 262 96 256 C96 250 104 248 110 254 C116 248 124 250 124 256 C124 262 116 268 110 276 Z" fill="#c8a040" opacity="0.8"/>')
    moss(s, 66, 304, 14, 5)
    # Erst aus Bronze, dann aus Silber, ganz oben aus Gold mit Schein.
    gold = {1: '#b8743a', 2: '#c8ccd4', 3: GOLD}[tier]
    vein = {1: '#6a3a14', 2: '#6a707a', 3: '#8a6414'}[tier]
    if tier >= 2:
        halo(s, 110, 110, 100, 100, light(gold, 0.4) if tier == 2 else '#fff0a0')
    # Der Stiel steckt im Sockel.
    s.add(f'<path d="M106 214 C104 180 108 160 110 140 L116 140 C114 162 112 184 114 214 Z" fill="{s.linear(104, 0, 116, 0, [(0, light(gold, 0.4)), (0.5, gold), (1, dark(gold, 0.45))])}"/>')

    # Vier Herzblätter: je zwei Rundungen außen, die Spitze zur Mitte.
    for angle in (-135, -45, 45, 135):
        a = math.radians(angle)
        px, py = math.cos(a), math.sin(a)
        qx, qy = -py, px
        lobes = [(112 + px * 32 + qx * 12 * side, 104 + py * 32 + qy * 12 * side) for side in (-1, 1)]
        tip = (112 + px * 4, 104 + py * 4)
        wing = [(112 + px * 30 + qx * 25 * side, 104 + py * 30 + qy * 25 * side) for side in (-1, 1)]
        shape = (f'<circle cx="{f(lobes[0][0])}" cy="{f(lobes[0][1])}" r="15"/><circle cx="{f(lobes[1][0])}" cy="{f(lobes[1][1])}" r="15"/>'
                 f'<path d="M{f(tip[0])} {f(tip[1])} L{f(wing[0][0])} {f(wing[0][1])} L{f(wing[1][0])} {f(wing[1][1])} Z"/>')
        s.add(f'<g fill="{lit(s, (50, 40, 176, 170), gold, (0.3, 0.25), 0.7, 0.5, 0.6)}">{shape}</g>')
        clip = s.clip(shape)
        s.add(f'<g {clip}><path d="M{f(tip[0])} {f(tip[1])} L{f(112 + px * 40)} {f(104 + py * 40)}" stroke="{vein}" stroke-width="1.6" opacity="0.6"/>'
              f'<ellipse cx="{f(lobes[0][0] - 3)}" cy="{f(lobes[0][1] - 5)}" rx="8" ry="5" fill="{light(gold, 0.7)}" opacity="0.55" {s.blur(3)}/>'
              f'<ellipse cx="{f(112 + px * 36)}" cy="{f(104 + py * 36)}" rx="22" ry="22" fill="none" stroke="{dark(gold, 0.25)}" stroke-width="4" opacity="0.35" {s.blur(2)}/></g>')
    s.add(f'<circle cx="112" cy="104" r="7" fill="{s.radial(110, 102, 8, [(0, light(gold, 0.6)), (1, dark(gold, 0.3))])}"/>')
    stars = [(62, 70, 7), (164, 82, 6), (150, 154, 5), (74, 150, 5), (112, 46, 8), (178, 130, 4)]
    glitter(s, stars[:2 * tier], light(gold, 0.8))
    tufts(s, 40, 190, 310, 22, 10)
    if tier >= 2:
        flower_row(s, 44, 186, 306, 8)
    if tier >= 3:
        laurel(s, 110, 262, 52, 160, 380, 12, '#f0c850', 5)
    s.save(name('kleeblatt', tier))


def alarm_bell(tier):
    s = Svg(128, 216, 'Alarmglocke: eine Glocke hängt in einem Holzgerüst mit kleinem Dach, ein Seil hängt herab; '
            'oben geschmückt, ganz oben aus Gold', 214)
    shadow(s, 66, 208, 56, 6)
    if tier >= 3:
        halo(s, 64, 100, 62, 80)
    wood = lambda x0, x1: s.linear(x0, 0, x1, 0, [(0, '#c49a64'), (0.45, '#8a643c'), (1, '#4a321c')])
    # Zwei Pfosten, schräg gegeneinander, mit Querbalken.
    for d in ('M20 206 L32 46 L42 46 L32 206 Z', 'M96 206 L86 46 L96 46 L108 206 Z'):
        s.add(f'<path d="{d}" fill="{wood(20, 108)}"/>')
        s.add(f'<g {s.clip_d(d)}>' + s.strokes(s.scatter(30, 20, 46, 108, 206), 90, 12, 0.8, ['#3e2814', '#d8b07a'], 0.5, 0.05) + '</g>')
    s.add(f'<path d="{block(22, 48, 106, 60, 3)}" fill="{s.linear(0, 48, 0, 60, [(0, "#d8b07a"), (1, "#6e4c2c")])}"/>'
          f'<path d="{block(30, 150, 98, 158, 2)}" fill="{s.linear(0, 150, 0, 158, [(0, "#c49a64"), (1, "#5a3c22")])}"/>')
    # Das Dach: Schindeln, ab Stufe 2 rot gestrichen.
    roof_color = '#8a6a48' if tier == 1 else '#c8402e'
    roof = 'M8 52 L64 12 L120 52 L112 56 L64 22 L16 56 Z'
    s.add(f'<path d="M8 52 L64 12 L120 52 Z" fill="{lit(s, (8, 12, 120, 52), roof_color, (0.3, 0.2), 0.9, 0.45, 0.3)}"/>'
          f'<path d="{roof}" fill="{dark(roof_color, 0.35)}"/>')
    for y in (26, 36, 46):
        half = (y - 12) * 56 / 40
        s.add(f'<path d="M{f(64 - half)} {y} L{f(64 + half)} {y}" stroke="{dark(roof_color, 0.3)}" stroke-width="1.4" opacity="0.6"/>')
    # Die Glocke: dunkle Bronze, ganz oben aus Gold.
    metal = {1: '#8a6a3a', 2: '#b0844a', 3: GOLD}[tier]
    bell = 'M64 64 C50 64 46 76 46 92 C46 106 40 114 36 118 L92 118 C88 114 82 106 82 92 C82 76 78 64 64 64 Z'
    s.add(f'<path d="M60 60 L68 60 L68 66 L60 66 Z" fill="#3e2814"/>'
          f'<path d="{bell}" fill="{lit(s, (36, 64, 92, 118), metal, (0.3, 0.25), 0.9, 0.5, 0.5)}"/>'
          f'<path d="M34 116 L94 116 L94 122 C80 126 48 126 34 122 Z" fill="{dark(metal, 0.25)}"/>'
          f'<path d="M52 80 C50 92 50 104 46 112" stroke="{light(metal, 0.6)}" stroke-width="3" fill="none" opacity="0.7" {s.blur(1)}/>')
    s.add(f'<circle cx="64" cy="126" r="5" fill="{dark(metal, 0.4)}"/>')
    # Das Seil zum Läuten.
    s.add('<path d="M64 128 C62 150 66 170 63 190" stroke="#d8c08a" stroke-width="3" fill="none"/>'
          '<path d="M64 128 C62 150 66 170 63 190" stroke="#8a7040" stroke-width="3" fill="none" stroke-dasharray="2 3"/>'
          '<ellipse cx="63" cy="192" rx="3.5" ry="5" fill="#c8a870"/>')
    for x, y, r in ((22, 204, 9), (106, 204, 9), (34, 208, 6), (94, 208, 6)):
        s.add(f'<ellipse cx="{x}" cy="{y}" rx="{r}" ry="{r * 0.7}" fill="{s.radial(x - 3, y - 3, r * 1.3, [(0, "#d8d4c8"), (0.6, "#a8a496"), (1, "#6a665a")])}"/>')
    tufts(s, 10, 118, 208, 14, 9)
    if tier >= 2:
        ribbon_bow(s, 64, 54, '#f0c040', 0.8)
        bunting(s, 30, 140, 98, 140, 6, 5)
        flower_row(s, 14, 114, 202, 6)
    if tier >= 3:
        # Eine goldene Kugel auf dem First.
        s.add(f'<circle cx="64" cy="9" r="5" fill="{s.radial(62, 7, 6, [(0, "#fff4c0"), (0.5, GOLD), (1, "#8a6414")])}"/>')
        glitter(s, [(54, 84, 6), (80, 100, 4), (24, 40, 4), (104, 40, 4), (64, 8, 4)])
    s.save(name('glocke', tier))


def icons():
    """Zwei Knöpfe für die Werkzeugleiste im flachen Stil der übrigen Symbole."""
    os.makedirs(f'{sys.argv[1] if len(sys.argv) > 1 else "."}/art_src/ui', exist_ok=True)
    root = sys.argv[1] if len(sys.argv) > 1 else '.'
    with open(f'{root}/art_src/ui/denkmaeler.svg', 'w', encoding='utf-8') as file:
        file.write('<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">'
                   '<path d="M14 50 h36 v8 h-36 Z" fill="#b2ad9f" stroke="#5a3a22" stroke-width="2.5"/>'
                   '<path d="M18 44 h28 v6 h-28 Z" fill="#c8c4b6" stroke="#5a3a22" stroke-width="2.5"/>'
                   '<ellipse cx="32" cy="38" rx="11" ry="7" fill="#d8d4c8" stroke="#5a3a22" stroke-width="2.5"/>'
                   '<ellipse cx="32" cy="27" rx="7" ry="9" fill="#d8d4c8" stroke="#5a3a22" stroke-width="2.5"/>'
                   '<path d="M28 20 C26 12 26 6 28 4 C31 6 31 14 30 19 M36 20 C38 12 39 6 37 4 C34 6 33 14 34 19" fill="#d8d4c8" stroke="#5a3a22" stroke-width="2.5" stroke-linejoin="round"/>'
                   '<circle cx="34" cy="25" r="1.4" fill="#5a3a22"/></svg>\n')
    with open(f'{root}/art_src/ui/errungenschaften.svg', 'w', encoding='utf-8') as file:
        file.write('<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">'
                   '<path d="M20 4 L30 28 L24 30 L14 8 Z" fill="#d8503a" stroke="#5a3a22" stroke-width="2.5" stroke-linejoin="round"/>'
                   '<path d="M44 4 L34 28 L40 30 L50 8 Z" fill="#5a8ad0" stroke="#5a3a22" stroke-width="2.5" stroke-linejoin="round"/>'
                   '<circle cx="32" cy="42" r="17" fill="#f4cf45" stroke="#5a3a22" stroke-width="3"/>'
                   '<circle cx="32" cy="42" r="11" fill="none" stroke="#c8962c" stroke-width="2"/>'
                   '<path d="M32 33 l2.8 5.8 6.2 0.9 -4.5 4.4 1.1 6.2 -5.6 -3 -5.6 3 1.1 -6.2 -4.5 -4.4 6.2 -0.9 Z" fill="#fff4c0" stroke="#c8962c" stroke-width="1.2" stroke-linejoin="round"/>'
                   '</svg>\n')


if __name__ == '__main__':
    for tier in (1, 2, 3):
        for draw in (statue, shovel, fox_stone, feather_mast, snow_rabbit, giant_carrot, ancestor_stone, obelisk,
                     cat_basket, maypole, mole, builder_arch, golden_clover, alarm_bell):
            draw(tier)
    icons()
    print('ok')
