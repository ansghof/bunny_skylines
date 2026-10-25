//! Spielkarte: ein Gitter mit Oberfläche und Untergrund auf derselben Fläche.
//! Jedes Untergrund-Feld kann gegraben sein und hat dann einen Tiefenwert.

use bevy::prelude::*;

use crate::daynight::{DayClock, Season};

pub const TILE_SIZE: f32 = 64.0;
pub const MAP_WIDTH: i32 = 40;
pub const MAP_HEIGHT: i32 = 30;

/// Von abgefressen bis voll dauert es gut eine Spielminute, etwa ein Viertel des Tages.
const GRASS_GROWTH_PER_SECOND: f32 = 0.014;
/// Nachts wächst kaum etwas nach.
const NIGHT_GROWTH_FACTOR: f32 = 0.3;
/// Im Frühling wächst alles schneller, im Herbst langsamer.
const SPRING_GROWTH_FACTOR: f32 = 1.3;
const AUTUMN_GROWTH_FACTOR: f32 = 0.5;
/// So schnell erfriert das Gras im Winter: nach gut anderthalb Spielminuten ist alles weg.
const FROST_PER_SECOND: f32 = 0.01;
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

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chamber {
    Sleeping,
    Storage,
}

impl Chamber {
    pub fn label(self) -> &'static str {
        match self {
            Chamber::Sleeping => "Schlafkammer",
            Chamber::Storage => "Vorratskammer",
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct SurfaceTile {
    /// 0 = abgefressen, 1 = volles Gras.
    pub grass: f32,
    pub entrance: bool,
    pub entrance_planned: bool,
    /// Wachposten: Von hier hält ein Kaninchen Ausschau.
    pub post: bool,
    /// Gesäter Klee: wächst schneller nach und ist nahrhafter als Gras.
    pub clover: bool,
}

impl SurfaceTile {
    /// Wie viel ein Bissen von diesem Feld wert ist, verglichen mit Gras.
    pub fn nutrition(&self) -> f32 {
        if self.clover { CLOVER_NUTRITION } else { 1.0 }
    }
}

#[derive(Clone, Copy, Default)]
pub struct UndergroundTile {
    pub dug: Option<Depth>,
    /// Grabauftrag mit gewünschter Tiefe.
    pub planned: Option<Depth>,
    pub chamber: Option<Chamber>,
}

#[derive(Resource)]
pub struct Grid {
    surface: Vec<SurfaceTile>,
    underground: Vec<UndergroundTile>,
    /// Schneedecke auf der ganzen Wiese, 0 bis 1.
    pub snow: f32,
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
                Layer::Underground => self.under(p).dug.is_some(),
            }
    }

    pub fn chamber_tiles(&self, chamber: Chamber) -> usize {
        self.underground
            .iter()
            .filter(|t| t.chamber == Some(chamber) && t.dug.is_some())
            .count()
    }

    /// Wiese mit leicht ungleichmäßigem Gras und einem kleinen Startbau:
    /// ein Eingang, ein kurzer Gang und eine Schlafkammer.
    pub fn starting_meadow() -> Self {
        let count = (MAP_WIDTH * MAP_HEIGHT) as usize;
        let mut grid = Self {
            surface: vec![SurfaceTile::default(); count],
            underground: vec![UndergroundTile::default(); count],
            snow: 0.0,
        };
        for p in Self::positions() {
            grid.surface_mut(p).grass = 0.5 + 0.5 * noise(p);
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
        if !tile.entrance && tile.grass < 1.0 {
            let speed = if tile.clover { CLOVER_GROWTH_FACTOR } else { 1.0 };
            tile.grass = (tile.grass + growth * speed).min(1.0);
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
    let mut rgb = mix([0.79, 0.70, 0.48], [0.56, 0.76, 0.37], tile.grass).map(|c| c * checker);
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
        None => [0.73, 0.54, 0.33],
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
        None => {}
    }
    if grid.surface(p).entrance {
        rgb = mix(rgb, [0.95, 0.90, 0.70], 0.45);
    }
    Color::srgb(rgb[0], rgb[1], rgb[2])
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}
