"""Zeichnet Figuren und Einrichtung als SVG: weich schattiert, mit Fell-, Blatt- und Halmstrichen und
ohne dicke Umrisse, damit sie zu den Bodentexturen passen. Licht kommt immer von links oben.

Aufruf aus dem Projektordner: `python tools/draw_art.py .`, danach `cargo run -p art_export`.
Trampelpfade und Dämme zeichnet `tools/draw_paths.py`."""
import math
import os
import random
import sys

root = sys.argv[1] if len(sys.argv) > 1 else '.'


def mix(a, b, t):
    """Mischt zwei Farben `#rrggbb`; t = 0 ist a, t = 1 ist b."""
    ca = [int(a[i:i + 2], 16) for i in (1, 3, 5)]
    cb = [int(b[i:i + 2], 16) for i in (1, 3, 5)]
    return '#' + ''.join(f'{round(x + (y - x) * t):02x}' for x, y in zip(ca, cb))


def light(c, t):
    return mix(c, '#ffffff', t)


def dark(c, t):
    return mix(c, '#000000', t)


def f(x):
    return f'{x:.1f}'.rstrip('0').rstrip('.')


class Svg:
    def __init__(self, w, h, comment, seed):
        self.w, self.h, self.comment = w, h, comment
        self.defs, self.body = [], []
        self.n = 0
        self.rng = random.Random(seed)

    def id(self, prefix):
        self.n += 1
        return f'{prefix}{self.n}'

    def add(self, markup):
        self.body.append(markup)

    def radial(self, cx, cy, r, stops):
        """Kugeliges Licht: `stops` als (Anteil, Farbe) vom Mittelpunkt nach außen."""
        name = self.id('r')
        s = ''.join(f'<stop offset="{o}" stop-color="{c}"/>' for o, c in stops)
        self.defs.append(f'<radialGradient id="{name}" gradientUnits="userSpaceOnUse" '
                         f'cx="{f(cx)}" cy="{f(cy)}" r="{f(r)}">{s}</radialGradient>')
        return f'url(#{name})'

    def linear(self, x1, y1, x2, y2, stops):
        name = self.id('l')
        s = ''.join(f'<stop offset="{o}" stop-color="{c}"' + (f' stop-opacity="{a}"' if a != 1 else '') + '/>'
                    for o, c, a in [(st + (1,))[:3] for st in stops])
        self.defs.append(f'<linearGradient id="{name}" gradientUnits="userSpaceOnUse" '
                         f'x1="{f(x1)}" y1="{f(y1)}" x2="{f(x2)}" y2="{f(y2)}">{s}</linearGradient>')
        return f'url(#{name})'

    def clip(self, shape):
        name = self.id('c')
        self.defs.append(f'<clipPath id="{name}">{shape}</clipPath>')
        return f'clip-path="url(#{name})"'

    def clip_d(self, d):
        return self.clip(f'<path d="{d}"/>')

    def rough(self, scale, freq=0.6, octaves=2):
        """Franst Kanten aus: Fell wird flauschig, Laub zerfällt in Blätter."""
        name = self.id('f')
        seed = self.rng.randrange(1000)
        self.defs.append(f'<filter id="{name}" x="-20%" y="-20%" width="140%" height="140%">'
                         f'<feTurbulence type="fractalNoise" baseFrequency="{freq}" numOctaves="{octaves}" seed="{seed}"/>'
                         f'<feDisplacementMap in="SourceGraphic" scale="{scale}" xChannelSelector="R" yChannelSelector="G"/>'
                         f'</filter>')
        return f'filter="url(#{name})"'

    def blur(self, sd):
        name = self.id('b')
        self.defs.append(f'<filter id="{name}" x="-50%" y="-50%" width="200%" height="200%">'
                         f'<feGaussianBlur stdDeviation="{sd}"/></filter>')
        return f'filter="url(#{name})"'

    def strokes(self, points, angle, length, width, colors, opacity, spread=0.35, curve=0.0):
        """Kurze Striche an `points` in Richtung `angle` (Grad): Fell, Halme, Rinde."""
        rng = self.rng
        out = []
        for x, y in points:
            a = math.radians(angle + rng.uniform(-1, 1) * spread * 57.3)
            l = length * rng.uniform(0.6, 1.3)
            dx, dy = math.cos(a) * l, math.sin(a) * l
            bx = -dy * curve * rng.uniform(0.5, 1.5)
            by = dx * curve * rng.uniform(0.5, 1.5)
            c = rng.choice(colors)
            out.append(f'<path d="M{f(x)} {f(y)} q{f(dx / 2 + bx)} {f(dy / 2 + by)} {f(dx)} {f(dy)}" '
                       f'stroke="{c}" stroke-width="{f(width * rng.uniform(0.7, 1.2))}" opacity="{opacity}"/>')
        return '<g fill="none" stroke-linecap="round">' + ''.join(out) + '</g>'

    def scatter(self, n, x0, y0, x1, y1):
        return [(self.rng.uniform(x0, x1), self.rng.uniform(y0, y1)) for _ in range(n)]

    def save(self, path):
        text = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w}" height="{self.h}" '
                f'viewBox="0 0 {self.w} {self.h}">\n  <!-- {self.comment} -->\n'
                f'  <defs>{"".join(self.defs)}</defs>\n  ' + '\n  '.join(self.body) + '\n</svg>\n')
        os.makedirs(os.path.dirname(f'{root}/art_src/{path}'), exist_ok=True)
        with open(f'{root}/art_src/{path}', 'w', encoding='utf-8') as file:
            file.write(text)


def furry(s, d, base, box, angle, density=0.5, length=5.0, fuzz=1.6, light_at=(0.38, 0.3), rim=0.28,
          hair=None, extra=''):
    """Eine fellige Form: kugelig schattiert, mit Haarstrichen und flauschigem Rand."""
    x0, y0, x1, y1 = box
    w, h = x1 - x0, y1 - y0
    cx, cy = x0 + w * light_at[0], y0 + h * light_at[1]
    fill = s.radial(cx, cy, max(w, h) * 0.8,
                    [(0, light(base, 0.35)), (0.45, base), (1, dark(base, rim))])
    clip = s.clip(f'<path d="{d}"/>')
    n = int(w * h * density / 40)
    hair = hair or [light(base, 0.55), dark(base, 0.3), dark(base, 0.15)]
    s.add(f'<g {s.rough(fuzz, 0.9)}><path d="{d}" fill="{fill}"/>'
          f'<g {clip}>{extra}{s.strokes(s.scatter(n, x0, y0, x1, y1), angle, length, 0.9, hair, 0.5)}</g></g>')


def glow(s, cx, cy, rx, ry, color, opacity, sd=None, clip=''):
    sd = sd or min(rx, ry) * 0.45
    return f'<g {clip}><ellipse cx="{f(cx)}" cy="{f(cy)}" rx="{f(rx)}" ry="{f(ry)}" fill="{color}" opacity="{opacity}" {s.blur(sd)}/></g>'


# ---------------------------------------------------------------------------------------------
# Kaninchen: hell gemalt, das Spiel färbt die Teile in der Fellfarbe ein.

FUR = '#ece6de'
FUR_HAIR = ['#ffffff', '#fbf8f3', '#a39a90', '#bdb4aa']


