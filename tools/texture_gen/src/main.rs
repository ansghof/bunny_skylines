//! Malt die Bodentexturen des Spiels als PNG-Dateien unter `assets/boden/`: Wiese, Erde, Sand,
//! Steine, Wasser, Waldboden und den Untergrund. Jede Textur ist nahtlos kachelbar und fast farblos;
//! das Spiel färbt sie je nach Feld ein (Grasmenge, Dürre, Schnee …). Jede Datei lässt sich später
//! durch ein gemaltes Bild mit demselben Namen ersetzen.
//!
//! Aufruf aus dem Projektordner: `cargo run -p texture_gen`

use std::f32::consts::TAU;
use std::fs;
use std::path::Path;

use resvg::tiny_skia::{IntSize, Pixmap};

/// Kantenlänge jeder Textur in Pixeln; im Spiel deckt eine Textur 4 × 4 Felder ab.
const SIZE: usize = 512;
/// So hell ist jede Textur im Mittel; das Spiel gleicht das wieder aus.
const MEAN: f32 = 0.72;

/// Ein einfacher, fester Zufallsgenerator, damit jeder Lauf dieselben Bilder ergibt.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, from: f32, to: f32) -> f32 {
        from + (to - from) * self.next()
    }
}

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65535.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Wertrauschen, das sich nach `period` Gitterzellen wiederholt; `x` und `y` laufen von 0 bis 1.
fn tile_noise(x: f32, y: f32, period_x: i32, period_y: i32, seed: u32) -> f32 {
    let (fx, fy) = (x * period_x as f32, y * period_y as f32);
    let (ix, iy) = (fx.floor() as i32, fy.floor() as i32);
    let (tx, ty) = (smooth(fx - ix as f32), smooth(fy - iy as f32));
    let at = |dx: i32, dy: i32| hash((ix + dx).rem_euclid(period_x), (iy + dy).rem_euclid(period_y), seed);
    let top = at(0, 0) + (at(1, 0) - at(0, 0)) * tx;
    let bottom = at(0, 1) + (at(1, 1) - at(0, 1)) * tx;
    top + (bottom - top) * ty
}

/// Mehrere Lagen Rauschen übereinander, von grob bis fein, 0 bis 1.
fn fbm(x: f32, y: f32, period: i32, octaves: u32, seed: u32) -> f32 {
    fbm_stretched(x, y, period, period, octaves, seed)
}

fn fbm_stretched(x: f32, y: f32, period_x: i32, period_y: i32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut weight, mut total) = (0.0, 1.0, 0.0);
    for octave in 0..octaves {
        let scale = 1 << octave;
        sum += tile_noise(x, y, period_x * scale, period_y * scale, seed + octave * 31) * weight;
        total += weight;
        weight *= 0.5;
    }
    sum / total
}

/// Eine Leinwand, die an den Rändern umbricht, damit alles nahtlos kachelt.
struct Canvas(Vec<[f32; 3]>);

impl Canvas {
    fn new(paint: impl Fn(f32, f32) -> [f32; 3]) -> Self {
        let mut pixels = Vec::with_capacity(SIZE * SIZE);
        for y in 0..SIZE {
            for x in 0..SIZE {
                pixels.push(paint(x as f32 / SIZE as f32, y as f32 / SIZE as f32));
            }
        }
        Self(pixels)
    }

    fn blend(&mut self, x: i32, y: i32, color: [f32; 3], alpha: f32) {
        let i = y.rem_euclid(SIZE as i32) as usize * SIZE + x.rem_euclid(SIZE as i32) as usize;
        let pixel = &mut self.0[i];
        for c in 0..3 {
            pixel[c] += (color[c] - pixel[c]) * alpha;
        }
    }

