//! Spielkarte: ein Gitter mit Oberfläche und Untergrund auf derselben Fläche.
//! Jedes Untergrund-Feld kann gegraben sein und hat dann einen Tiefenwert.

use bevy::prelude::*;

use crate::daynight::{DayClock, Season};

pub const TILE_SIZE: f32 = 64.0;
pub const MAP_WIDTH: i32 = 40;
pub const MAP_HEIGHT: i32 = 30;
/// Ab dieser Zeile beginnt in Waldrand-Karten der Wald.
pub const FOREST_FROM: i32 = 24;

/// Von abgefressen bis voll dauert es gut eine Spielminute, etwa ein Viertel des Tages.
const GRASS_GROWTH_PER_SECOND: f32 = 0.014;
/// Nachts wächst kaum etwas nach.
const NIGHT_GROWTH_FACTOR: f32 = 0.3;
/// Im Frühling wächst alles schneller, im Herbst langsamer.
const SPRING_GROWTH_FACTOR: f32 = 1.3;
const AUTUMN_GROWTH_FACTOR: f32 = 0.5;
/// So schnell erfriert das Gras im Winter: nach gut anderthalb Spielminuten ist alles weg.
const FROST_PER_SECOND: f32 = 0.01;
/// So viel frisst ein Kaninchen mit einem Bissen.
const BITE: f32 = 0.5;
/// Darunter gilt ein Feld als kahlgefressen.
const BARE: f32 = 0.05;
/// So sehr leidet der Boden, wenn ein Feld kahlgefressen wird.
const OVERGRAZE_DAMAGE: f32 = 0.12;
/// So sehr düngt ein maßvoller Bissen.
const DUNG_BONUS: f32 = 0.03;
const MIN_FERTILITY: f32 = 0.1;
/// Gut gedüngte Wiesen tragen mehr als eine gesunde.
const MAX_FERTILITY: f32 = 1.3;
/// Nach so vielen Sekunden Ruhe erholt sich ein ausgelaugtes Feld …
const RECOVERY_AFTER_SECONDS: f32 = 40.0;
/// … um so viel pro Sekunde: von ganz kahl bis gesund gut drei Spielminuten.
const RECOVERY_PER_SECOND: f32 = 0.004;
/// Klee wächst so viel schneller nach als Gras.
const CLOVER_GROWTH_FACTOR: f32 = 2.0;
/// Und ist so viel nahrhafter.
const CLOVER_NUTRITION: f32 = 1.5;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Grid::starting_meadow())
            .init_resource::<ViewLayer>()
            .add_systems(Update, (toggle_view, grow_grass).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TilePos {
    pub x: i32,
    pub y: i32,
}

impl TilePos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn in_bounds(self) -> bool {
        (0..MAP_WIDTH).contains(&self.x) && (0..MAP_HEIGHT).contains(&self.y)
    }

    /// Mittelpunkt in Weltkoordinaten; die Karte ist um den Ursprung zentriert.
    pub fn center(self) -> Vec2 {
        map_origin() + (Vec2::new(self.x as f32, self.y as f32) + 0.5) * TILE_SIZE
    }

    pub fn from_world(position: Vec2) -> Option<Self> {
        let local = ((position - map_origin()) / TILE_SIZE).floor();
        let tile = Self::new(local.x as i32, local.y as i32);
        tile.in_bounds().then_some(tile)
    }

    pub fn from_world_clamped(position: Vec2) -> Self {
        let local = ((position - map_origin()) / TILE_SIZE).floor();
        Self::new(
            (local.x as i32).clamp(0, MAP_WIDTH - 1),
            (local.y as i32).clamp(0, MAP_HEIGHT - 1),
        )
    }

    pub fn neighbors(self) -> impl Iterator<Item = TilePos> {
        [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .map(move |(dx, dy)| TilePos::new(self.x + dx, self.y + dy))
            .filter(|p| p.in_bounds())
    }
}

fn map_origin() -> Vec2 {
    -Vec2::new(MAP_WIDTH as f32, MAP_HEIGHT as f32) * TILE_SIZE / 2.0
}

/// Alle Felder eines Rechtecks zwischen zwei Ecken, egal in welcher Richtung gezogen wurde.
pub fn tiles_between(a: TilePos, b: TilePos) -> impl Iterator<Item = TilePos> {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| TilePos::new(x, y)))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Layer {
    #[default]
    Surface,
    Underground,
}

