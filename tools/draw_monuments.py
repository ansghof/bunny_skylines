"""Zeichnet die Denkmäler, die sich die Familie mit Errungenschaften verdient, im selben Stil wie
`draw_art.py`: weich schattiert, ohne dicke Umrisse, Licht von links oben. Alle stehen aufrecht auf
der Wiese wie Bäume; unten in der Mitte ist der Boden.

Aufruf aus dem Projektordner: `python tools/draw_monuments.py .`, danach `cargo run -p art_export`."""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from draw_art import Svg, dark, f, flower, foliage, glow, light, mix, ring  # noqa: E402

STONE = '#b2ad9f'
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


# ---------------------------------------------------------------------------------------------

def statue():
    s = Svg(240, 340, 'Kaninchenstatue: ein steinernes Kaninchen macht Männchen auf einem hohen Sockel, mit Moos und Gras am Fuß', 201)
    shadow(s, 124, 330, 104, 9)
    plinth(s, 46, 236, 194, 330)
    # Eine Tafel vorn auf dem Sockel, mit eingeritzter Pfote.
    s.add(f'<path d="{block(84, 266, 156, 306, 3)}" fill="{s.linear(0, 266, 0, 306, [(0, "#8e897c"), (1, "#a8a396")])}" opacity="0.8"/>')
    for dx, dy, r in ((-10, -6, 4), (-3, -11, 4), (5, -11, 4), (12, -6, 4)):
        s.add(f'<ellipse cx="{120 + dx}" cy="{290 + dy}" rx="{r * 0.8}" ry="{r}" fill="#6e6a5e" opacity="0.8"/>')
    s.add('<ellipse cx="120" cy="294" rx="10" ry="8" fill="#6e6a5e" opacity="0.8"/>')
    moss(s, 64, 322, 16, 7)
    moss(s, 182, 326, 12, 5)
    moss(s, 96, 240, 14, 4)

    # Das Kaninchen: eine einzige Steinform aus Teilen, die alle dasselbe Licht bekommen.
    box = (58, 20, 186, 240)
    g = lit(s, box, STONE, (0.3, 0.3), 0.8, 0.5)
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
    s.add(f'<g fill="{lit(s, (130, 130, 170, 170), STONE, (0.3, 0.2), 1.0, 0.4)}">'
          '<ellipse cx="146" cy="146" rx="11" ry="16" transform="rotate(-20 146 146)"/>'
          '<ellipse cx="160" cy="150" rx="10" ry="15" transform="rotate(-25 160 150)"/></g>')
    s.add('<ellipse cx="143" cy="46" rx="5" ry="30" transform="rotate(10 143 46)" fill="#8a8578" opacity="0.6"/>'
          '<ellipse cx="119" cy="48" rx="4" ry="26" transform="rotate(-12 119 48)" fill="#8a8578" opacity="0.45"/>')
    # Schatten unter Kopf und Brust, Licht an der linken Kante.
    s.add(f'<ellipse cx="128" cy="124" rx="28" ry="8" fill="#2a261c" opacity="0.3" {s.blur(4)}/>'
          f'<ellipse cx="122" cy="186" rx="40" ry="8" fill="#2a261c" opacity="0.25" {s.blur(5)}/>'
          f'<path d="M92 120 C84 150 84 180 74 200" stroke="#efeadc" stroke-width="4" fill="none" opacity="0.5" {s.blur(2)}/>')
    # Gesicht: Auge, Nase, Schnurrhaarlöcher.
    s.add('<ellipse cx="146" cy="90" rx="5" ry="6" fill="#5a564c"/><ellipse cx="144.5" cy="88" rx="1.6" ry="2" fill="#d8d4c8"/>'
          '<path d="M160 104 q3 2 1 5" stroke="#5a564c" stroke-width="2" fill="none"/>'
          '<circle cx="154" cy="110" r="0.9" fill="#6e6a5e"/><circle cx="150" cy="112" r="0.9" fill="#6e6a5e"/>')
    # Ein feiner Riss und Flechten, damit der Stein alt wirkt.
    s.add('<path d="M102 176 l6 10 l-3 9 l7 12" stroke="#6e6a5e" stroke-width="1.2" fill="none" opacity="0.7"/>')
    for x, y in ((100, 214), (150, 214), (112, 140), (128, 74)):
        s.add(f'<circle cx="{x}" cy="{y}" r="{f(s.rng.uniform(3, 5))}" fill="#c8c48a" opacity="0.45" {s.blur(1)}/>')
    tufts(s, 40, 200, 330, 22)
    s.save('einrichtung/denkmal_statue.svg')