def rabbit():
    s = Svg(116, 92, 'Körper des Kaninchens von der Seite: hinten hoch und rund, Brust vorne, heller Bauch', 11)
    d = 'M8 62 C6 34 28 8 58 8 C84 8 104 22 110 46 C114 66 104 84 76 87 L34 87 C18 86 10 78 8 62 Z'
    belly = glow(s, 68, 86, 42, 13, '#ffffff', 0.75, 6, s.clip(f'<path d="{d}"/>'))
    back = glow(s, 50, 10, 46, 14, '#8f857a', 0.35, 8, s.clip(f'<path d="{d}"/>'))
    furry(s, d, FUR, (6, 8, 112, 88), 175, 1.6, 5.5, hair=FUR_HAIR, rim=0.42, extra=back + belly, light_at=(0.45, 0.25))
    s.save('figuren/kaninchen/koerper.svg')

    s = Svg(74, 42, 'Hinterlauf: kräftige Keule über dem langen Fuß, die Zehen zeigen nach vorne', 12)
    d = ('M5 31 C4 24 8 18 12 14 C16 6 28 2 40 4 C52 6 58 14 56 24 L64 28 C71 30 72 38 66 40 '
         'L14 40 C8 40 5 36 5 31 Z')
    toe = '<path d="M60 34 l6 1 M58 38 l7 0" stroke="#a39a90" stroke-width="0.9" opacity="0.6" fill="none"/>'
    shade = glow(s, 40, 31, 22, 4, '#7d746a', 0.35, 2.5, s.clip(f'<path d="{d}"/>'))
    furry(s, d, FUR, (4, 2, 72, 41), 185, 1.6, 4.5, hair=FUR_HAIR, rim=0.42, extra=shade + toe, light_at=(0.4, 0.2))
    s.save('figuren/kaninchen/bein_hinten.svg')

    s = Svg(66, 66, 'Kopf von der Seite: runder Schädel, Schnauze nach rechts, Nase, Maul und Schnurrhaare', 13)
    d = 'M5 31 C5 14 18 4 34 4 C48 4 57 13 61 25 C64 31 64 38 60 42 C56 50 47 56 37 58 C21 60 5 50 5 31 Z'
    c = s.clip(f'<path d="{d}"/>')
    cheek = glow(s, 40, 46, 16, 9, '#ffffff', 0.7, 4, c)
    muzzle = glow(s, 57, 40, 7, 6, '#ffffff', 0.8, 2.2, c)
    under = glow(s, 26, 58, 26, 6, '#7d746a', 0.3, 4, c)
    furry(s, d, FUR, (4, 3, 64, 60), 200, 1.5, 4, hair=FUR_HAIR, rim=0.42, extra=cheek + muzzle + under, light_at=(0.4, 0.25))
    s.add('<path d="M58.6 31.6 C60 30.4 62.6 30.6 63.4 32 C63 33.6 61.8 34.4 61 35.6 C60.2 34.2 59 33.2 58.6 31.6 Z" fill="#c98e88"/>')
    s.add('<path d="M61 35.6 L61 38.6 C60 40.4 58 40.6 56.6 39.8 M61 38.6 C61.6 40 62.6 40.4 63.4 40" '
          'fill="none" stroke="#8a6a64" stroke-width="0.9" stroke-linecap="round"/>')
    s.add('<g fill="none" stroke="#ffffff" stroke-width="0.5" opacity="0.75" stroke-linecap="round">'
          '<path d="M57 38 Q61 37 65.5 35.5"/><path d="M57 39.5 Q61 40 65.5 41"/><path d="M56 41 Q59 43 63 46"/></g>')
    s.save('figuren/kaninchen/kopf.svg')

    for name, shade_t in (('links', 0.0), ('rechts', 0.1)):
        s = Svg(28, 70, 'Ohr: lang, oben gerundet, innen zartrosa, mit dunklerer Spitze', 14 + len(name))
        base = dark(FUR, shade_t)
        d = 'M9 68 C5 50 3 30 5 16 C7 6 10.5 2 14 2 C17.5 2 21 6 23 16 C25 30 23 50 19 68 Z'
        c = s.clip(f'<path d="{d}"/>')
        inner_d = 'M11 63 C9 48 8.5 30 10 18 C11 11 12.6 8 14 8 C15.6 8 17.4 11 18.4 18 C19.8 30 19 48 17 63 Z'
        inner = (f'<path d="{inner_d}" fill="{s.linear(0, 8, 0, 64, [(0, "#e6aaa3"), (0.6, "#efc0b8"), (1, "#d8a49c")])}" '
                 f'opacity="{0.85 - shade_t}" {s.blur(0.8)}/>'
                 '<path d="M14 58 C13.6 44 13.8 28 14.2 14 M14 40 l-2.6 -6 M14.2 30 l2.4 -5" fill="none" '
                 'stroke="#c98e88" stroke-width="0.6" opacity="0.6"/>')
        tip = f'<path d="{d}" fill="none" stroke="{s.linear(0, 2, 0, 20, [(0, "#5a5048", 0.8), (1, "#5a5048", 0)])}" stroke-width="2.4"/>'
        furry(s, d, base, (3, 2, 25, 69), 270, 1.3, 3.5, 1.2, hair=FUR_HAIR, rim=0.42, extra=inner + tip, light_at=(0.35, 0.3))
        s.save(f'figuren/kaninchen/ohr_{name}.svg')

    s = Svg(34, 24, 'Vorderpfote: klein und länglich, mit angedeuteten Zehen', 16)
    d = 'M3 14 C3 8 10 6 18 7 C26 8 31 11 31 15 C31 19 26 20 18 20 L8 20 C5 20 3 18 3 14 Z'
    toes = '<path d="M25 17 l5 0.4 M24 13.6 l5.4 -0.2" stroke="#a39a90" stroke-width="0.8" fill="none" opacity="0.7"/>'
    furry(s, d, FUR, (3, 6, 31, 21), 0, 1.3, 3, 1.2, hair=FUR_HAIR, rim=0.42, extra=toes)
    s.save('figuren/kaninchen/pfote_vorne.svg')

    s = Svg(36, 36, 'Blume (Schwanz): ein flauschiger weißer Puschel', 17)
    fill = s.radial(14, 13, 16, [(0, '#ffffff'), (0.6, '#f6f2ec'), (1, '#cfc8be')])
    tufts = s.strokes([(18 + math.cos(a) * 9, 18 + math.sin(a) * 9) for a in [i * 0.42 for i in range(15)]],
                      0, 0, 1, ['#ffffff'], 0)
    rays = ''.join(
        f'<path d="M{f(18 + math.cos(a) * 8)} {f(18 + math.sin(a) * 8)} l{f(math.cos(a) * 6)} {f(math.sin(a) * 6)}" '
        f'stroke="{"#ffffff" if i % 3 else "#e4ded6"}" stroke-width="2.2" stroke-linecap="round"/>'
        for i, a in enumerate([j * 0.33 for j in range(19)]))
    s.add(f'<g {s.rough(3.2, 0.7)}>{rays}<circle cx="18" cy="18" r="11.5" fill="{fill}"/></g>{tufts}')
    s.save('figuren/kaninchen/schwanz.svg')

    s = Svg(14, 16, 'Offenes Auge: groß, dunkelbraun und glänzend, mit hellem Lidrand', 18)
    iris = s.radial(6.4, 7, 6, [(0, '#5a3418'), (0.55, '#2a160a'), (1, '#0c0603')])
    s.add('<ellipse cx="7" cy="8" rx="5.6" ry="6.2" fill="#f2ece4" opacity="0.65"/>')
    s.add(f'<ellipse cx="7" cy="8" rx="4.6" ry="5.2" fill="{iris}"/>')
    s.add('<ellipse cx="5.4" cy="6" rx="1.3" ry="1.1" fill="#ffffff" opacity="0.95"/>'
          '<circle cx="8.8" cy="10.4" r="0.6" fill="#ffffff" opacity="0.5"/>')
    s.save('figuren/kaninchen/auge_offen.svg')

    s = Svg(14, 16, 'Geschlossenes Auge: ein gebogener Lidstrich', 19)
    s.add('<path d="M2 7.4 Q7 11.6 12 7.4" fill="none" stroke="#2b1a0e" stroke-width="1.8" stroke-linecap="round"/>'
          '<path d="M2.6 6.6 Q7 9.6 11.4 6.6" fill="none" stroke="#f2ece4" stroke-width="1" opacity="0.6"/>')
    s.save('figuren/kaninchen/auge_zu.svg')

    s = Svg(128, 80, 'Ein schlaff herabhängendes Kaninchen, hell gemalt, damit das Spiel es in der Fellfarbe einfärbt', 20)
    d = 'M20 40 C20 24 44 18 66 22 C88 26 104 32 106 44 C108 56 92 62 70 62 C46 62 20 56 20 40 Z'
    for leg in ('M34 56 C32 64 30 70 26 76 L31 77 C36 71 40 64 42 58 Z', 'M80 58 C83 66 88 72 92 76 L96 74 C93 69 90 64 88 58 Z'):
        furry(s, leg, FUR, (24, 54, 98, 78), 90, 1.2, 3, 1.2, hair=FUR_HAIR)
    furry(s, d, FUR, (18, 20, 108, 63), 180, 1.4, 5, hair=FUR_HAIR, rim=0.42,
          extra=glow(s, 64, 62, 40, 8, '#ffffff', 0.6, 4, s.clip(f'<path d="{d}"/>')))
    s.add(f'<g {s.rough(2.6, 0.7)}><circle cx="17" cy="42" r="7" fill="{s.radial(15, 40, 8, [(0, "#ffffff"), (1, "#d6d0c8")])}"/></g>')
    for ear, t in (('M102 26 C110 36 110 52 104 64 C100 58 98 44 98 32 Z', 0.1),
                   ('M112 30 C118 40 120 56 116 70 C112 66 108 52 106 40 Z', 0.0)):
        furry(s, ear, dark(FUR, t), (96, 24, 121, 71), 90, 1.2, 3, 1.2, hair=FUR_HAIR)
    furry(s, 'M94 38 C94 30 100 25 108 25 C117 25 123 32 123 39 C123 46 116 51 108 51 C100 51 94 46 94 38 Z',
          FUR, (93, 25, 124, 52), 200, 1.4, 3.5, hair=FUR_HAIR)
    s.add('<path d="M107 37 q4 3 8 0" fill="none" stroke="#2b1a0e" stroke-width="1.6" stroke-linecap="round"/>'
          '<circle cx="122" cy="42" r="1.8" fill="#c98e88"/>')
    s.save('figuren/beute/getragen.svg')


# ---------------------------------------------------------------------------------------------
# Fuchs: rotbraun, weiße Kehle und Schwanzspitze, schwarze Läufe und Ohrrückseiten.

FOX = '#c4642c'
FOX_HAIR = ['#e89a5c', '#8e3f18', '#d9783a', '#f0b27a']
WHITE = '#f3eee6'
WHITE_HAIR = ['#ffffff', '#d8d0c4']
SOCK = '#2a1d16'