impl Layer {
    pub fn other(self) -> Self {
        match self {
            Layer::Surface => Layer::Underground,
            Layer::Underground => Layer::Surface,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Layer::Surface => "Oberfläche",
            Layer::Underground => "Untergrund",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Depth {
    Shallow,
    Medium,
    Deep,
}

impl Depth {
    pub fn next(self) -> Self {
        match self {
            Depth::Shallow => Depth::Medium,
            Depth::Medium => Depth::Deep,
            Depth::Deep => Depth::Shallow,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Depth::Shallow => "flach",
            Depth::Medium => "mittel",
            Depth::Deep => "tief",
        }
    }

    /// Sekunden Spielzeit, die ein Kaninchen für ein Feld dieser Tiefe gräbt.
    pub fn dig_seconds(self) -> f32 {
        match self {
            Depth::Shallow => 3.0,
            Depth::Medium => 4.5,
            Depth::Deep => 6.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Chamber {
    Sleeping,
    Storage,
    /// Hier erzählen alte Kaninchen, und alle sammeln Weisheit.
    Story,
    /// Junge wachsen hier schneller heran.
    Nursery,
}

impl Chamber {
    pub fn label(self) -> &'static str {
        match self {
            Chamber::Sleeping => "Schlafkammer",
            Chamber::Storage => "Vorratskammer",
            Chamber::Story => "Erzählkammer",
            Chamber::Nursery => "Kinderstube",
        }
    }
}

/// Bauwerke an der Oberfläche, eins pro Feld.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum Feature {
    /// Dornenhecke: Füchse kommen nur langsam hindurch.
    Hedge,
    /// Fluchtloch: ein kleines Versteck ohne Verbindung zum Bau.
    BoltHole,
    /// Sonnenplatz: macht Kaninchen in der Nähe zufriedener.
    SunSpot,
    /// Kräuter: Wer hier frisst, wird zufriedener.
    Herbs,
    /// Spielwiese: macht die ganze Familie zufriedener.
    Playground,
}

impl Feature {
    pub fn label(self) -> &'static str {
        match self {
            Feature::Hedge => "Dornenhecke",
            Feature::BoltHole => "Fluchtloch",
            Feature::SunSpot => "Sonnenplatz",
            Feature::Herbs => "Kräutergarten",
            Feature::Playground => "Spielwiese",
        }
    }

    /// Ob auf dem Feld weiter Gras zum Fressen wächst.
    pub fn keeps_grass(self) -> bool {
        matches!(self, Feature::Herbs | Feature::SunSpot | Feature::Playground)
    }
}

#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct SurfaceTile {
    /// 0 = abgefressen, 1 = volles Gras.
    pub grass: f32,
    pub entrance: bool,
    pub entrance_planned: bool,
    /// Wachposten: Von hier hält ein Kaninchen Ausschau.
    pub post: bool,
    /// Gesäter Klee: wächst schneller nach und ist nahrhafter als Gras.
    pub clover: bool,
    /// Bodengüte: wie viel Gras höchstens wächst und wie schnell. 1 = gesunde Wiese.
    /// Kahlfressen laugt den Boden aus, maßvolles Beweiden düngt ihn.
    pub fertility: f32,
    /// Sekunden seit dem letzten Bissen; ruhende Felder erholen sich.
    rest: f32,
    pub feature: Option<Feature>,
    /// Waldboden: wenig Gras, aus dem Wald kommen die Füchse.
    #[serde(default)]
    pub forest: bool,
    /// Hier steht ein Baum (nur Zierde, man kann darunter hindurch).
    #[serde(default)]
    pub tree: bool,
}

impl SurfaceTile {
    /// Wie viel ein Bissen von diesem Feld wert ist, verglichen mit Gras.
    pub fn nutrition(&self) -> f32 {
        if self.clover { CLOVER_NUTRITION } else { 1.0 }
    }

    /// Ein Kaninchen frisst oder rupft einen Bissen ab. Gibt zurück, wie viel Gras es war.
    pub fn bite(&mut self) -> f32 {
        let eaten = self.grass.min(BITE);
        self.grass -= eaten;
        self.fertility = if self.grass < BARE {
            // Bis auf den Boden abgefressen: Die Wurzeln leiden.
            (self.fertility - OVERGRAZE_DAMAGE).max(MIN_FERTILITY)
        } else {
            // Es bleibt genug stehen, und die Köttel düngen.
            (self.fertility + DUNG_BONUS).min(MAX_FERTILITY)
        };
        self.rest = 0.0;
        eaten
    }
}

/// Bodenart unter der Wiese. Sie bestimmt, wie breit Gänge und Kammern werden dürfen,
/// damit niemand den ganzen Untergrund zu einer einzigen Halle aushöhlt.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Soil {
    /// Locker und rieselnd: nur schmale Gänge.
    Sand,
    #[default]
    Earth,
    /// Von Wurzeln zusammengehalten: Hier halten auch breite Kammern.
    Roots,
}

impl Soil {
    pub fn label(self) -> &'static str {
        match self {
            Soil::Sand => "Sand",
            Soil::Earth => "Erde",
            Soil::Roots => "Wurzelerde",
        }
    }