def shovel():
    s = Svg(128, 184, 'Schaufelmonument: eine große Schaufel steckt in einem Erdhaufen, mit roter Ehrenschleife am Stiel', 202)
    shadow(s, 66, 178, 58, 6)
    # Der Stiel ragt schräg aus dem Haufen; die Klinge steckt halb in der Erde.
    handle = 'M58 112 L72 24 L80 25 L67 113 Z'
    s.add(f'<path d="{handle}" fill="{s.linear(58, 0, 80, 0, [(0, "#d8b07a"), (0.45, "#b08350"), (1, "#6e4c2c")])}"/>')
    s.add(f'<g {s.clip_d(handle)}>' + s.strokes(s.scatter(24, 58, 24, 80, 112), 98, 10, 0.8, ['#7a5634', '#e8c890'], 0.6, 0.08) + '</g>')
    # Griff oben: ein Querholz.
    s.add(f'<path d="M60 22 C62 14 88 14 92 22 C90 28 62 28 60 22 Z" fill="{s.linear(0, 14, 0, 28, [(0, "#e0bc88"), (1, "#7a5634")])}"/>'
          '<path d="M66 22 L86 22" stroke="#5a3c22" stroke-width="1" opacity="0.5"/>')
    # Die Klinge, Metall mit Glanzkante und etwas Rost.
    blade = 'M40 150 C38 128 46 110 64 108 C80 108 90 124 88 148 L82 156 L46 156 Z'
    s.add(f'<path d="{blade}" fill="{lit(s, (38, 104, 92, 156), "#9aa2a6", (0.25, 0.2), 0.9, 0.5, 0.5)}"/>')
    s.add(f'<path d="M46 132 C48 118 56 112 64 112" stroke="#f4f8fa" stroke-width="3" fill="none" opacity="0.7" {s.blur(1)}/>'
          f'<g {s.clip_d(blade)}><ellipse cx="80" cy="140" rx="10" ry="16" fill="#8a5a2c" opacity="0.35" {s.blur(4)}/></g>'
          '<path d="M60 110 L64 108 L68 110 L66 124 L62 124 Z" fill="#6e767a"/>')
    # Der Erdhaufen davor verdeckt die Spitze der Klinge.
    mound = 'M6 178 C10 156 28 140 52 142 C60 146 70 146 80 142 C102 140 120 158 124 178 Z'
    earth_mound(s, mound, (6, 140, 124, 178))
    s.add(f'<path d="M40 148 C50 152 76 152 88 146" stroke="#2e1e10" stroke-width="3" fill="none" opacity="0.45" {s.blur(1.5)}/>')
    ribbon_bow(s, 70, 64, '#d0402e', 1.0)
    tufts(s, 8, 122, 177, 14, 8)
    for x in (22, 104):
        s.add(flower(s, x, 168, 4, '#ffffff'))
    s.save('einrichtung/denkmal_schaufel.svg')