def fox():
    s = Svg(192, 88, 'Körper des Fuchses: langer, schlanker Rumpf, dunklerer Rücken, weiße Brust vorne unten', 21)
    d = 'M8 42 C10 24 40 12 90 12 C138 12 174 18 184 38 C190 54 178 70 150 75 C110 82 60 80 30 72 C14 68 6 56 8 42 Z'
    c = s.clip(f'<path d="{d}"/>')
    chest_d = 'M118 74 Q160 70 182 44 Q190 62 164 76 Q140 84 118 74 Z'
    back = glow(s, 96, 12, 80, 12, '#5a2a10', 0.45, 7, c)
    belly = glow(s, 80, 80, 60, 10, '#e8b088', 0.6, 6, c)
    chest = (f'<g {c}><path d="{chest_d}" fill="{WHITE}" {s.blur(2.2)}/>'
             f'{s.strokes(s.scatter(40, 120, 52, 186, 80), 160, 4, 0.9, WHITE_HAIR, 0.6)}</g>')
    furry(s, d, FOX, (6, 12, 188, 80), 182, 1.4, 7, hair=FOX_HAIR, extra=back + belly + chest, light_at=(0.5, 0.2))
    s.save('figuren/fuchs/koerper.svg')

    s = Svg(106, 72, 'Kopf des Fuchses: spitze Schnauze nach rechts, weiße Wangen, schwarze Nase', 22)
    d = 'M8 34 C8 16 22 6 40 6 C56 6 66 14 74 22 L97 35 C103 38 104 44 98 47 L70 54 C58 62 40 66 26 60 C14 56 8 46 8 34 Z'
    c = s.clip(f'<path d="{d}"/>')
    cheek_d = 'M12 44 C24 50 42 50 58 46 L100 44 C96 50 80 54 66 58 C50 66 26 66 12 44 Z'
    cheek = (f'<g {c}><path d="{cheek_d}" fill="{WHITE}" {s.blur(1.6)}/>'
             f'{s.strokes(s.scatter(40, 14, 44, 98, 64), 190, 3.5, 0.8, WHITE_HAIR, 0.6)}</g>')
    streak = f'<g {c}><path d="M56 30 L96 40" stroke="#6a2c10" stroke-width="5" opacity="0.4" {s.blur(2)}/></g>'
    furry(s, d, FOX, (7, 5, 104, 64), 195, 1.4, 4.5, hair=FOX_HAIR, extra=streak + cheek, light_at=(0.35, 0.2))
    s.add(f'<ellipse cx="99" cy="41" rx="4.2" ry="3.6" fill="{s.radial(98, 39.6, 4.4, [(0, "#4a3a32"), (1, "#0d0806")])}"/>'
          '<path d="M98 45 C90 49 80 50 72 50" fill="none" stroke="#2a1a12" stroke-width="1.1" opacity="0.7"/>')
    s.save('figuren/fuchs/kopf.svg')

    for name, t in (('nah', 0.0), ('fern', 0.18)):
        s = Svg(34, 46, 'Ohr des Fuchses: spitz, außen rot mit schwarzer Spitze, innen weißes Fell', 23 + t * 10)
        d = 'M4 44 C6 30 11 13 16 3 C17 2 18 2 19 3 C23 13 28 30 30 44 Z'
        c = s.clip(f'<path d="{d}"/>')
        tip = f'<g {c}><rect x="0" y="0" width="34" height="18" fill="{SOCK}" {s.blur(2.5)}/></g>'
        inner = (f'<g {c}><path d="M10 42 C12 32 15 22 17.5 14 C20 22 22 32 24 42 Z" fill="{dark(WHITE, t * 2)}" {s.blur(1)}/>'
                 f'{s.strokes(s.scatter(14, 11, 20, 23, 42), 280, 4, 0.7, WHITE_HAIR, 0.7)}</g>')
        furry(s, d, dark(FOX, t), (4, 2, 30, 45), 270, 1.2, 3.5, 1.2, hair=FOX_HAIR, extra=tip + inner)
        s.save(f'figuren/fuchs/ohr_{name}.svg')

        s = Svg(28, 68, 'Lauf des Fuchses: oben rotbraun, unten schwarz wie ein Strumpf', 25 + t * 10)
        d = 'M6 4 C8 1 20 1 22 4 L21 54 C21 60 24 62 23 65 C22 67 6 67 5 65 C4 62 7 60 7 54 Z'
        c = s.clip(f'<path d="{d}"/>')
        sock = f'<g {c}><rect x="0" y="36" width="28" height="34" fill="{dark(SOCK, t)}" {s.blur(3)}/></g>'
        furry(s, d, dark(FOX, t), (4, 1, 24, 67), 90, 1.2, 3.5, 1.2, hair=FOX_HAIR, extra=sock)
        s.save(f'figuren/fuchs/bein_{name}.svg')

    s = Svg(132, 56, 'Lunte des Fuchses: buschig, nach hinten dunkler, mit weißer Spitze', 27)
    d = 'M130 27 C104 5 50 3 15 16 C3 21 2 35 14 40 C50 54 104 50 130 31 Z'
    c = s.clip(f'<path d="{d}"/>')
    tip = (f'<g {c}><rect x="-10" y="0" width="44" height="56" fill="{WHITE}" {s.blur(4)}/>'
           f'<rect x="34" y="0" width="16" height="56" fill="#5a2a10" opacity="0.45" {s.blur(5)}/></g>')
    furry(s, d, FOX, (2, 3, 132, 52), 185, 1.6, 8, 3.5, hair=FOX_HAIR + WHITE_HAIR[:1], extra=tip, light_at=(0.6, 0.2))
    s.save('figuren/fuchs/schwanz.svg')

    s = Svg(16, 12, 'Offenes Fuchsauge: bernsteingelb, mandelförmig, mit länglicher Pupille', 28)
    iris = s.radial(9, 5.4, 6, [(0, '#f0b848'), (0.7, '#c47a1c'), (1, '#6a3a0c')])
    s.add(f'<path d="M1.6 7 Q8 0.4 14.4 5.4 Q9 11.4 1.6 7 Z" fill="{iris}" stroke="#1a100a" stroke-width="1.1"/>'
          '<ellipse cx="9" cy="6" rx="1.1" ry="2.6" fill="#120a05"/><circle cx="7.4" cy="4.4" r="0.9" fill="#ffffff" opacity="0.9"/>')
    s.save('figuren/fuchs/auge_offen.svg')

    s = Svg(16, 12, 'Geschlossenes Fuchsauge: ein schmaler dunkler Strich', 29)
    s.add('<path d="M2 6.4 Q8 9.4 14 5.4" fill="none" stroke="#1a100a" stroke-width="1.6" stroke-linecap="round"/>')
    s.save('figuren/fuchs/auge_zu.svg')


# ---------------------------------------------------------------------------------------------
# Marder und Bussard: je ein Bild.

def marten():
    """Steinmarder in Teilen wie der Fuchs, damit er im Galopp springen kann."""
    brown = '#7a5434'
    hair = ['#a07a56', '#3a2414', '#6e4c32', '#b08a62']
    bib = '#efe2c2'

    s = Svg(150, 56, 'Körper des Marders: lang und schlank, Rücken dunkler, heller Kehlfleck an der Brust', 31)
    d = 'M6 34 C6 16 24 4 54 3 C84 2 114 8 132 14 C146 19 148 36 140 44 C128 52 100 50 76 50 C50 52 22 54 10 46 C6 42 6 38 6 34 Z'
    c = s.clip_d(d)
    back = glow(s, 70, 8, 64, 8, '#2a180c', 0.45, 5, c)
    chest = f'<g {c}><path d="M104 52 C120 50 136 44 146 30 C150 46 140 56 124 58 C114 58 106 56 104 52 Z" fill="{bib}" {s.blur(1.6)}/></g>'
    furry(s, d, brown, (5, 6, 148, 52), 182, 1.4, 5, hair=hair, extra=back + chest, light_at=(0.5, 0.2))
    s.save('figuren/marder/koerper.svg')

    s = Svg(64, 46, 'Kopf des Marders: runde Ohren mit hellem Rand, spitze Schnauze, heller Kehlfleck', 33)
    for x, y in ((18, 9), (29, 7)):
        s.add(f'<circle cx="{x}" cy="{y}" r="6.5" fill="{bib}"/>')
        s.add(f'<circle cx="{x + 0.4}" cy="{y + 0.8}" r="5" fill="{dark(brown, 0.2)}"/>')
    d = 'M4 26 C4 14 14 6 28 6 C40 6 48 12 54 18 L60 22 C63 24 63 28 60 30 L50 34 C42 40 30 42 20 40 C10 38 4 34 4 26 Z'
    c = s.clip_d(d)
    throat = f'<g {c}><path d="M4 34 C18 42 34 42 50 34 L52 46 L4 46 Z" fill="{bib}" {s.blur(1.4)}/></g>'
    furry(s, d, brown, (3, 5, 63, 42), 195, 1.4, 3.5, hair=hair, extra=throat, light_at=(0.35, 0.2))
    s.add(f'<ellipse cx="60.4" cy="25.6" rx="2.8" ry="2.4" fill="{s.radial(59.6, 24.8, 3, [(0, "#4a3a32"), (1, "#0d0806")])}"/>'
          '<path d="M59 30 C55 33 50 34 46 34" fill="none" stroke="#1a100a" stroke-width="0.9" opacity="0.7"/>'
          '<g fill="none" stroke="#e8dcc8" stroke-width="0.4" opacity="0.7"><path d="M55 28 L63.5 26"/><path d="M55 29 L63.5 31"/></g>')
    s.save('figuren/marder/kopf.svg')

    for name, t in (('nah', 0.0), ('fern', 0.2)):
        s = Svg(22, 40, 'Lauf des Marders: kurz und kräftig, unten fast schwarz', 34 + int(t * 10))
        d = 'M5 4 C5 1 17 1 17 4 L16 30 C16 34 20 35 20 37 C20 39.4 4 39.4 3 37 C3 35 6 34 6 30 Z'
        paw = f'<g {s.clip_d(d)}><rect x="0" y="22" width="22" height="20" fill="#1e140c" opacity="0.8" {s.blur(3)}/></g>'
        furry(s, d, dark(brown, 0.25 + t), (3, 1, 20, 39), 90, 1.2, 3, 1.2, hair=hair, extra=paw)
        s.save(f'figuren/marder/bein_{name}.svg')

    s = Svg(96, 40, 'Schwanz des Marders: lang und buschig, zur Spitze hin dunkler', 35)
    d = 'M95 18 C80 8 50 4 24 8 C8 11 2 18 4 24 C6 31 18 34 34 32 C60 30 82 28 95 24 Z'
    tip = f'<g {s.clip_d(d)}><rect x="-10" y="0" width="50" height="40" fill="#1e140c" opacity="0.55" {s.blur(8)}/></g>'
    furry(s, d, dark(brown, 0.1), (2, 4, 96, 34), 185, 1.6, 7, 3.2, hair=hair, extra=tip, light_at=(0.6, 0.2))
    s.save('figuren/marder/schwanz.svg')

    s = Svg(10, 8, 'Offenes Marderauge: rund, schwarz und glänzend', 36)
    s.add(f'<circle cx="5" cy="4" r="3" fill="{s.radial(4.4, 3.4, 3.2, [(0, "#3a2a20"), (1, "#050302")])}"/>'
          '<circle cx="4" cy="3" r="0.9" fill="#ffffff" opacity="0.9"/>')
    s.save('figuren/marder/auge_offen.svg')

    s = Svg(10, 8, 'Geschlossenes Marderauge: ein kurzer Strich', 37)
    s.add('<path d="M2 4 Q5 6 8 4" fill="none" stroke="#050302" stroke-width="1.3" stroke-linecap="round"/>')
    s.save('figuren/marder/auge_zu.svg')