    /// So viele Felder breit darf ein offener Bereich hier höchstens sein, ohne einzustürzen.
    pub fn max_width(self) -> i32 {
        match self {
            Soil::Sand => 1,
            Soil::Earth => 2,
            Soil::Roots => 4,
        }
    }
}

/// Breiter als das trägt kein Boden, größere Quadrate muss niemand prüfen.
const WIDEST: i32 = 4;

#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct UndergroundTile {
    pub soil: Soil,
    pub dug: Option<Depth>,
    /// Ein vorbereiteter Erdpfropf: Kommt ein Marder, wird der Gang hier verschlossen.
    #[serde(default)]
    pub plug: bool,
    /// Ein Platz für die Nachtwache.
    #[serde(default)]
    pub watch: bool,
    /// Grabauftrag mit gewünschter Tiefe.
    pub planned: Option<Depth>,
    pub chamber: Option<Chamber>,
}

#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct Grid {
    surface: Vec<SurfaceTile>,
    underground: Vec<UndergroundTile>,
    /// Schneedecke auf der ganzen Wiese, 0 bis 1.
    pub snow: f32,
    /// Solange ein Marder im Bau ist, sind alle Erdpfropfen zu.
    #[serde(skip)]
    pub plugs_closed: bool,
}

impl Grid {
    fn index(p: TilePos) -> usize {
        debug_assert!(p.in_bounds());
        (p.y * MAP_WIDTH + p.x) as usize
    }

    pub fn surface(&self, p: TilePos) -> &SurfaceTile {
        &self.surface[Self::index(p)]
    }

    pub fn surface_mut(&mut self, p: TilePos) -> &mut SurfaceTile {
        &mut self.surface[Self::index(p)]
    }

    pub fn under(&self, p: TilePos) -> &UndergroundTile {
        &self.underground[Self::index(p)]
    }

    pub fn under_mut(&mut self, p: TilePos) -> &mut UndergroundTile {
        &mut self.underground[Self::index(p)]
    }

    pub fn positions() -> impl Iterator<Item = TilePos> {
        (0..MAP_HEIGHT).flat_map(|y| (0..MAP_WIDTH).map(move |x| TilePos::new(x, y)))
    }

    pub fn walkable(&self, layer: Layer, p: TilePos) -> bool {
        p.in_bounds()
            && match layer {
                Layer::Surface => true,
                Layer::Underground => {
                    let tile = self.under(p);
                    tile.dug.is_some() && !(tile.plug && self.plugs_closed)
                }
            }
    }

    /// Ob hier ein Gang hinpasst. Gibt sonst den Boden zurück, der für die Breite zu schwach ist:
    /// Ein offenes Quadrat darf nicht breiter werden, als sein lockerster Boden trägt.
    pub fn too_wide(&self, p: TilePos) -> Option<Soil> {
        let open = |q: TilePos| q == p || (q.in_bounds() && (self.under(q).dug.is_some() || self.under(q).planned.is_some()));
        for size in 2..=WIDEST + 1 {
            for top in p.y - size + 1..=p.y {
                for left in p.x - size + 1..=p.x {
                    let corner = TilePos::new(left, top);
                    let far = TilePos::new(left + size - 1, top + size - 1);
                    if !tiles_between(corner, far).all(open) {
                        continue;
                    }
                    let weakest = tiles_between(corner, far).map(|q| self.under(q).soil).min_by_key(|soil| soil.max_width());
                    if let Some(soil) = weakest.filter(|soil| size > soil.max_width()) {
                        return Some(soil);
                    }
                }
            }
        }
        None
    }

    pub fn chamber_tiles(&self, chamber: Chamber) -> usize {
        self.underground
            .iter()
            .filter(|t| t.chamber == Some(chamber) && t.dug.is_some())
            .count()
    }