def fox_stone():
    s = Svg(260, 210, 'Fuchsschreck: ein mächtiger Findling mit tiefen Kratzspuren, Moos obenauf und ein Büschel rotes Fuchsfell', 203)
    shadow(s, 134, 198, 120, 10, 0.45)
    d = ('M18 198 C10 160 22 104 58 74 C90 46 140 34 182 50 C222 66 248 112 246 158 C246 178 240 192 232 198 Z')
    stone(s, d, '#a39e90', (10, 34, 248, 198), 2.6, True, (0.28, 0.18))
    # Kanten und Flächen: eine hellere Oberseite, dunklere rechte Flanke.
    s.add(f'<g {s.clip_d(d)}><path d="M40 96 C80 56 150 44 200 66" stroke="#e4e0d2" stroke-width="10" fill="none" opacity="0.35" {s.blur(5)}/>'
          f'<ellipse cx="226" cy="150" rx="40" ry="70" fill="#2a261c" opacity="0.3" {s.blur(12)}/>'
          f'<ellipse cx="130" cy="200" rx="120" ry="20" fill="#2a261c" opacity="0.35" {s.blur(8)}/></g>')
    # Drei Kratzspuren schräg über den Stein: tiefe Rillen mit heller Kante.
    for i in range(3):
        x = 104 + i * 22
        s.add(f'<path d="M{x} 92 C{x + 8} 112 {x + 14} 134 {x + 16} 160" stroke="#4a463c" stroke-width="5" fill="none" stroke-linecap="round"/>'
              f'<path d="M{x - 2} 93 C{x + 6} 113 {x + 12} 135 {x + 14} 160" stroke="#e8e4d6" stroke-width="1.5" fill="none" opacity="0.7" stroke-linecap="round"/>')
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
    s.save('einrichtung/denkmal_fuchsschreck.svg')


def feather_mast():
    s = Svg(104, 260, 'Federmast: ein Holzpfahl mit einem Fächer gestreifter Bussardfedern und bunten Bändern, unten von Steinen gehalten', 204)
    shadow(s, 54, 252, 40, 6)
    pole = 'M47 250 L49 36 L57 36 L59 250 Z'
    s.add(f'<path d="{pole}" fill="{s.linear(47, 0, 59, 0, [(0, "#c49a64"), (0.4, "#8a643c"), (1, "#4a321c")])}"/>')
    s.add(f'<g {s.clip_d(pole)}>' + s.strokes(s.scatter(40, 47, 36, 59, 250), 90, 12, 0.8, ['#3e2814', '#d8b07a'], 0.5, 0.05) + '</g>')
    s.add(f'<ellipse cx="53" cy="34" rx="7" ry="4" fill="{s.radial(51, 33, 7, [(0, "#e8c890"), (1, "#8a643c")])}"/>')
    # Die Federn: ein Fächer, unten zusammengebunden, mit hellen und dunklen Querbändern.
    for i, a in enumerate((-48, -30, -12, 6, 24, 42)):
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
    for color, dx, sway in (('#d0402e', -4, -14), ('#4a7ad0', 2, 10), ('#f0c040', 6, 18)):
        s.add(f'<path d="M{53 + dx} 76 C{48 + dx + sway} 96 {58 + dx - sway} 112 {50 + dx + sway} 132" stroke="{color}" '
              f'stroke-width="3.5" fill="none" stroke-linecap="round"/>')
    # Steine halten den Pfahl.
    for x, y, r in ((36, 246, 11), (70, 246, 12), (53, 240, 10), (46, 252, 9), (64, 252, 9)):
        s.add(f'<ellipse cx="{x}" cy="{y}" rx="{r}" ry="{r * 0.72}" fill="{s.radial(x - 3, y - 3, r * 1.3, [(0, "#d8d4c8"), (0.6, "#a8a496"), (1, "#6a665a")])}"/>')
    tufts(s, 16, 90, 254, 12, 9)
    s.save('einrichtung/denkmal_federmast.svg')


def snow_rabbit():
    s = Svg(132, 168, 'Winterkaninchen: ein Kaninchen aus Schnee und Eis auf einem flachen Stein, mit Eiszapfen und Glitzern', 205)
    shadow(s, 68, 160, 56, 6)
    stone(s, block(16, 132, 116, 162, 8), '#9a9688', (16, 132, 116, 162), 1.6)
    s.add('<path d="M22 136 C40 130 92 130 110 136" stroke="#f4f8ff" stroke-width="6" fill="none" stroke-linecap="round"/>')
    for x in (30, 48, 86, 102):
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
    for x, y, r in ((44, 92, 6), (96, 56, 5), (60, 118, 4), (108, 100, 3.5), (30, 60, 4)):
        sparkle(s, x, y, r, '#ffffff')
    s.save('einrichtung/denkmal_winterkaninchen.svg')


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