def buzzard():
    s = Svg(192, 96, 'Mäusebussard von schräg unten: breite Schwingen mit dunklen Fingern, gebänderter Bauch', 32)
    wing = ('M96 40 C78 30 56 22 30 22 C18 22 8 26 2 34 C12 34 18 38 22 42 C14 44 10 48 8 54 C20 50 30 50 40 54 '
            'C34 58 32 62 32 66 C48 58 70 54 88 54 Z')
    for mirror in (False, True):
        t = 'transform="translate(192 0) scale(-1 1)"' if mirror else ''
        c = s.clip(f'<path d="{wing}"/>')
        tips = f'<g {c}><rect x="-4" y="16" width="34" height="56" fill="#2a1a10" opacity="0.8" {s.blur(4)}/></g>'
        band = f'<g {c}><path d="M30 30 C54 32 74 38 92 46" stroke="#e2cfa8" stroke-width="7" fill="none" opacity="0.7" {s.blur(2.5)}/></g>'
        bars = f'<g {c} fill="none" stroke="#3a2414" stroke-width="1" opacity="0.45">' + ''.join(
            f'<path d="M{x} {24 + (x - 20) * 0.08} q4 16 {x / 12} 34"/>' for x in range(26, 92, 7)) + '</g>'
        s.add(f'<g {t}>')
        furry(s, wing, '#6e4a2e', (2, 22, 96, 66), 170, 1.2, 5, 1.2, hair=['#8a603c', '#3e2716'], extra=band + bars + tips)
        s.add('</g>')
    tail = 'M84 52 L80 84 C88 90 104 90 112 84 L108 52 Z'
    bars = ''.join(f'<path d="M80 {y} H112" stroke="#4a2e1a" stroke-width="1.6" opacity="0.6"/>' for y in (62, 70, 78))
    furry(s, tail, '#9a7048', (80, 52, 112, 90), 90, 1.2, 4, 1.2, hair=['#c9a77a', '#5a3a22'], extra=bars)
    body = 'M81 46 C81 34 88 28 96 28 C104 28 111 34 111 46 C111 58 104 64 96 64 C88 64 81 58 81 46 Z'
    bands = ''.join(f'<path d="M84 {y} q12 4 24 0" stroke="#8a5a34" stroke-width="2" fill="none" opacity="0.7"/>'
                    for y in (42, 48, 54, 59))
    furry(s, body, '#eadcbc', (81, 28, 111, 64), 90, 1.2, 3, 1.2, hair=['#ffffff', '#b89a70'], extra=bands)
    furry(s, 'M86 26 C86 20 90 16 96 16 C102 16 106 20 106 26 C106 32 102 36 96 36 C90 36 86 32 86 26 Z',
          '#7a5233', (86, 16, 106, 36), 90, 1.2, 2.5, 1.2, hair=['#a07a50', '#3e2716'])
    s.add('<path d="M93 31 L96 38 L99 31 Z" fill="#e8b830"/><path d="M95 35 L96 38 L97 35 Z" fill="#2a1a10"/>'
          '<circle cx="91.4" cy="24" r="1.8" fill="#1d120a"/><circle cx="100.6" cy="24" r="1.8" fill="#1d120a"/>')
    s.save('figuren/bussard/bussard.svg')


# ---------------------------------------------------------------------------------------------
# Pflanzen: Laub aus vielen kleinen Blattbüscheln, oben links heller, unten rechts im Schatten.

def foliage(s, clumps, base, size, light_from=(0.3, 0.2), box=None, leaves=True):
    """Ein Laubdach aus Büscheln (x, y, r). Jedes Büschel ist kugelig schattiert und ausgefranst."""
    x0, y0, x1, y1 = box
    rough = s.rough(size * 0.9, 0.18, 2)
    parts = []
    for x, y, r in clumps:
        # Wie hell ein Büschel ist, hängt davon ab, wie weit oben links es liegt.
        t = ((x - x0) / (x1 - x0) - light_from[0]) * 0.5 + ((y - y0) / (y1 - y0) - light_from[1]) * 0.8
        c = mix(light(base, 0.28), dark(base, 0.45), max(0.0, min(1.0, t)))
        g = s.radial(x - r * 0.35, y - r * 0.4, r * 1.3, [(0, light(c, 0.3)), (0.55, c), (1, dark(c, 0.35))])
        parts.append(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(r)}" fill="{g}"/>')
    s.add(f'<g {rough}>' + ''.join(parts) + '</g>')
    if leaves:
        pts = s.scatter(int((x1 - x0) * (y1 - y0) / 30), x0, y0, x1, y1)
        inside = [(x, y) for x, y in pts if any((x - cx) ** 2 + (y - cy) ** 2 < (r * 0.85) ** 2 for cx, cy, r in clumps)]
        out = []
        for x, y in inside:
            t = (x - x0) / (x1 - x0) * 0.5 + (y - y0) / (y1 - y0) * 0.8
            c = light(base, 0.45) if s.rng.random() > t else dark(base, 0.5)
            a = s.rng.uniform(0, 180)
            out.append(f'<ellipse cx="{f(x)}" cy="{f(y)}" rx="{f(size * 0.32)}" ry="{f(size * 0.16)}" '
                       f'transform="rotate({f(a)} {f(x)} {f(y)})" fill="{c}" opacity="0.55"/>')
        s.add('<g>' + ''.join(out) + '</g>')


def ring(s, n, cx, cy, rx, ry, r0, r1, jitter):
    out = []
    for i in range(n):
        a = i / n * math.tau + s.rng.uniform(-0.2, 0.2)
        k = math.sqrt(s.rng.uniform(0.0, 1.0))
        out.append((cx + math.cos(a) * rx * k + s.rng.uniform(-jitter, jitter),
                    cy + math.sin(a) * ry * k + s.rng.uniform(-jitter, jitter), s.rng.uniform(r0, r1)))
    return out