    /// Ein weicher Fleck: eine Ellipse mit Radien `rx`, `ry`, um `angle` gedreht.
    fn blob(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, angle: f32, color: [f32; 3], alpha: f32) {
        let reach = rx.max(ry).ceil() as i32 + 1;
        let (sin, cos) = angle.sin_cos();
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (px, py) = (cx.floor() as i32 + dx, cy.floor() as i32 + dy);
                let (ox, oy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
                let (u, v) = ((ox * cos + oy * sin) / rx, (-ox * sin + oy * cos) / ry);
                let d = (u * u + v * v).sqrt();
                // Weicher Rand über etwa einen Pixel.
                let edge = ((1.0 - d) * rx.min(ry) + 0.5).clamp(0.0, 1.0);
                if edge > 0.0 {
                    self.blend(px, py, color, alpha * edge);
                }
            }
        }
    }

    /// Ein Strich von (x0, y0) nach (x1, y1), dessen Farbe zum Ende hin in `tip` übergeht.
    fn stroke(&mut self, from: (f32, f32), to: (f32, f32), width: f32, root: [f32; 3], tip: [f32; 3], alpha: f32) {
        let length = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
        let steps = (length * 2.0).ceil().max(1.0) as usize;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let color = [0, 1, 2].map(|c| root[c] + (tip[c] - root[c]) * t);
            // Halme werden zur Spitze schmaler.
            let r = width * (1.0 - 0.5 * t) / 2.0;
            let (x, y) = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
            self.blob(x, y, r.max(0.5), r.max(0.5), 0.0, color, alpha);
        }
    }

    /// Bringt die mittlere Helligkeit auf `MEAN` und speichert das Bild.
    fn save(self, path: &Path) {
        let luminance = |p: &[f32; 3]| 0.3 * p[0] + 0.55 * p[1] + 0.15 * p[2];
        let mean = self.0.iter().map(luminance).sum::<f32>() / self.0.len() as f32;
        let gain = MEAN / mean;
        let bytes: Vec<u8> = self
            .0
            .iter()
            .flat_map(|p| {
                let [r, g, b] = p.map(|c| ((c * gain).clamp(0.0, 1.0) * 255.0).round() as u8);
                [r, g, b, 255]
            })
            .collect();
        let pixmap = Pixmap::from_vec(bytes, IntSize::from_wh(SIZE as u32, SIZE as u32).unwrap()).unwrap();
        pixmap.save_png(path).unwrap();
    }
}

fn gray(v: f32) -> [f32; 3] {
    [v, v, v]
}

fn tinted(v: f32, tint: [f32; 3]) -> [f32; 3] {
    tint.map(|c| c * v)
}

/// Kurze Halme in vielen Hell- und Dunkeltönen, über einer leicht fleckigen Grasnarbe.
fn meadow() -> Canvas {
    let mut canvas = Canvas::new(|x, y| {
        let patches = fbm(x, y, 4, 4, 11);
        let fine = tile_noise(x, y, 128, 128, 12);
        tinted(0.78 + 0.2 * patches + 0.06 * fine, [1.0, 1.0, 0.92])
    });
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..14_000 {
        let (x, y) = (rng.range(0.0, SIZE as f32), rng.range(0.0, SIZE as f32));
        let length = rng.range(4.0, 11.0);
        let angle = -TAU / 4.0 + rng.range(-0.6, 0.6);
        let tip = (x + angle.cos() * length, y + angle.sin() * length);
        let width = rng.range(1.2, 2.2);
        if rng.next() < 0.55 {
            let v = rng.range(0.55, 0.75);
            canvas.stroke((x, y), tip, width, gray(v * 0.85), tinted(v, [1.0, 1.0, 0.9]), 0.75);
        } else {
            let v = rng.range(1.0, 1.15);
            canvas.stroke((x, y), tip, width, gray(v * 0.9), tinted(v, [1.04, 1.04, 0.86]), 0.7);
        }
    }
    canvas
}

/// Nackte Erde: Schollen, Krümel und ein paar Kiesel.
fn dirt() -> Canvas {
    let mut canvas = Canvas::new(|x, y| {
        let clods = fbm(x, y, 6, 5, 21);
        let crumbs = tile_noise(x, y, 96, 96, 22);
        gray(0.7 + 0.28 * clods + 0.08 * (crumbs - 0.5))
    });
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for _ in 0..5_000 {
        let (x, y) = (rng.range(0.0, SIZE as f32), rng.range(0.0, SIZE as f32));
        let r = rng.range(0.6, 1.6);
        let v = if rng.next() < 0.6 { rng.range(0.5, 0.7) } else { rng.range(1.0, 1.15) };
        canvas.blob(x, y, r, r, 0.0, gray(v), 0.7);
    }
    pebbles(&mut canvas, &mut rng, 90, (2.0, 5.0));
    canvas
}

