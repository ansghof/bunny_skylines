"""Zeichnet die Stücke für Trampelpfade und Dämme als SVG. Alle Stücke treffen die Kantenmitte
in derselben Breite, damit benachbarte nahtlos zusammenpassen. Oben im Bild ist Norden."""
import random
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
        # Über die Kante hinaus, damit der weiche Rand an der Kante nicht ausdünnt.
        'N': band_ns(lo, hi, -16, 64),
        'S': band_ns(lo, hi, 64, 144),
        'E': band_ew(lo, hi, 64, 144),
        'W': band_ew(lo, hi, -16, 64),
    }[direction]


def hub(lo, hi):
    """Ein runder Platz in der Mitte, damit Abzweigungen weich wirken."""
    r = (hi - lo) / 2
    return f'<circle cx="64" cy="64" r="{r}"/>'


def curve_ne(lo, hi):
    """Ein Viertelkreis von der Nordkante zur Ostkante um die Ecke rechts oben."""
    r_in, r_out = 128 - hi, 128 - lo
    return (f'<path d="M {lo} 0 A {r_out} {r_out} 0 0 0 128 {r_out} '
            f'L 128 {r_in} A {r_in} {r_in} 0 0 1 {hi} 0 Z"/>' + band_ns(lo, hi, -16, 0) + band_ew(lo, hi, 128, 144))


def dots(spots, color, rx, ry):
    """Kiesel: rundlich schattiert, mit kleinem Schatten."""
    return ''.join(f'<ellipse cx="{x + 0.8}" cy="{y + 1}" rx="{rx}" ry="{ry}" fill="#2a1e10" opacity="0.35"/>'
                   f'<ellipse cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="{color}"/>'
                   f'<ellipse cx="{x - rx * 0.3}" cy="{y - ry * 0.35}" rx="{rx * 0.45}" ry="{ry * 0.35}" fill="#ffffff" opacity="0.35"/>'
                   for x, y in spots)


# Jede Art: Kommentar, Schichten von außen nach innen als (Farbe, Breite, Deckkraft, Weichheit),
# die Krümel darauf und die Kiesel. Die Ränder sind weich, damit der Weg in die Wiese übergeht.
STYLES = {
    'pfad': ('Trampelpfad: festgetretene Erde mit abgewetztem Gras am Rand, Krümeln und ein paar Kieseln',
             [('#8f8a50', (28, 100), 0.55, 4), (EDGE, OUT, 0.9, 2.5), (FILL, IN, 0.85, 3.5)],
             ['#8a6e48', '#e2cca0', '#6e5638'], ('#a8a090', 3, 2.2)),
    # Ein breiter Erdwall mit hellerer Krone und eingeschlagenen Holzpflöcken.
    'damm': ('Damm: ein aufgeschütteter Erdwall mit hellerer Krone und Holzpflöcken',
             [('#3e2a18', (20, 108), 0.6, 4), ('#5e4128', (24, 104), 1, 2), ('#8a6240', (30, 98), 1, 3),
              ('#a87d52', (48, 80), 0.9, 4)],
             ['#4a2e18', '#c49a6c', '#6e4c2c'], ('#5a3a1e', 4.5, 4.5)),
}


NL = chr(10)


def tile(style, shapes, spots, seed):
    comment, layers, crumbs, (dot_color, rx, ry) = STYLES[style]
    rng = random.Random(seed)
    defs, groups = [], []
    for i, (color, width, opacity, soft) in enumerate(layers):
        defs.append(f'<filter id="w{i}" x="-20%" y="-20%" width="140%" height="140%">'
                    f'<feTurbulence type="fractalNoise" baseFrequency="0.09" numOctaves="2" seed="{i}"/>'
                    f'<feDisplacementMap in="SourceGraphic" scale="{soft}" xChannelSelector="R" yChannelSelector="G"/>'
                    f'<feGaussianBlur stdDeviation="{soft * 0.5}"/></filter>')
        groups.append(f'  <g fill="{color}" opacity="{opacity}" filter="url(#w{i})">' +
                      ''.join(shape(*width) for shape in shapes) + '</g>' + NL)
    # Krümel nur auf dem Weg: abgeschnitten an der breitesten Schicht.
    lo, hi = layers[1][1]
    defs.append('<clipPath id="weg">' + ''.join(shape(lo + 4, hi - 4) for shape in shapes) + '</clipPath>')
    grain = ''.join(f'<circle cx="{rng.uniform(0, 128):.1f}" cy="{rng.uniform(0, 128):.1f}" r="{rng.uniform(0.5, 1.6):.1f}" '
                    f'fill="{rng.choice(crumbs)}" opacity="0.6"/>' for _ in range(260))
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">' + NL +
            f'  <!-- {comment} -->' + NL + f'  <defs>{"".join(defs)}</defs>' + NL + ''.join(groups) +
            f'  <g clip-path="url(#weg)">{grain}</g>' + NL + f'  {dots(spots, dot_color, rx, ry)}' + NL + '</svg>' + NL)


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
            f.write(tile(style, shapes, spots, sum(map(ord, style + name))))
print('ok', len(STYLES) * len(pieces))