def plants():
    s = Svg(160, 220, 'Laubbaum: Stamm mit Borke und Wurzelanlauf, dichte Krone aus vielen Blattbüscheln', 41)
    trunk = 'M66 218 C70 196 71 160 68 120 L92 120 C89 160 90 196 94 218 C86 214 74 214 66 218 Z'
    bark = s.linear(66, 0, 94, 0, [(0, '#3e2a18'), (0.35, '#7a5a3a'), (0.6, '#5e4228'), (1, '#2e1e10')])
    s.add(f'<ellipse cx="80" cy="216" rx="34" ry="5" fill="#1e1408" opacity="0.35" {s.blur(3)}/>')
    s.add(f'<path d="{trunk}" fill="{bark}"/>')
    s.add(f'<g {s.clip_d(trunk)}>' +
          s.strokes(s.scatter(70, 64, 120, 96, 218), 90, 9, 1.2, ['#2a1a0c', '#8a6a48', '#3a2614'], 0.6, 0.12) + '</g>')
    s.add('<path d="M58 218 C62 210 68 208 72 210 L74 218 Z M88 218 L88 210 C94 208 100 212 102 218 Z" fill="#4a3220"/>')
    for d in ('M78 136 C68 126 56 120 44 120', 'M84 128 C94 118 106 114 118 114'):
        s.add(f'<path d="{d}" stroke="#4a3220" stroke-width="5" fill="none" stroke-linecap="round"/>')
    base = '#4a8030'
    crown = ring(s, 46, 80, 72, 62, 52, 14, 22, 6) + [(80, 72, 30), (60, 92, 26), (100, 92, 26)]
    crown.sort(key=lambda c: -c[1] * 0.4 + c[0] * 0.0)
    foliage(s, crown, base, 9, box=(10, 10, 150, 130))
    s.add(glow(s, 80, 124, 50, 8, '#1a2e10', 0.35, 5))
    s.save('einrichtung/baum.svg')

    s = Svg(128, 96, 'Hecke: dichter Schlehenbusch mit dunklen Beeren, unten im Schatten', 42)
    s.add(f'<ellipse cx="64" cy="92" rx="60" ry="5" fill="#1a1408" opacity="0.4" {s.blur(3)}/>')
    s.add('<path d="M40 94 L44 70 M66 94 L64 66 M90 94 L86 72" stroke="#3a2614" stroke-width="3" stroke-linecap="round"/>')
    clumps = [(16 + i * 12 + s.rng.uniform(-3, 3), 70 + s.rng.uniform(-6, 8), s.rng.uniform(13, 17)) for i in range(9)]
    clumps += [(26 + i * 13 + s.rng.uniform(-3, 3), 44 + s.rng.uniform(-6, 6), s.rng.uniform(13, 17)) for i in range(7)]
    clumps += [(46 + i * 12 + s.rng.uniform(-3, 3), 24 + s.rng.uniform(-4, 4), s.rng.uniform(11, 14)) for i in range(4)]
    clumps.sort(key=lambda c: c[1])
    foliage(s, clumps, '#3a7028', 6, box=(2, 8, 126, 90))
    berries = []
    for x, y in s.scatter(9, 18, 30, 110, 80):
        berries.append(f'<circle cx="{f(x)}" cy="{f(y)}" r="2.6" fill="{s.radial(x - 0.8, y - 0.9, 3, [(0, "#8a86b8"), (0.5, "#3a2e5a"), (1, "#1a1028")])}"/>')
    s.add('<g>' + ''.join(berries) + '</g>')
    s.save('einrichtung/hecke.svg')

    s = Svg(128, 128, 'Kleefeld von oben: ein dichter Teppich aus Kleeblättern mit hellem Winkel und weißen Kleeblüten', 43)
    # Bis an die Kanten, damit benachbarte Kleefelder einen durchgehenden Teppich bilden.
    leaves = []
    for x, y in s.scatter(150, 4, 4, 124, 124):
        r = s.rng.uniform(4.5, 7)
        rot = s.rng.uniform(0, 120)
        c = mix('#62a84c', '#2e6a26', s.rng.random())
        g = ''
        for k in range(3):
            a = rot + k * 120
            g += (f'<g transform="rotate({f(a)} {f(x)} {f(y)})"><ellipse cx="{f(x)}" cy="{f(y - r * 0.55)}" rx="{f(r * 0.5)}" ry="{f(r * 0.55)}" '
                  f'fill="{c}"/><path d="M{f(x - r * 0.25)} {f(y - r * 0.7)} L{f(x)} {f(y - r * 0.45)} L{f(x + r * 0.25)} {f(y - r * 0.7)}" '
                  f'stroke="#c4e4a8" stroke-width="0.8" fill="none" opacity="0.6"/></g>')
        leaves.append(f'<ellipse cx="{f(x + 1.5)}" cy="{f(y + 2)}" rx="{f(r)}" ry="{f(r * 0.9)}" fill="#1e3a14" opacity="0.3"/>' + g)
    s.add('<g>' + ''.join(leaves) + '</g>')
    for x, y in s.scatter(5, 16, 16, 112, 112):
        petals = ''.join(f'<path d="M{f(x)} {f(y)} l{f(math.cos(i * 0.52) * 4.2)} {f(math.sin(i * 0.52) * 4.2)}" '
                         f'stroke="{"#ffffff" if i % 2 else "#f0e4e0"}" stroke-width="1.5" stroke-linecap="round"/>'
                         for i in range(12))
        s.add(f'<circle cx="{f(x + 1)}" cy="{f(y + 1.5)}" r="5" fill="#1e3a14" opacity="0.3" {s.blur(1.2)}/>'
              f'<g>{petals}</g><circle cx="{f(x)}" cy="{f(y)}" r="1.6" fill="#e8dcc0"/>')
    s.save('einrichtung/klee.svg')

    s = Svg(128, 128, 'Kräuterbeet von oben: Thymian- und Salbeipolster mit lila Blüten', 44)
    for cx, cy, r, base in ((40, 42, 26, '#5e9442'), (90, 40, 22, '#4e8a3a'), (36, 92, 22, '#4e8a3a'), (88, 90, 28, '#5e9442')):
        clumps = ring(s, 14, cx, cy, r * 0.75, r * 0.75, r * 0.25, r * 0.4, 2)
        s.add(f'<circle cx="{cx + 2}" cy="{cy + 3}" r="{r}" fill="#1e3014" opacity="0.4" {s.blur(3)}/>')
        foliage(s, clumps, base, 4, box=(cx - r, cy - r, cx + r, cy + r))
    for x, y in s.scatter(22, 16, 16, 112, 112):
        s.add(f'<circle cx="{f(x)}" cy="{f(y)}" r="2.2" fill="{s.radial(x - 0.6, y - 0.6, 2.6, [(0, "#d8b8f0"), (1, "#7a4f99")])}"/>')
    s.save('einrichtung/kraeuter.svg')

    s = Svg(128, 128, 'Möhrenbeet von oben: lockere Erde in Reihen, gefiedertes Möhrengrün, orange Möhrenköpfe', 45)
    soil = s.radial(56, 50, 80, [(0, '#7a5434'), (1, '#4e3220')])
    s.add(f'<g {s.rough(4, 0.12)}><rect x="10" y="12" width="108" height="104" rx="14" fill="{soil}"/></g>')
    bed = s.clip('<rect x="10" y="12" width="108" height="104" rx="14"/>')
    s.add(f'<g {bed}>' +
          ''.join(f'<path d="M10 {y} H118" stroke="#3a2414" stroke-width="7" opacity="0.5" {s.blur(2)}/>'
                  f'<path d="M10 {y - 6} H118" stroke="#9a7450" stroke-width="3" opacity="0.4" {s.blur(1.5)}/>' for y in (40, 66, 92)) +
          ''.join(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(s.rng.uniform(0.6, 1.6))}" fill="{s.rng.choice(["#9a7450", "#3a2414"])}" opacity="0.7"/>'
                  for x, y in s.scatter(140, 10, 12, 118, 116)) + '</g>')
    for y in (34, 60, 86):
        for x in range(26, 112, 17):
            x += s.rng.uniform(-3, 3)
            s.add(f'<circle cx="{f(x)}" cy="{f(y + 3)}" r="3.4" fill="{s.radial(x - 1, y + 2, 4, [(0, "#f8a850"), (1, "#c45a14")])}"/>')
            fronds = ''
            for k in range(6):
                a = math.radians(-90 + (k - 2.5) * 26 + s.rng.uniform(-8, 8))
                l = s.rng.uniform(10, 15)
                ex, ey = x + math.cos(a) * l, y + math.sin(a) * l * 0.8
                fronds += f'<path d="M{f(x)} {f(y)} L{f(ex)} {f(ey)}" stroke="#3a7a2c" stroke-width="1"/>'
                for j in range(1, 5):
                    px, py = x + (ex - x) * j / 5, y + (ey - y) * j / 5
                    fronds += f'<circle cx="{f(px)}" cy="{f(py)}" r="{f(2.4 - j * 0.3)}" fill="{mix("#5aa442", "#3a7a2c", s.rng.random())}"/>'
            s.add(f'<g {s.rough(1.6, 0.8)}>{fronds}</g>')
    s.save('einrichtung/moehren.svg')


# ---------------------------------------------------------------------------------------------
# Futter, Heu und Erde.

def grass_heap(s, d, box, carrots=(), flower=None, seed_shift=0):
    x0, y0, x1, y1 = box
    fill = s.radial(x0 + (x1 - x0) * 0.4, y0 + (y1 - y0) * 0.2, (x1 - x0) * 0.7,
                    [(0, '#a8cc70'), (0.5, '#6e9a42'), (1, '#3e5e24')])
    s.add(f'<ellipse cx="{f((x0 + x1) / 2)}" cy="{f(y1 - 2)}" rx="{f((x1 - x0) * 0.52)}" ry="4" fill="#1a1408" opacity="0.35" {s.blur(2.5)}/>')
    s.add(f'<g {s.rough(3, 0.5)}><path d="{d}" fill="{fill}"/></g>')
    blades = s.strokes(s.scatter(int((x1 - x0) * (y1 - y0) / 14), x0 + 4, y0 + 4, x1 - 4, y1 - 2), -90, 9, 1.3,
                       ['#c4e08a', '#8ab858', '#4e7a2c', '#a8a860', '#5e8a34'], 0.85, 0.9, 0.25)
    s.add(f'<g {s.clip_d(d)}>{blades}</g>')
    for cx, cy, rot in carrots:
        g = s.linear(-10, 0, 12, 0, [(0, '#c45a14'), (0.5, '#f4a050'), (1, '#e07a2a')])
        s.add(f'<g transform="translate({cx} {cy}) rotate({rot})">'
              f'<path d="M-12 -4 C-4 -6 6 -3 14 0 C6 3 -4 6 -12 4 C-14 2 -14 -2 -12 -4 Z" fill="{g}"/>'
              '<path d="M-6 -3 l1 2 M0 -2.5 l1 2 M6 -1.4 l1 1.6" stroke="#a84a14" stroke-width="0.8" fill="none"/>'
              '<path d="M-12 0 l-6 -5 M-12 0 l-8 0 M-12 0 l-6 5" stroke="#4e8a30" stroke-width="1.6" stroke-linecap="round"/></g>')
    if flower:
        fx, fy = flower
        petals = ''.join(f'<path d="M{fx} {fy} l{f(math.cos(i * 0.5) * 6)} {f(math.sin(i * 0.5) * 6)}" stroke="#f4cf45" stroke-width="1.6"/>'
                         for i in range(13))
        s.add(f'<g {s.rough(1, 0.9)} stroke-linecap="round">{petals}<circle cx="{fx}" cy="{fy}" r="2.6" fill="#e8a820"/></g>')