def giant_carrot():
    s = Svg(220, 320, 'Riesenmöhre: eine aus Holz geschnitzte, bemalte Möhre steckt mit der Spitze in der Erde, oben ein Schopf gefiederter Blätter', 206)
    shadow(s, 112, 310, 92, 8)
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
    ribbon_bow(s, 64, 138, '#f0c040', 1.1)
    s.save('einrichtung/denkmal_riesenmoehre.svg')


def ancestor_stone():
    s = Svg(128, 168, 'Ahnenstein: ein aufrechter Stein mit eingeritzten Pfotenabdrücken, einer Reihe pro Generation, Moos und Blumen am Fuß', 207)
    shadow(s, 66, 162, 52, 6)
    d = 'M26 162 C22 120 22 62 32 36 C42 14 86 12 98 34 C108 58 108 120 104 162 Z'
    stone(s, d, '#a8a496', (22, 12, 108, 162), 2.0)
    s.add(f'<g {s.clip_d(d)}><ellipse cx="102" cy="100" rx="16" ry="70" fill="#2a261c" opacity="0.3" {s.blur(8)}/>'
          f'<path d="M34 50 C40 26 70 18 88 30" stroke="#ece8da" stroke-width="6" fill="none" opacity="0.4" {s.blur(3)}/></g>')

    def paw(x, y, r):
        out = f'<ellipse cx="{f(x)}" cy="{f(y + r * 0.4)}" rx="{f(r)}" ry="{f(r * 0.8)}" fill="#5e5a4e"/>'
        for dx, dy in ((-1.0, -0.9), (-0.35, -1.35), (0.35, -1.35), (1.0, -0.9)):
            out += f'<ellipse cx="{f(x + dx * r)}" cy="{f(y + dy * r)}" rx="{f(r * 0.34)}" ry="{f(r * 0.42)}" fill="#5e5a4e"/>'
        return f'<g opacity="0.85">{out}</g><g opacity="0.4" transform="translate(-0.8 -0.8)">{out.replace("#5e5a4e", "#e8e4d6")}</g>'
    # Ganz oben die Gründer, darunter immer mehr Nachkommen.
    rows = [(1, 46), (2, 70), (3, 94), (4, 118), (5, 140)]
    for n, y in rows:
        r = 4.2 - n * 0.35
        for i in range(n):
            x = 65 + (i - (n - 1) / 2) * (r * 4.2)
            s.add(paw(x, y, r))
    moss(s, 36, 156, 14, 6)
    moss(s, 96, 52, 8, 5)
    tufts(s, 18, 112, 162, 14, 9)
    for x, c in ((20, '#f2b8b0'), (108, '#ffffff'), (98, '#b98ad9')):
        s.add(flower(s, x, 156, 4.5, c))
    s.save('einrichtung/denkmal_ahnenstein.svg')


def obelisk():
    s = Svg(200, 360, 'Kristallobelisk: ein hoher, dunkler Steinpfeiler aus der Tiefe, mit leuchtenden Kristallen am Fuß und einem eingelassenen Kristall', 208)
    shadow(s, 104, 350, 86, 8)
    s.add(f'<ellipse cx="100" cy="320" rx="80" ry="30" fill="#b48cf0" opacity="0.25" {s.blur(16)}/>')
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
    # Ein eingelassener, leuchtender Kristall in halber Höhe.
    s.add(glow(s, 100, 160, 26, 30, '#c8a8ff', 0.55, 10))
    s.add('<path d="M100 136 L112 156 L100 186 L88 156 Z" fill="#b890f8"/>'
          '<path d="M100 136 L112 156 L100 162 Z" fill="#e8d8ff"/><path d="M100 136 L88 156 L100 162 Z" fill="#cbb0ff"/>'
          '<path d="M88 156 L100 162 L100 186 Z" fill="#8a60d8"/>')
    sparkle(s, 96, 146, 6)

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
    for x, y, h, w, t, c in ((58, 348, 34, 7, -24, '#9a70e8'), (78, 350, 52, 9, -8, '#b890f8'), (96, 352, 40, 8, 6, '#8a60d8'),
                             (122, 350, 58, 10, 14, '#a880f0'), (142, 348, 30, 7, 30, '#9a70e8'), (110, 352, 24, 6, -16, '#c8a8ff')):
        s.add(crystal(x, y, h, w, t, c))
    for x, y, r in ((74, 306, 5), (128, 300, 6), (148, 330, 4), (60, 326, 3.5)):
        sparkle(s, x, y, r, '#f4ecff')
    s.save('einrichtung/denkmal_obelisk.svg')