/// Kleine Steinchen mit Schatten darunter und Licht darauf.
fn pebbles(canvas: &mut Canvas, rng: &mut Rng, count: usize, size: (f32, f32)) {
    for _ in 0..count {
        let (x, y) = (rng.range(0.0, SIZE as f32), rng.range(0.0, SIZE as f32));
        let rx = rng.range(size.0, size.1);
        let ry = rx * rng.range(0.6, 0.9);
        let angle = rng.range(0.0, TAU);
        let v = rng.range(0.95, 1.15);
        canvas.blob(x + 1.0, y + 1.5, rx, ry, angle, gray(0.45), 0.6);
        canvas.blob(x, y, rx, ry, angle, gray(v), 1.0);
        canvas.blob(x - rx * 0.25, y - ry * 0.3, rx * 0.45, ry * 0.4, angle, gray(v * 1.12), 0.6);
    }
}

/// Feiner Sand mit sanften Wellen vom Wind.
fn sand() -> Canvas {
    let mut rng = Rng(0x5851_F42D_4C95_7F2D);
    let mut canvas = Canvas::new(|x, y| {
        let warp = fbm(x, y, 3, 3, 31);
        let ripple = (TAU * (y * 14.0 + x * 2.0 + warp * 1.5)).sin();
        let patches = fbm(x, y, 5, 3, 32);
        gray(0.88 + 0.04 * ripple + 0.08 * (patches - 0.5))
    });
    for i in 0..SIZE * SIZE {
        let grain = rng.range(-0.07, 0.07);
        canvas.0[i] = canvas.0[i].map(|c| c + grain);
    }
    pebbles(&mut canvas, &mut rng, 40, (1.5, 3.0));
    canvas
}

/// Feldsteine: unregelmäßige, abgerundete Steine mit dunklen Fugen dazwischen.
fn stones() -> Canvas {
    const CELLS: i32 = 11;
    let point = |cx: i32, cy: i32| {
        let (wx, wy) = (cx.rem_euclid(CELLS), cy.rem_euclid(CELLS));
        (cx as f32 + 0.15 + 0.7 * hash(wx, wy, 41), cy as f32 + 0.15 + 0.7 * hash(wx, wy, 42), hash(wx, wy, 43))
    };
    Canvas::new(|x, y| {
        let (fx, fy) = (x * CELLS as f32, y * CELLS as f32);
        let (ix, iy) = (fx.floor() as i32, fy.floor() as i32);
        let (mut d1, mut d2, mut shade, mut toward) = (f32::MAX, f32::MAX, 0.0, (0.0, 0.0));
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (px, py, v) = point(ix + dx, iy + dy);
                let d = ((fx - px).powi(2) + (fy - py).powi(2)).sqrt();
                if d < d1 {
                    (d2, d1, shade, toward) = (d1, d, v, (fx - px, fy - py));
                } else if d < d2 {
                    d2 = d;
                }
            }
        }
        let gap = d2 - d1;
        let surface = fbm(x, y, 24, 3, 44);
        // Zur Mitte hin gewölbt; oben links trifft das Licht.
        let rim = smooth(((gap - 0.04) / 0.3).clamp(0.0, 1.0));
        let light = -(toward.0 + toward.1) * 0.5;
        let stone = 0.72 + 0.16 * shade + 0.1 * (surface - 0.5) + 0.1 * light;
        let mortar = 0.5 + 0.08 * surface;
        gray(mortar + (stone * (0.8 + 0.2 * rim) - mortar) * smooth(((gap - 0.03) / 0.06).clamp(0.0, 1.0)))
    })
}

/// Wasser mit langgezogenen Wellen und ein paar Lichtreflexen.
fn water() -> Canvas {
    let mut canvas = Canvas::new(|x, y| {
        let swell = fbm_stretched(x, y, 2, 8, 4, 51);
        let crest = ((swell - 0.55) / 0.15).clamp(0.0, 1.0);
        let depth = fbm(x, y, 3, 3, 52);
        gray(0.8 + 0.12 * depth + 0.22 * smooth(crest))
    });
    let mut rng = Rng(0x1405_7B7E_F767_814F);
    for _ in 0..260 {
        let (x, y) = (rng.range(0.0, SIZE as f32), rng.range(0.0, SIZE as f32));
        let length = rng.range(4.0, 14.0);
        canvas.stroke((x, y), (x + length, y + rng.range(-1.0, 1.0)), 1.6, gray(1.25), gray(1.1), 0.55);
    }
    canvas
}