def stuff():
    s = Svg(80, 56, 'Kleiner Futterhaufen: ein Büschel frisch gerupftes Gras', 51)
    grass_heap(s, 'M6 52 C10 30 26 14 40 14 C54 14 70 30 74 52 Z', (6, 14, 74, 54))
    s.save('einrichtung/futter_klein.svg')

    s = Svg(104, 72, 'Mittlerer Futterhaufen: Gras und eine Möhre', 52)
    grass_heap(s, 'M4 68 C8 40 26 18 46 18 C58 18 66 28 70 34 C78 30 92 40 100 68 Z', (4, 18, 100, 70), [(80, 50, -30)])
    s.save('einrichtung/futter_mittel.svg')

    s = Svg(124, 88, 'Voller Futterhaufen: ein großer Berg Gras mit Möhren und einer Löwenzahnblüte', 53)
    grass_heap(s, 'M4 84 C8 52 30 22 60 18 C86 16 108 40 120 84 Z', (4, 16, 120, 86),
               [(96, 60, -28), (26, 66, 200)], (60, 36))
    s.save('einrichtung/futter_voll.svg')

    s = Svg(112, 72, 'Heunest von oben: ein Ring aus weichem Heu mit einer Mulde zum Kuscheln', 54)
    s.add(f'<ellipse cx="57" cy="40" rx="52" ry="29" fill="#2a1a08" opacity="0.35" {s.blur(3)}/>')
    s.add(f'<g {s.rough(4, 0.4)}><ellipse cx="56" cy="37" rx="50" ry="28" fill="{s.radial(46, 26, 56, [(0, "#f0d88a"), (0.6, "#d4b060"), (1, "#9a7a34")])}"/></g>')
    s.add(f'<ellipse cx="57" cy="38" rx="28" ry="13" fill="{s.radial(60, 42, 30, [(0, "#8a6a2c"), (1, "#b8963c")])}" {s.blur(2)}/>')
    pts = []
    for _ in range(170):
        a = s.rng.uniform(0, math.tau)
        k = s.rng.uniform(0.55, 1.0)
        pts.append((56 + math.cos(a) * 46 * k, 37 + math.sin(a) * 25 * k, math.degrees(a) + 90))
    straws = ''.join(s.strokes([(x, y)], a, 12, 1.2, ['#f8e8a8', '#c9a446', '#e3c46a', '#a8862f'], 0.9, 0.5, 0.2)
                     for x, y, a in pts)
    s.add(straws)
    s.save('einrichtung/heunest.svg')

    s = Svg(96, 64, 'Fluchtloch von oben: ein Ring aus ausgeworfener Erde um ein dunkles Loch', 55)
    # Nur ein schmaler Wall aus frischer Erde, nach außen weich, damit Wiese, Sand oder Waldboden drumherum durchscheinen.
    s.add(f'<ellipse cx="48" cy="33" rx="40" ry="24" fill="#6e4c2e" opacity="0.45" {s.blur(4)}/>')
    s.add(f'<g {s.rough(5, 0.25)}><ellipse cx="48" cy="32" rx="35" ry="21" fill="{s.radial(40, 22, 40, [(0, "#a07a50"), (0.6, "#7a5634"), (1, "#5a3c22")])}"/></g>')
    s.add(''.join(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(s.rng.uniform(0.8, 2.4))}" fill="{s.rng.choice(["#b89068", "#4e3220", "#8a8478"])}" opacity="0.8"/>'
                  for x, y in s.scatter(50, 8, 8, 88, 56) if ((x - 48) / 34) ** 2 + ((y - 32) / 20) ** 2 < 1 and ((x - 48) / 28) ** 2 + ((y - 32) / 16) ** 2 > 1))
    s.add(f'<ellipse cx="48" cy="33" rx="27" ry="16" fill="{s.radial(48, 38, 26, [(0, "#050302"), (0.7, "#1e1208"), (1, "#3e2814")])}" {s.blur(1.5)}/>')
    s.add(f'<path d="M24 28 C30 20 66 20 72 28" stroke="#c49a6c" stroke-width="2" fill="none" opacity="0.5" {s.blur(1)}/>')
    s.save('einrichtung/fluchtloch.svg')

    s = Svg(128, 104, 'Ein Stück alter Steinwall: aufgeschichtete Feldsteine mit Moos und dunklen Ritzen', 56)
    s.add(f'<ellipse cx="64" cy="100" rx="62" ry="5" fill="#141008" opacity="0.4" {s.blur(3)}/>')
    s.add(f'<path d="M14 100 C16 66 34 40 64 34 C94 40 112 66 114 100 Z" fill="#2a251c" opacity="0.7" {s.blur(3)}/>')
    stones = [(24, 87, 18, 15, '#9c988c'), (64, 86, 23, 16, '#aaa598'), (104, 89, 18, 13, '#938f84'),
              (36, 60, 21, 14, '#a39f93'), (78, 60, 22, 15, '#9a968a'), (57, 35, 22, 12, '#b0ab9e')]
    for x, y, rx, ry, c in stones:
        d = (f'M{x - rx} {y + ry * 0.3} C{x - rx} {y - ry * 0.9} {x - rx * 0.4} {y - ry} {x + rx * 0.1} {y - ry} '
             f'C{x + rx * 0.8} {y - ry} {x + rx} {y - ry * 0.4} {x + rx} {y + ry * 0.4} C{x + rx * 0.9} {y + ry} {x - rx * 0.8} {y + ry} {x - rx} {y + ry * 0.3} Z')
        g = s.radial(x - rx * 0.35, y - ry * 0.5, rx * 1.3, [(0, light(c, 0.35)), (0.55, c), (1, dark(c, 0.45))])
        speck = ''.join(f'<circle cx="{f(px)}" cy="{f(py)}" r="{f(s.rng.uniform(0.4, 1.1))}" fill="{s.rng.choice(["#6a665c", "#d8d4c8", "#7e7a6e"])}" opacity="0.7"/>'
                        for px, py in s.scatter(int(rx * ry / 8), x - rx, y - ry, x + rx, y + ry))
        s.add(f'<g {s.rough(2, 0.15)}><path d="{d}" fill="{g}"/></g><g {s.clip_d(d)}>{speck}</g>')
    for x, y, rx, ry in ((44, 26, 12, 4), (68, 46, 13, 4), (22, 72, 9, 3.5), (100, 76, 8, 3)):
        s.add(f'<g {s.rough(5, 0.35)}><ellipse cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="{s.radial(x - 3, y - 2, rx, [(0, "#9ac060"), (1, "#4a7a2c")])}"/></g>')
    s.save('einrichtung/steinwall.svg')

    s = Svg(128, 96, 'Geschlossener Erdpfropf: eine feste Wand aus Erde, Steinen und Wurzeln quer durch den Gang', 57)
    d = 'M4 94 C4 40 26 10 64 10 C102 10 124 40 124 94 Z'
    s.add(f'<g {s.rough(4, 0.2)}><path d="{d}" fill="{s.radial(50, 30, 90, [(0, "#8a643c"), (0.5, "#5e3f22"), (1, "#3a2614")])}"/></g>')
    clip = s.clip(f'<path d="{d}"/>')
    crumbs = ''.join(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(s.rng.uniform(0.6, 2))}" fill="{s.rng.choice(["#a07a50", "#2e1e10", "#7a5a3a"])}" opacity="0.8"/>'
                     for x, y in s.scatter(160, 4, 10, 124, 94))
    roots = s.strokes(s.scatter(10, 10, 20, 118, 80), 20, 18, 1.6, ['#c4a074', '#a8865a'], 0.8, 0.8, 0.4)
    s.add(f'<g {clip}>{crumbs}{roots}</g>')
    for x, y, rx, ry in ((38, 80, 9, 6), (88, 76, 11, 7), (64, 34, 7, 5)):
        s.add(f'<ellipse cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="{s.radial(x - 3, y - 2, rx * 1.2, [(0, "#d0cbbf"), (1, "#6a665c")])}"/>')
    s.save('einrichtung/erdpfropf_zu.svg')

    s = Svg(128, 80, 'Offener Erdpfropf: zwei bereitliegende Erdhaufen links und rechts, ein Brett dazwischen', 58)
    for d in ('M2 78 C4 50 18 36 34 36 C46 36 50 52 50 78 Z', 'M78 78 C78 52 84 36 96 36 C112 36 126 50 126 78 Z'):
        s.add(f'<g {s.rough(4, 0.25)}><path d="{d}" fill="{s.radial(30, 44, 50, [(0, "#9a7448"), (0.6, "#6e4c2c"), (1, "#4a321c")])}"/></g>')
        s.add(f'<g {s.clip_d(d)}>' + ''.join(
            f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(s.rng.uniform(0.6, 2))}" fill="{s.rng.choice(["#b08a5a", "#2e1e10"])}" opacity="0.8"/>'
            for x, y in s.scatter(60, 0, 34, 128, 80)) + '</g>')
    s.add(f'<path d="M48 70 L80 64 L82 72 L50 78 Z" fill="{s.linear(0, 64, 0, 78, [(0, "#b08a5a"), (1, "#6e4c2c")])}"/>'
          '<path d="M52 72 L78 67" stroke="#5a3c22" stroke-width="0.8" opacity="0.6"/>')
    s.save('einrichtung/erdpfropf_offen.svg')


# ---------------------------------------------------------------------------------------------
# Plätze von oben.