def cat_basket():
    s = Svg(152, 112, 'Katzenkorb: ein runder, geflochtener Weidenkorb mit rot-weiß kariertem Kissen und einem Wollknäuel davor', 209)
    shadow(s, 78, 104, 66, 7)
    # Rückwand innen, dann das Kissen, dann die Vorderwand.
    s.add(f'<ellipse cx="76" cy="50" rx="62" ry="20" fill="{s.linear(0, 30, 0, 70, [(0, "#6e4a24"), (1, "#a87a40")])}"/>')
    cushion = 'M24 56 C24 40 128 40 128 56 C128 66 24 66 24 56 Z'
    s.add(f'<path d="{cushion}" fill="#e8e0d4"/>')
    checks = ''.join(f'<rect x="{x}" y="38" width="8" height="30" fill="#c8302a" opacity="0.75"/>' for x in range(24, 130, 16))
    checks += ''.join(f'<rect x="20" y="{y}" width="112" height="4" fill="#c8302a" opacity="0.45"/>' for y in (46, 54, 62))
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
    # Ein Wollknäuel mit losem Faden.
    s.add(f'<circle cx="134" cy="96" r="10" fill="{s.radial(130, 92, 12, [(0, "#9ac8f0"), (1, "#3a6ab0")])}"/>'
          '<path d="M126 92 C132 88 140 92 142 98 M128 100 C132 94 140 96 140 102" stroke="#2a5090" stroke-width="1" fill="none" opacity="0.7"/>'
          '<path d="M124 100 C114 108 100 100 92 106" stroke="#5a8ad0" stroke-width="1.4" fill="none"/>')
    s.save('einrichtung/denkmal_katzenkorb.svg')


def maypole():
    s = Svg(250, 420, 'Maibaum: ein hoher, geschälter Stamm mit Laubkranz und Krone, bunte Bänder laufen von oben zum Boden, Blumen am Fuß', 210)
    shadow(s, 128, 410, 110, 9)
    top = (125, 72)
    colors = ['#d8503a', '#f0c040', '#5a9a48', '#5a8ad0', '#d86a90', '#f08a3a']
    feet = [22, 52, 84, 166, 198, 228]
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
    spiral = ''.join(f'<path d="M119 {y} L131 {y - 10}" stroke="{colors[(y // 20) % 6]}" stroke-width="4"/>' for y in range(100, 400, 20))
    s.add(f'<g {s.clip_d(pole)}>{spiral}</g>')
    s.add(''.join(band(i, x) for i, x in enumerate(feet) if i % 2 == 0))
    # Der Kranz am Ring, darüber eine kleine Krone aus Zweigen mit einem Wimpel.
    wreath = [(125 + math.cos(a) * 30, 74 + math.sin(a) * 10, 9) for a in [i * math.tau / 16 for i in range(16)]]
    foliage(s, wreath, '#4a8a34', 4, box=(90, 56, 160, 92))
    for i, a in enumerate([i * math.tau / 8 + 0.2 for i in range(8)]):
        x, y = 125 + math.cos(a) * 30, 76 + math.sin(a) * 10
        if math.sin(a) > -0.3:
            s.add(flower(s, x, y, 5, ['#ffffff', '#f2b8b0', '#f4e7a0', '#b98ad9'][i % 4]))
    foliage(s, [(125, 34, 14), (112, 44, 11), (138, 44, 11), (125, 50, 12)], '#3e7a2a', 4, box=(100, 18, 150, 62))
    s.add('<path d="M125 22 L125 6" stroke="#8a7a5a" stroke-width="2"/><path d="M125 6 L146 12 L125 18 Z" fill="#d8503a"/>')
    # Blumen und Gras am Fuß, wo die Bänder an Pflöcken festgemacht sind.
    for x in feet:
        s.add(f'<path d="M{x} 400 L{x} 412" stroke="#6e4c2c" stroke-width="3"/>')
    for _ in range(18):
        x = s.rng.uniform(16, 234)
        s.add(flower(s, x, s.rng.uniform(400, 414), s.rng.uniform(3.5, 5), s.rng.choice(['#ffffff', '#ffffff', '#f2b8b0', '#b98ad9', '#f4e7a0'])))
    tufts(s, 10, 240, 412, 30, 11)
    s.save('einrichtung/denkmal_maibaum.svg')


