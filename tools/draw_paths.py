"""Zeichnet die Stücke für Trampelpfade und Dämme als SVG. Alle Stücke treffen die Kantenmitte
in derselben Breite, damit benachbarte nahtlos zusammenpassen. Oben im Bild ist Norden."""
import sys

# Aufruf aus dem Projektordner: `python tools/draw_paths.py .`, danach `cargo run -p art_export`.
root = sys.argv[1]
EDGE = "#a8864e"   # Rand des festgetretenen Wegs
FILL = "#d2b47c"   # Wegmitte
# Breite des Wegs: dunkler Rand 34..94, helle Mitte 40..88.
OUT = (34, 94)
IN = (40, 88)


def band_ns(lo, hi, y0=0, y1=128):
    return f'<rect x="{lo}" y="{y0}" width="{hi - lo}" height="{y1 - y0}"/>'


def band_ew(lo, hi, x0=0, x1=128):
    return f'<rect x="{x0}" y="{lo}" width="{x1 - x0}" height="{hi - lo}"/>'


def arm(direction, lo, hi):
    """Ein Wegstück von der Mitte bis zur Kante in Richtung N, E, S oder W."""
    # Jeder Arm endet genau in der Mitte; der runde Mittelplatz schließt Enden und Abzweigungen.
    return {
        'N': band_ns(lo, hi, 0, 64),
        'S': band_ns(lo, hi, 64, 128),
        'E': band_ew(lo, hi, 64, 128),
        'W': band_ew(lo, hi, 0, 64),
    }[direction]


def hub(lo, hi):
    """Ein runder Platz in der Mitte, damit Abzweigungen weich wirken."""
    r = (hi - lo) / 2
    return f'<circle cx="64" cy="64" r="{r}"/>'


def curve_ne(lo, hi):
    """Ein Viertelkreis von der Nordkante zur Ostkante um die Ecke rechts oben."""
    r_in, r_out = 128 - hi, 128 - lo
    return (f'<path d="M {lo} 0 A {r_out} {r_out} 0 0 0 128 {r_out} '
            f'L 128 {r_in} A {r_in} {r_in} 0 0 1 {hi} 0 Z"/>')


def dots(spots, color, rx, ry):
    return ''.join(f'<ellipse cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="{color}"/>' for x, y in spots)


# Jede Art: Kommentar, Schichten von außen nach innen als (Farbe, Breite) und die Tupfer darauf.
STYLES = {
    'pfad': ('Trampelpfad: festgetretene Erde mit hellerer Mitte und ein paar Kieseln',
             [(EDGE, OUT), (FILL, IN)], ('#9a8466', 3, 2.2)),
    # Ein breiter Erdwall mit hellerer Krone und eingeschlagenen Holzpflöcken.
    'damm': ('Damm: ein aufgeschütteter Erdwall mit hellerer Krone und Holzpflöcken',
             [('#5e4128', (24, 104)), ('#8a6240', (30, 98)), ('#a87d52', (48, 80))], ('#4a2e18', 4.5, 4.5)),
}


def tile(style, shapes, spots):
    comment, layers, (dot_color, rx, ry) = STYLES[style]
    groups = ''.join(f'  <g fill="{color}">' + ''.join(shape(*width) for shape in shapes) + '</g>\n'
                     for color, width in layers)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">\n'
            f'  <!-- {comment} -->\n{groups}  {dots(spots, dot_color, rx, ry)}\n</svg>\n')


def a(direction):
    return lambda lo, hi: arm(direction, lo, hi)


pieces = {
    # Verbindungen: oben = Norden.
    '_punkt': ([hub], [(56, 60), (70, 70)]),
    '_ende': ([a('N'), hub], [(58, 20), (68, 52)]),
    '_gerade': ([a('N'), a('S'), hub], [(56, 18), (70, 60), (58, 104)]),
    '_kurve': ([curve_ne], [(66, 26), (96, 58)]),
    '_t': ([a('N'), a('S'), a('E'), hub], [(56, 22), (100, 66), (60, 106)]),
    '_kreuz': ([a('N'), a('S'), a('E'), a('W'), hub], [(56, 20), (104, 60), (22, 70), (68, 108)]),
}
for style in STYLES:
    for name, (shapes, spots) in pieces.items():
        with open(f'{root}/art_src/einrichtung/{style}{name}.svg', 'w', encoding='utf-8') as f:
            f.write(tile(style, shapes, spots))
print('ok', len(STYLES) * len(pieces))