def places():
    s = Svg(128, 128, 'Sonnenplatz von oben: ein flacher, warmer Stein in einem Fleck Sonnenlicht', 61)
    s.add(f'<circle cx="64" cy="64" r="58" fill="#fff0b0" opacity="0.45" {s.blur(10)}/>')
    s.add(f'<circle cx="64" cy="64" r="36" fill="#fff6c8" opacity="0.5" {s.blur(8)}/>')
    s.add(f'<ellipse cx="67" cy="73" rx="31" ry="18" fill="#2a2418" opacity="0.4" {s.blur(3)}/>')
    s.add(f'<g {s.rough(2.5, 0.12)}><ellipse cx="64" cy="68" rx="30" ry="17" fill="{s.radial(54, 60, 36, [(0, "#e8dcc0"), (0.6, "#bcb098"), (1, "#8a806a")])}"/></g>')
    s.add(''.join(f'<circle cx="{f(x)}" cy="{f(y)}" r="0.9" fill="{s.rng.choice(["#8a806a", "#f4ecd8"])}" opacity="0.7"/>'
                  for x, y in s.scatter(60, 38, 54, 90, 82) if ((x - 64) / 28) ** 2 + ((y - 68) / 15) ** 2 < 1))
    s.save('einrichtung/sonnenplatz.svg')

    s = Svg(128, 128, 'Spielwiese von oben: ein liegender Ast zum Springen, ein Kiefernzapfen und Laub', 62)
    s.add(f'<ellipse cx="64" cy="66" rx="50" ry="44" fill="#a8885a" opacity="0.4" {s.blur(8)}/>')
    s.add(f'<path d="M22 94 L102 66" stroke="#1e1408" stroke-width="11" opacity="0.35" stroke-linecap="round" {s.blur(2.5)}/>')
    s.add(f'<path d="M20 90 L100 62" stroke="{s.linear(0, 70, 0, 84, [(0, "#a88458"), (0.5, "#7a5a38"), (1, "#4a3220")])}" stroke-width="10" stroke-linecap="round"/>')
    s.add(s.strokes([(30 + i * 6, 86 - i * 2.1) for i in range(12)], -19, 5, 0.8, ['#3a2614', '#c4a074'], 0.6, 0.1))
    s.add(f'<ellipse cx="100" cy="62" rx="4" ry="5" fill="#c4a074" transform="rotate(-19 100 62)"/>'
          f'<path d="M58 77 l-6 -12" stroke="#6a4a2c" stroke-width="3" stroke-linecap="round"/>')
    cone = ''.join(f'<ellipse cx="{f(40 + (i % 3 - 1) * 3.4)}" cy="{f(34 + i * 2.6)}" rx="3" ry="2" fill="{mix("#a87444", "#5a3a20", i / 7)}"/>'
                   for i in range(8))
    s.add(f'<ellipse cx="42" cy="46" rx="7" ry="12" fill="#1e1408" opacity="0.3" {s.blur(2)}/>' + cone)
    for x, y, a, c in ((88, 32, 30, '#c47a2c'), (24, 60, -40, '#a8a03c'), (96, 98, 70, '#b8642c')):
        s.add(f'<g transform="rotate({a} {x} {y})"><path d="M{x - 8} {y} C{x - 4} {y - 5} {x + 4} {y - 5} {x + 8} {y} '
              f'C{x + 4} {y + 5} {x - 4} {y + 5} {x - 8} {y} Z" fill="{c}"/><path d="M{x - 8} {y} H{x + 8}" stroke="{dark(c, 0.3)}" stroke-width="0.7"/></g>')
    s.save('einrichtung/spielwiese.svg')

    s = Svg(128, 128, 'Platz der Nachtwache von oben: eine Strohmatte, daneben Glühwürmchen in einem Rindenschälchen', 63)
    s.add(f'<ellipse cx="66" cy="73" rx="47" ry="34" fill="#1e1408" opacity="0.35" {s.blur(3)}/>')
    s.add(f'<g {s.rough(3, 0.4)}><ellipse cx="64" cy="70" rx="46" ry="33" fill="{s.radial(52, 58, 52, [(0, "#ecd48a"), (0.6, "#c9a446"), (1, "#8a6a2c")])}"/></g>')
    weave = ''.join(f'<path d="M{20 + i * 6} {42 + abs(i - 7) * 1.5} L{22 + i * 6} {98 - abs(i - 7) * 1.5}" stroke="#a8862f" stroke-width="1.2" opacity="0.6"/>'
                    for i in range(15))
    mat = s.clip('<ellipse cx="64" cy="70" rx="44" ry="31"/>')
    s.add(f'<g {mat}>{weave}' +
          s.strokes(s.scatter(80, 22, 40, 106, 100), 80, 6, 0.9, ['#f8e8a8', '#a8862f'], 0.6, 0.3) + '</g>')
    s.add(f'<circle cx="100" cy="34" r="20" fill="#f8f0a0" opacity="0.45" {s.blur(6)}/>')
    s.add(f'<ellipse cx="100" cy="36" rx="11" ry="8" fill="{s.radial(98, 33, 12, [(0, "#8a6440"), (1, "#3e2814")])}"/>')
    for x, y in ((97, 34), (102, 37), (100, 32), (104, 33)):
        s.add(f'<circle cx="{x}" cy="{y}" r="2.6" fill="#fff8a0" opacity="0.5" {s.blur(1.2)}/><circle cx="{x}" cy="{y}" r="1.1" fill="#fffde0"/>')
    s.save('einrichtung/nachtwache.svg')


def burrow():
    s = Svg(192, 192, 'Baueingang von oben: eine ovale Röhre führt schräg in die Erde, davor ein Fächer aus ausgeworfener Erde', 71)
    # Ausgeworfene Erde, nach vorn (unten im Bild) breit auslaufend und zum Rand hin dünner.
    spoil = 'M60 74 C40 96 34 132 52 160 C70 184 122 184 140 160 C158 132 152 96 132 74 C116 62 76 62 60 74 Z'
    s.add(f'<path d="{spoil}" fill="#6e4c2e" opacity="0.4" {s.blur(9)}/>')
    s.add(f'<g {s.rough(7, 0.08)}><path d="{spoil}" fill="{s.radial(92, 100, 80, [(0, "#9a7448"), (0.55, "#7e5a36"), (1, "#5e4228")])}" opacity="0.9" {s.blur(2)}/></g>')
    clip = s.clip_d(spoil)
    crumbs = ''.join(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(s.rng.uniform(0.6, 2.4))}" fill="{s.rng.choice(["#b89068", "#4e3220", "#a07a50", "#8a8478"])}" opacity="0.8"/>'
                     for x, y in s.scatter(320, 30, 60, 162, 186))
    clods = ''.join(f'<ellipse cx="{f(x)}" cy="{f(y)}" rx="{f(s.rng.uniform(2.5, 5))}" ry="{f(s.rng.uniform(2, 3.5))}" '
                    f'fill="{s.radial(x - 1.5, y - 1.5, 5, [(0, "#b08a5c"), (1, "#5e4228")])}"/>'
                    for x, y in s.scatter(26, 50, 110, 142, 172))
    # Kratzspuren vom Graben: Rillen vom Loch nach vorn.
    scratches = ''.join(f'<path d="M{f(x)} 112 q{f(s.rng.uniform(-6, 6))} 20 {f((x - 96) * 0.5)} 46" stroke="#4a3220" stroke-width="1.6" fill="none" opacity="0.45"/>'
                        for x in (82, 90, 99, 107))
    s.add(f'<g {clip}>{crumbs}{scratches}{clods}</g>')
    for x, y in ((128, 150), (66, 146), (110, 168)):
        s.add(f'<ellipse cx="{x + 1}" cy="{y + 1.5}" rx="4.5" ry="3.2" fill="#2a1e10" opacity="0.4"/>'
              f'<ellipse cx="{x}" cy="{y}" rx="4.5" ry="3.2" fill="{s.radial(x - 1.5, y - 1.2, 5, [(0, "#d8d2c4"), (1, "#7e786a")])}"/>')

    # Erdlippe rund um das Loch: hinten (oben) im Licht, weil dort die Wand der Röhre zu sehen ist.
    lip = 'M52 86 C52 58 70 44 96 44 C122 44 140 58 140 86 C140 108 120 120 96 120 C72 120 52 108 52 86 Z'
    s.add(f'<g {s.rough(4, 0.2)}><path d="{lip}" fill="{s.radial(96, 70, 52, [(0, "#a07a50"), (0.7, "#7a5634"), (1, "#5a3c22")])}"/></g>')
    hole = 'M60 86 C60 64 76 54 96 54 C116 54 132 64 132 86 C132 102 116 112 96 112 C76 112 60 102 60 86 Z'
    s.add(f'<path d="{hole}" fill="{s.linear(0, 54, 0, 112, [(0, "#6a4a2c"), (0.25, "#3e2814"), (0.6, "#120a04"), (1, "#050302")])}"/>')
    # Die Röhre führt nach vorn unten weg: unten im Loch am dunkelsten, die Rückwand bekommt etwas Licht.
    s.add(f'<g {s.clip_d(hole)}><ellipse cx="96" cy="100" rx="28" ry="14" fill="#000000" opacity="0.8" {s.blur(6)}/>'
          f'<path d="M64 74 C76 60 116 60 128 74" stroke="#9a7448" stroke-width="4" fill="none" opacity="0.5" {s.blur(2)}/>'
          + s.strokes(s.scatter(30, 64, 56, 128, 80), 90, 6, 0.8, ['#8a6440', '#2e1e10'], 0.5, 0.3) + '</g>')
    # Ein paar Wurzeln und Grashalme hängen über den hinteren Rand.
    s.add(s.strokes([(70 + i * 7 + s.rng.uniform(-2, 2), 52 + abs(i - 4) * 1.6) for i in range(9)], 95, 9, 1.1,
                    ['#c4a074', '#8a6a48'], 0.8, 0.25, 0.3))
    s.add(s.strokes([(x, y) for x, y in s.scatter(60, 40, 30, 152, 70) if (x - 96) ** 2 / 56 ** 2 + (y - 80) ** 2 / 36 ** 2 > 0.9],
                    -90, 9, 1.4, ['#8ab858', '#5e8a34', '#a8cc70'], 0.9, 0.5, 0.2))
    s.save('einrichtung/eingang.svg')


def log_end(s, cx, cy, r, rings=4):
    """Ein Holzstamm von oben: Rinde außen, helles Holz mit Jahresringen innen, Schatten daneben."""
    out = (f'<circle cx="{f(cx + r * 0.25)}" cy="{f(cy + r * 0.35)}" r="{f(r * 1.05)}" fill="#1e1408" opacity="0.4" {s.blur(r * 0.25)}/>'
           f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(r)}" fill="{s.radial(cx - r * 0.3, cy - r * 0.3, r * 1.3, [(0, "#8a6440"), (1, "#3e2814")])}"/>'
           f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(r * 0.82)}" fill="{s.radial(cx - r * 0.2, cy - r * 0.25, r, [(0, "#e8c890"), (0.7, "#c49a64"), (1, "#a07a4a")])}"/>')
    out += ''.join(f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(r * 0.82 * k / (rings + 1))}" fill="none" stroke="#8a643c" '
                   f'stroke-width="{f(max(0.6, r * 0.03))}" opacity="0.55"/>' for k in range(1, rings + 1))
    out += f'<path d="M{f(cx)} {f(cy)} l{f(r * 0.5)} {f(-r * 0.45)}" stroke="#6e4c2c" stroke-width="{f(max(0.6, r * 0.04))}" opacity="0.5"/>'
    return out


def flower(s, x, y, r, petal, middle='#f4cf45', n=6):
    petals = ''.join(f'<ellipse cx="{f(x + math.cos(i * math.tau / n) * r * 0.55)}" cy="{f(y + math.sin(i * math.tau / n) * r * 0.55)}" '
                     f'rx="{f(r * 0.5)}" ry="{f(r * 0.32)}" transform="rotate({f(i * 360 / n)} {f(x + math.cos(i * math.tau / n) * r * 0.55)} '
                     f'{f(y + math.sin(i * math.tau / n) * r * 0.55)})" fill="{petal}"/>' for i in range(n))
    return (f'<circle cx="{f(x + 1)}" cy="{f(y + 1.5)}" r="{f(r * 1.1)}" fill="#1e3014" opacity="0.3" {s.blur(r * 0.3)}/>'
            f'{petals}<circle cx="{f(x)}" cy="{f(y)}" r="{f(r * 0.3)}" fill="{middle}"/>')