def mole():
    s = Svg(128, 152, 'Maulwurfsdenkmal: ein Maulwurf aus dunklem Holz schaut aus seinem Hügel, mit rosa Nase und großen rosa Grabhänden auf dem Rand', 211)
    shadow(s, 66, 146, 56, 6)
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
    ribbon_bow(s, 62, 120, '#d0402e', 0.8)
    s.save('einrichtung/denkmal_maulwurf.svg')


def builder_arch():
    s = Svg(288, 248, 'Baumeister-Bogen: ein Torbogen aus Stampferde und Steinen, über den sich dicke Wurzeln winden, darin ein dunkler Gang', 212)
    shadow(s, 148, 238, 132, 9)
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
    foliage(s, [(60, 50, 12), (76, 38, 10), (214, 42, 11), (230, 56, 12), (144, 20, 10)], '#4a8a34', 4, box=(40, 8, 250, 70))
    tufts(s, 16, 272, 240, 26, 11)
    s.save('einrichtung/denkmal_bogen.svg')


def golden_clover():
    s = Svg(220, 320, 'Goldenes Kleeblatt: ein glänzendes, vierblättriges Kleeblatt aus Gold auf einem Steinsockel, mit Lichtschein und Funkeln', 213)
    shadow(s, 114, 310, 94, 8)
    plinth(s, 50, 212, 170, 310)
    s.add(f'<path d="{block(80, 244, 140, 282, 3)}" fill="#968f80" opacity="0.6"/>')
    s.add('<path d="M110 276 C104 268 96 262 96 256 C96 250 104 248 110 254 C116 248 124 250 124 256 C124 262 116 268 110 276 Z" fill="#c8a040" opacity="0.8"/>')
    moss(s, 66, 304, 14, 5)
    s.add(glow(s, 110, 110, 90, 90, '#fff0a0', 0.4, 26))
    # Der Stiel steckt im Sockel.
    s.add(f'<path d="M106 214 C104 180 108 160 110 140 L116 140 C114 162 112 184 114 214 Z" fill="{s.linear(104, 0, 116, 0, [(0, "#f8e08a"), (0.5, "#c89a2c"), (1, "#7a5a14")])}"/>')
    gold = '#e0b440'

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
        s.add(f'<g {clip}><path d="M{f(tip[0])} {f(tip[1])} L{f(112 + px * 40)} {f(104 + py * 40)}" stroke="#8a6414" stroke-width="1.6" opacity="0.6"/>'
              f'<ellipse cx="{f(lobes[0][0] - 3)}" cy="{f(lobes[0][1] - 5)}" rx="8" ry="5" fill="#fff8d0" opacity="0.55" {s.blur(3)}/>'
              f'<ellipse cx="{f(112 + px * 36)}" cy="{f(104 + py * 36)}" rx="22" ry="22" fill="none" stroke="#a87a1c" stroke-width="4" opacity="0.35" {s.blur(2)}/></g>')
    s.add(f'<circle cx="112" cy="104" r="7" fill="{s.radial(110, 102, 8, [(0, "#fff4c0"), (1, "#a87a1c")])}"/>')
    for x, y, r in ((62, 70, 7), (164, 82, 6), (150, 154, 5), (74, 150, 5), (112, 46, 8), (178, 130, 4)):
        sparkle(s, x, y, r, '#fffbe0')
    tufts(s, 40, 190, 310, 22, 10)
    s.save('einrichtung/denkmal_kleeblatt.svg')


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
    statue()
    shovel()
    fox_stone()
    feather_mast()
    snow_rabbit()
    giant_carrot()
    ancestor_stone()
    obelisk()
    cat_basket()
    maypole()
    mole()
    builder_arch()
    golden_clover()
    icons()
    print('ok')