/// Waldboden: Moos, Laub in vielen Tönen und ein paar Zweige.
fn forest_floor() -> Canvas {
    let mut canvas = Canvas::new(|x, y| {
        let moss = fbm(x, y, 5, 5, 61);
        gray(0.7 + 0.25 * moss)
    });
    let mut rng = Rng(0x6C07_8965_D5B4_A3C1);
    for _ in 0..3_200 {
        let (x, y) = (rng.range(0.0, SIZE as f32), rng.range(0.0, SIZE as f32));
        let rx = rng.range(3.0, 6.5);
        let v = rng.range(0.7, 1.2);
        let tint = if rng.next() < 0.4 { [1.15, 0.95, 0.75] } else { [1.0, 1.0, 0.9] };
        canvas.blob(x + 0.8, y + 1.0, rx, rx * 0.5, rng.range(0.0, TAU), gray(0.5), 0.3);
        canvas.blob(x, y, rx, rx * 0.5, rng.range(0.0, TAU), tinted(v, tint), 0.9);
    }
    for _ in 0..160 {
        let (x, y) = (rng.range(0.0, SIZE as f32), rng.range(0.0, SIZE as f32));
        let angle = rng.range(0.0, TAU);
        let length = rng.range(8.0, 22.0);
        let end = (x + angle.cos() * length, y + angle.sin() * length);
        canvas.stroke((x, y), end, 1.8, gray(0.45), gray(0.55), 0.85);
    }
    canvas
}

/// Ungegrabene Erde im Untergrund: grobe Schollen, Krümel und Steinchen.
fn soil() -> Canvas {
    let mut canvas = Canvas::new(|x, y| {
        let clods = fbm(x, y, 8, 4, 71);
        let cracks = (fbm(x, y, 12, 2, 72) - 0.5).abs();
        let crack = 1.0 - 0.1 * (1.0 - (cracks / 0.03).min(1.0));
        gray((0.72 + 0.26 * clods) * crack)
    });
    let mut rng = Rng(0x3C6E_F372_FE94_F82B);
    for _ in 0..3_500 {
        let (x, y) = (rng.range(0.0, SIZE as f32), rng.range(0.0, SIZE as f32));
        let r = rng.range(0.7, 2.0);
        let v = if rng.next() < 0.65 { rng.range(0.5, 0.7) } else { rng.range(1.0, 1.15) };
        canvas.blob(x, y, r, r * 0.8, 0.0, gray(v), 0.6);
    }
    pebbles(&mut canvas, &mut rng, 70, (1.5, 4.0));
    canvas
}

/// Festgetretener Gangboden: glatter als die Erde ringsum, mit ein paar Krümeln.
fn tunnel() -> Canvas {
    let mut canvas = Canvas::new(|x, y| {
        let packed = fbm(x, y, 5, 4, 81);
        gray(0.84 + 0.14 * packed)
    });
    let mut rng = Rng(0xA54F_F53A_5F1D_36F1);
    for _ in 0..1_800 {
        let (x, y) = (rng.range(0.0, SIZE as f32), rng.range(0.0, SIZE as f32));
        let r = rng.range(0.6, 1.3);
        let v = if rng.next() < 0.6 { rng.range(0.6, 0.75) } else { rng.range(1.0, 1.1) };
        canvas.blob(x, y, r, r, 0.0, gray(v), 0.6);
    }
    canvas
}

fn main() {
    let target = Path::new("assets/boden");
    fs::create_dir_all(target).unwrap();
    let textures: [(&str, fn() -> Canvas); 8] = [
        ("wiese", meadow),
        ("erde", dirt),
        ("sand", sand),
        ("steine", stones),
        ("wasser", water),
        ("waldboden", forest_floor),
        ("untergrund", soil),
        ("gang", tunnel),
    ];
    for (name, paint) in textures {
        let path = target.join(format!("{name}.png"));
        paint().save(&path);
        println!("{}", path.display());
    }
}