def festival():
    s = Svg(384, 384, 'Festplatz von oben, 3 × 3 Felder: ein festgetretener Tanzplatz mit Steinkreis, Blumenmast in der Mitte, '
            'Wimpelketten zu vier Pfosten, eine Festtafel auf einem Baumstumpf und Blumen am Rand', 81)
    c = 192
    # Der Platz: festgetretene Erde, zum Rand hin weich ins Gras.
    s.add(f'<circle cx="{c}" cy="{c}" r="176" fill="#8a7448" opacity="0.4" {s.blur(14)}/>')
    s.add(f'<g {s.rough(10, 0.03)}><circle cx="{c}" cy="{c}" r="160" '
          f'fill="{s.radial(170, 160, 190, [(0, "#e2cc9c"), (0.6, "#ccb07a"), (1, "#b0925c")])}" {s.blur(4)}/></g>')
    place = s.clip(f'<circle cx="{c}" cy="{c}" r="156"/>')
    grain = ''.join(f'<circle cx="{f(x)}" cy="{f(y)}" r="{f(s.rng.uniform(0.6, 2))}" fill="{s.rng.choice(["#9a7a4c", "#f0dcb0", "#7e6440"])}" opacity="0.6"/>'
                    for x, y in s.scatter(900, 30, 30, 354, 354))
    # Ein festgetretener Ring vom Tanzen, etwas dunkler.
    ring = f'<circle cx="{c}" cy="{c}" r="112" fill="none" stroke="#a88a58" stroke-width="30" opacity="0.45" {s.blur(8)}/>'
    tufts = s.strokes([(x, y) for x, y in s.scatter(160, 20, 20, 364, 364) if 140 < math.hypot(x - c, y - c) < 172],
                      -90, 9, 1.4, ['#8ab858', '#5e8a34', '#a8cc70'], 0.8, 0.6, 0.2)
    paws = ''
    for _ in range(14):
        a, d = s.rng.uniform(0, math.tau), s.rng.uniform(95, 130)
        x, y = c + math.cos(a) * d, c + math.sin(a) * d
        rot = math.degrees(a) + 90
        paws += (f'<g transform="rotate({f(rot)} {f(x)} {f(y)})" fill="#8a6c40" opacity="0.5">'
                 f'<ellipse cx="{f(x - 4)}" cy="{f(y)}" rx="2.6" ry="4.4"/><ellipse cx="{f(x + 4)}" cy="{f(y)}" rx="2.6" ry="4.4"/></g>')
    s.add(f'<g {place}>{grain}{ring}{paws}</g>{tufts}')

    # Der Steinkreis.
    for i in range(8):
        a = i * math.tau / 8 + 0.2
        x, y = c + math.cos(a) * 82, c + math.sin(a) * 82
        s.add(f'<ellipse cx="{f(x + 2)}" cy="{f(y + 3)}" rx="9" ry="6.5" fill="#2a2418" opacity="0.4" {s.blur(2)}/>'
              f'<g {s.rough(1.6, 0.2)}><ellipse cx="{f(x)}" cy="{f(y)}" rx="9" ry="6.5" transform="rotate({f(math.degrees(a))} {f(x)} {f(y)})" '
              f'fill="{s.radial(x - 3, y - 3, 11, [(0, "#d8d4c8"), (0.6, "#a8a496"), (1, "#6e6a5e")])}"/></g>')

    # Wimpelketten vom Mast zu den Pfosten, mit Schatten am Boden.
    posts = [(70, 70), (314, 70), (314, 314), (70, 314)]
    colors = ['#d8503a', '#f0c040', '#5a9a48', '#5a8ad0', '#d86a90', '#f08a3a']
    for k, (px, py) in enumerate(posts):
        mx, my = (c + px) / 2 + (py - c) * 0.12, (c + py) / 2 - (px - c) * 0.12
        line = f'M{c} {c} Q{f(mx)} {f(my)} {px} {py}'
        s.add(f'<path d="{line}" transform="translate(8 12)" stroke="#1e1408" stroke-width="2" fill="none" opacity="0.25" {s.blur(2)}/>')
        flags, shadows = '', ''
        for j in range(1, 9):
            u = j / 9
            x = (1 - u) ** 2 * c + 2 * (1 - u) * u * mx + u * u * px
            y = (1 - u) ** 2 * c + 2 * (1 - u) * u * my + u * u * py
            dx, dy = 2 * (1 - u) * (mx - c) + 2 * u * (px - mx), 2 * (1 - u) * (my - c) + 2 * u * (py - my)
            n = math.hypot(dx, dy)
            tx, ty, nx, ny = dx / n, dy / n, -dy / n, dx / n
            tri = f'M{f(x - tx * 6)} {f(y - ty * 6)} L{f(x + tx * 6)} {f(y + ty * 6)} L{f(x + nx * 13)} {f(y + ny * 13)} Z'
            col = colors[(j + k) % len(colors)]
            shadows += f'<path d="{tri}" transform="translate(8 12)" fill="#1e1408" opacity="0.22"/>'
            flags += (f'<path d="{tri}" fill="{s.linear(x, y, x + nx * 13, y + ny * 13, [(0, light(col, 0.2)), (1, dark(col, 0.25))])}"/>')
        s.add(f'<g {s.blur(1.5)}>{shadows}</g>')
        s.add(f'<path d="{line}" stroke="#f4ecd8" stroke-width="1.4" fill="none"/>{flags}')
    for px, py in posts:
        s.add(log_end(s, px, py, 10, 2))

    # Der Blumenmast: ein Kranz aus Laub und Blumen um das Ende des Mastes.
    wreath = [(c + math.cos(a) * 22, c + math.sin(a) * 22, 9) for a in [i * math.tau / 14 for i in range(14)]]
    foliage(s, wreath, '#4a8a34', 3.5, box=(c - 32, c - 32, c + 32, c + 32))
    for i, col in enumerate(['#f2b8b0', '#ffffff', '#b98ad9', '#f4e7a0', '#d8503a', '#ffffff', '#f2b8b0']):
        a = i * math.tau / 7 + 0.3
        s.add(flower(s, c + math.cos(a) * 22, c + math.sin(a) * 22, 5.5, col))
    s.add(log_end(s, c, c, 11, 3))

    # Die Festtafel: ein breiter Baumstumpf mit Möhren, Klee und einer Löwenzahnblüte.
    tx, ty = c, 318
    s.add(f'<ellipse cx="{tx + 8}" cy="{ty + 10}" rx="50" ry="30" fill="#1e1408" opacity="0.4" {s.blur(6)}/>')
    s.add(f'<g {s.rough(2, 0.15)}><ellipse cx="{tx}" cy="{ty}" rx="46" ry="26" fill="{s.radial(tx - 14, ty - 10, 56, [(0, "#8a6440"), (1, "#3e2814")])}"/></g>')
    s.add(f'<ellipse cx="{tx}" cy="{ty}" rx="39" ry="20" fill="{s.radial(tx - 10, ty - 6, 44, [(0, "#e8c890"), (0.7, "#c49a64"), (1, "#a07a4a")])}"/>')
    s.add(''.join(f'<ellipse cx="{tx}" cy="{ty}" rx="{f(39 * k / 5)}" ry="{f(20 * k / 5)}" fill="none" stroke="#8a643c" stroke-width="1" opacity="0.5"/>'
                  for k in range(1, 5)))
    for x, y, rot in ((tx - 20, ty - 2, -12), (tx + 2, ty + 6, 8)):
        g = s.linear(-10, 0, 12, 0, [(0, '#c45a14'), (0.5, '#f4a050'), (1, '#e07a2a')])
        s.add(f'<g transform="translate({x} {y}) rotate({rot})">'
              f'<path d="M-2 1.5 C4 -1 10 -1 14 0 C10 1 4 4 -2 4 Z" fill="#1e1408" opacity="0.35"/>'
              f'<path d="M-12 -4 C-4 -6 6 -3 14 0 C6 3 -4 6 -12 4 C-14 2 -14 -2 -12 -4 Z" fill="{g}"/>'
              '<path d="M-6 -3 l1 2 M0 -2.5 l1 2 M6 -1.4 l1 1.6" stroke="#a84a14" stroke-width="0.8" fill="none"/>'
              '<path d="M-12 0 l-6 -5 M-12 0 l-8 0 M-12 0 l-6 5" stroke="#4e8a30" stroke-width="1.6" stroke-linecap="round"/></g>')
    for x, y in ((tx + 22, ty - 8), (tx + 28, ty - 2), (tx + 21, ty + 1)):
        s.add(f'<circle cx="{x + 1}" cy="{y + 1.5}" r="5" fill="#1e3014" opacity="0.3"/>'
              f'<circle cx="{x}" cy="{y}" r="5" fill="{s.radial(x - 1.5, y - 1.5, 6, [(0, "#8ac870"), (1, "#3a7a2c")])}"/>')
    s.add(flower(s, tx - 2, ty - 12, 5, '#f4cf45', '#e8a820', 10))

    # Blumen am Rand des Platzes.
    for _ in range(26):
        a, d = s.rng.uniform(0, math.tau), s.rng.uniform(150, 176)
        x, y = c + math.cos(a) * d, c + math.sin(a) * d
        if 8 < x < 376 and 8 < y < 376:
            s.add(flower(s, x, y, s.rng.uniform(3.5, 5.5), s.rng.choice(['#ffffff', '#ffffff', '#f2b8b0', '#b98ad9', '#f4e7a0', '#d8503a'])))
    s.save('einrichtung/festplatz.svg')


festival()
burrow()
rabbit()
fox()
marten()
buzzard()
plants()
stuff()
places()
print('ok')