    /// Die Wiese mit Wald am oberen Rand: Unter den Bäumen wächst wenig Gras, dafür halten Wurzeln
    /// den Boden zusammen. Am Waldsaum stehen ein paar natürliche Hecken.
    pub fn forest_edge() -> Self {
        let mut grid = Self::starting_meadow();
        for p in Self::positions() {
            if p.y >= FOREST_FROM {
                let deep = (p.y - FOREST_FROM) as f32 / (MAP_HEIGHT - FOREST_FROM) as f32;
                let tile = grid.surface_mut(p);
                tile.forest = true;
                tile.fertility = 0.5 - 0.2 * deep;
                tile.grass = tile.grass.min(tile.fertility);
                tile.tree = p.y > FOREST_FROM && noise(p) > 0.45 - 0.25 * deep;
                grid.under_mut(p).soil = Soil::Roots;
            } else if p.y >= FOREST_FROM - 2 && noise(TilePos::new(p.x + 101, p.y)) > 0.7 {
                grid.surface_mut(p).feature = Some(Feature::Hedge);
            }
        }
        grid
    }

    /// Wiese mit leicht ungleichmäßigem Gras und einem kleinen Startbau:
    /// ein Eingang, ein kurzer Gang und eine Schlafkammer.
    pub fn starting_meadow() -> Self {
        let count = (MAP_WIDTH * MAP_HEIGHT) as usize;
        let mut grid = Self {
            surface: vec![SurfaceTile::default(); count],
            underground: vec![UndergroundTile::default(); count],
            snow: 0.0,
            plugs_closed: false,
        };
        for p in Self::positions() {
            let tile = grid.surface_mut(p);
            tile.grass = 0.5 + 0.5 * noise(p);
            tile.fertility = 1.0;
            grid.under_mut(p).soil = soil_at(p);
        }

        let entrance = TilePos::new(20, 15);
        grid.surface_mut(entrance).entrance = true;
        grid.under_mut(entrance).dug = Some(Depth::Shallow);
        grid.under_mut(TilePos::new(21, 15)).dug = Some(Depth::Shallow);
        // Eine kleine Vorratskammer mit dem mitgebrachten Futter.
        let store = grid.under_mut(TilePos::new(22, 15));
        store.dug = Some(Depth::Medium);
        store.chamber = Some(Chamber::Storage);
        for p in tiles_between(TilePos::new(23, 14), TilePos::new(24, 16)) {
            let tile = grid.under_mut(p);
            tile.dug = Some(Depth::Medium);
            tile.chamber = Some(Chamber::Sleeping);
        }
        grid
    }
}

/// Bodenart in größeren Flecken: Sandadern, Erde und durchwurzelte Stellen.
/// Rund um den Startbau ist immer Erde, damit die mitgebrachte Schlafkammer hält.
fn soil_at(p: TilePos) -> Soil {
    if (p.x - 22).abs() <= 3 && (p.y - 15).abs() <= 3 {
        return Soil::Earth;
    }
    match smooth_noise(p, 6, 101) {
        v if v < 0.35 => Soil::Sand,
        v if v > 0.62 => Soil::Roots,
        _ => Soil::Earth,
    }
}

/// Weicher Verlauf aus Zufallswerten auf einem groben Raster mit `cell` Feldern Abstand.
fn smooth_noise(p: TilePos, cell: i32, seed: i32) -> f32 {
    let (cx, cy) = (p.x.div_euclid(cell), p.y.div_euclid(cell));
    let fade = |t: f32| t * t * (3.0 - 2.0 * t);
    let tx = fade(p.x.rem_euclid(cell) as f32 / cell as f32);
    let ty = fade(p.y.rem_euclid(cell) as f32 / cell as f32);
    let at = |x: i32, y: i32| noise(TilePos::new(x * 7 + seed, y * 13 - seed));
    let top = at(cx, cy) + (at(cx + 1, cy) - at(cx, cy)) * tx;
    let bottom = at(cx, cy + 1) + (at(cx + 1, cy + 1) - at(cx, cy + 1)) * tx;
    top + (bottom - top) * ty
}

/// Fester Pseudo-Zufallswert 0..1 pro Feld, damit die Wiese nicht wie ein Schachbrett aussieht.
fn noise(p: TilePos) -> f32 {
    let mut h = (p.x as u32).wrapping_mul(0x9E37_79B1) ^ (p.y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Welche Ansicht gerade zu sehen ist.
#[derive(Resource, Default)]
pub struct ViewLayer(pub Layer);

fn toggle_view(keys: Res<ButtonInput<KeyCode>>, mut view: ResMut<ViewLayer>) {
    if keys.just_pressed(KeyCode::Tab) {
        view.0 = view.0.other();
    }
}

fn grow_grass(time: Res<Time>, clock: Res<DayClock>, mut grid: ResMut<Grid>, mut timer: Local<f32>) {
    // Nur einmal pro Sekunde, damit der Boden nicht jedes Bild neu gebaut wird.
    *timer += time.delta_secs();
    if *timer < 1.0 {
        return;
    }
    let elapsed = std::mem::take(&mut *timer);
    grid.snow = clock.snow();

    // Im Winter erfriert das Gras, statt nachzuwachsen.
    if clock.season() == Season::Winter {
        let frost = FROST_PER_SECOND * elapsed;
        for tile in &mut grid.surface {
            tile.grass = (tile.grass - frost).max(0.0);
        }
        return;
    }

    let night = if clock.is_night() { NIGHT_GROWTH_FACTOR } else { 1.0 };
    let season = match clock.season() {
        Season::Spring => SPRING_GROWTH_FACTOR,
        Season::Autumn => AUTUMN_GROWTH_FACTOR,
        Season::Summer | Season::Winter => 1.0,
    };
    let growth = GRASS_GROWTH_PER_SECOND * night * season * elapsed;
    for tile in &mut grid.surface {
        tile.rest += elapsed;
        // Lange nicht beweidet: Der Boden erholt sich bis zur gesunden Wiese.
        if tile.rest > RECOVERY_AFTER_SECONDS && tile.fertility < 1.0 {
            tile.fertility = (tile.fertility + RECOVERY_PER_SECOND * elapsed).min(1.0);
        }
        // Ausgelaugter Boden trägt weniger Gras, und es wächst langsamer.
        if !tile.entrance && tile.grass < tile.fertility {
            let speed = if tile.clover { CLOVER_GROWTH_FACTOR } else { 1.0 };
            tile.grass = (tile.grass + growth * speed * tile.fertility).min(tile.fertility);
        }
    }
}

/// Farbe eines Oberflächenfeldes: Gras je nach Bewuchs, Markierungen für Aufträge und Posten.
pub fn surface_color(grid: &Grid, p: TilePos) -> Color {
    let tile = grid.surface(p);
    if tile.entrance {
        return Color::srgb(0.29, 0.20, 0.13);
    }
    let checker = if (p.x + p.y) % 2 == 0 { 1.0 } else { 0.96 };
    let mut rgb = mix([0.79, 0.70, 0.48], [0.56, 0.76, 0.37], tile.grass.min(1.0)).map(|c| c * checker);
    if tile.forest {
        // Waldboden: Moos und Laub im Schatten der Bäume.
        rgb = mix(rgb, [0.30 * checker, 0.40 * checker, 0.22 * checker], 0.7);
    } else {
        // Ausgelaugter Boden wird zu nackter, brauner Erde.
        let worn = (1.0 - tile.fertility).clamp(0.0, 1.0);
        rgb = mix(rgb, [0.62 * checker, 0.47 * checker, 0.31 * checker], worn * 0.85);
    }
    if tile.entrance_planned {
        rgb = mix(rgb, [0.95, 0.62, 0.25], 0.55);
    }
    if tile.clover {
        // Kräftiges Blaugrün, je voller, desto deutlicher.
        rgb = mix(rgb, [0.33, 0.60, 0.34], 0.35 + 0.35 * tile.grass);
    }
    if tile.post {
        rgb = mix(rgb, [0.60, 0.48, 0.30], 0.7);
    }
    match tile.feature {
        Some(Feature::Hedge) => rgb = mix(rgb, [0.25, 0.42, 0.22], 0.75),
        Some(Feature::BoltHole) => rgb = mix(rgb, [0.36, 0.26, 0.17], 0.8),
        Some(Feature::SunSpot) => rgb = mix(rgb, [0.95, 0.85, 0.50], 0.45),
        Some(Feature::Herbs) => rgb = mix(rgb, [0.45, 0.62, 0.40], 0.4),
        Some(Feature::Playground) => rgb = mix(rgb, [0.70, 0.82, 0.45], 0.4),
        None => {}
    }
    // Gänge darunter scheinen leicht durch.
    if grid.under(p).dug.is_some() {
        rgb = rgb.map(|c| c * 0.9);
    }
    // Schnee deckt alles zu, nur ein Hauch der Farbe darunter bleibt.
    if grid.snow > 0.0 {
        rgb = mix(rgb, [checker, checker, 1.0], grid.snow * 0.88);
    }
    Color::srgb(rgb[0], rgb[1], rgb[2])
}

/// Farbe eines Untergrund-Feldes: Erde, Gang je nach Tiefe, Aufträge und Kammern.
pub fn underground_color(grid: &Grid, p: TilePos) -> Color {
    let tile = grid.under(p);
    let checker = if (p.x + p.y) % 2 == 0 { 1.0 } else { 0.95 };
    let mut rgb = match tile.dug {
        // Ungegraben sieht man den Boden: heller Sand, Erde, dunkle Wurzelerde.
        None => match tile.soil {
            Soil::Sand => [0.86, 0.73, 0.48],
            Soil::Earth => [0.73, 0.54, 0.33],
            Soil::Roots => [0.60, 0.52, 0.30],
        },
        Some(Depth::Shallow) => [0.55, 0.39, 0.25],
        Some(Depth::Medium) => [0.43, 0.30, 0.18],
        Some(Depth::Deep) => [0.32, 0.21, 0.12],
    }
    .map(|c| c * checker);
    if let Some(depth) = tile.planned {
        // Je tiefer der Auftrag, desto dunkler die Markierung.
        let strength = match depth {
            Depth::Shallow => 0.35,
            Depth::Medium => 0.5,
            Depth::Deep => 0.65,
        };
        rgb = mix(rgb, [0.95, 0.80, 0.45], strength);
    }
    match tile.chamber {
        Some(Chamber::Sleeping) => rgb = mix(rgb, [0.66, 0.52, 0.78], 0.35),
        Some(Chamber::Storage) => rgb = mix(rgb, [0.91, 0.78, 0.42], 0.4),
        Some(Chamber::Story) => rgb = mix(rgb, [0.55, 0.70, 0.86], 0.35),
        Some(Chamber::Nursery) => rgb = mix(rgb, [0.95, 0.70, 0.72], 0.35),
        None => {}
    }
    if grid.surface(p).entrance {
        rgb = mix(rgb, [0.95, 0.90, 0.70], 0.45);
    }
    if tile.watch {
        rgb = mix(rgb, [0.55, 0.62, 0.85], 0.4);
    }
    if tile.plug {
        // Offen nur ein dunkler Erdwall, verschlossen fast schwarz.
        let closed = if grid.plugs_closed { 0.85 } else { 0.55 };
        rgb = mix(rgb, [0.22, 0.14, 0.08], closed);
    }
    Color::srgb(rgb[0], rgb[1], rgb[2])
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_of(soil: Soil) -> Grid {
        let mut grid = Grid::starting_meadow();
        for p in Grid::positions() {
            *grid.under_mut(p) = UndergroundTile { soil, ..default() };
        }
        grid
    }

    /// Gräbt alle Felder der Reihe nach, soweit der Boden es erlaubt, und zählt sie.
    fn dig_block(grid: &mut Grid, size: i32) -> usize {
        let mut dug = 0;
        for p in tiles_between(TilePos::new(5, 5), TilePos::new(5 + size - 1, 5 + size - 1)) {
            if grid.too_wide(p).is_none() {
                grid.under_mut(p).dug = Some(Depth::Medium);
                dug += 1;
            }
        }
        dug
    }

    #[test]
    fn sand_allows_only_single_tunnels() {
        let mut grid = grid_of(Soil::Sand);
        assert_eq!(dig_block(&mut grid, 1), 1);
        assert_eq!(grid.too_wide(TilePos::new(6, 5)), None);
        grid.under_mut(TilePos::new(6, 5)).dug = Some(Depth::Medium);
        assert_eq!(grid.too_wide(TilePos::new(5, 6)), None);
        grid.under_mut(TilePos::new(5, 6)).dug = Some(Depth::Medium);
        assert_eq!(grid.too_wide(TilePos::new(6, 6)), Some(Soil::Sand));
    }

    #[test]
    fn widths_follow_soil() {
        assert_eq!(dig_block(&mut grid_of(Soil::Earth), 2), 4);
        assert!(dig_block(&mut grid_of(Soil::Earth), 3) < 9);
        assert_eq!(dig_block(&mut grid_of(Soil::Roots), 4), 16);
        assert!(dig_block(&mut grid_of(Soil::Roots), 5) < 25);
    }

    #[test]
    fn weakest_soil_decides() {
        let mut grid = grid_of(Soil::Roots);
        grid.under_mut(TilePos::new(6, 6)).soil = Soil::Sand;
        assert_eq!(dig_block(&mut grid, 2), 3);
    }
}
