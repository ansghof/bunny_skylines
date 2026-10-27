//! Spielkarte: ein Gitter mit Oberfläche und Untergrund auf derselben Fläche.
//! Jedes Untergrund-Feld kann gegraben sein und hat dann einen Tiefenwert.

use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;

use crate::daynight::{DayClock, Season};

pub const TILE_SIZE: f32 = 64.0;
pub const MAP_WIDTH: i32 = 40;
pub const MAP_HEIGHT: i32 = 30;
/// Ab dieser Zeile beginnt in Waldrand-Karten der Wald.
pub const FOREST_FROM: i32 = 24;
/// In dieser Spalte beginnt der alte Steinwall.
const WALL_X: i32 = 31;

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
/// Möhren machen dreimal so satt wie Gras …
const CARROT_NUTRITION: f32 = 3.0;
/// … wachsen aber nur halb so schnell.
const CARROT_GROWTH_FACTOR: f32 = 0.5;
/// In der Aue am Bach wächst das Gras schneller.
const FLOODPLAIN_GROWTH_FACTOR: f32 = 1.6;
/// So weit tritt der Bach bei Hochwasser über die Ufer, in Feldern.
pub const FLOOD_REACH: i32 = 3;
/// Bei voller Dürre wächst auf offener Wiese nur noch so viel vom üblichen Gras nach …
const DROUGHT_GROWTH_FACTOR: f32 = 0.08;
/// … und es verdorrt um so viel pro Sekunde, bis auf einen kümmerlichen Rest.
const WILT_PER_SECOND: f32 = 0.003;
const WILT_FLOOR: f32 = 0.1;
/// Im Schatten von Hecken und Bäumen und am Wasser wächst es auch in der Dürre so gut.
const SHADE_GROWTH_FACTOR: f32 = 0.6;
/// So weit reicht der Schatten einer Hecke oder eines Baums, und so weit hält ein Tümpel den Boden feucht.
const SHADE_REACH: i32 = 1;
const WATER_REACH: i32 = 2;
/// In diesen Reihen liegen Trittsteine im Bach.
const BROOK_FORDS: [i32; 3] = [5, 12, 24];

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Grid::starting_meadow())
            .init_resource::<ViewLayer>()
            .add_systems(Update, (toggle_view, grow_grass).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
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

/// Die Felder, die ein Bau der Seitenlänge `size` um sein Mittelfeld belegt.
pub fn footprint(center: TilePos, size: i32) -> impl Iterator<Item = TilePos> {
    let reach = size / 2;
    tiles_between(
        TilePos::new(center.x - reach, center.y - reach),
        TilePos::new(center.x + reach, center.y + reach),
    )
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
    /// Möhrenbeet: wächst langsam, macht aber sehr satt.
    Carrots,
    /// Festplatz: Kaninchen ohne Nest bleiben länger zufrieden.
    Festival,
    /// Damm: ein Erdwall, der Hochwasser aufhält.
    Dam,
}

impl Feature {
    pub fn label(self) -> &'static str {
        match self {
            Feature::Hedge => "Dornenhecke",
            Feature::BoltHole => "Fluchtloch",
            Feature::SunSpot => "Sonnenplatz",
            Feature::Herbs => "Kräutergarten",
            Feature::Playground => "Spielwiese",
            Feature::Carrots => "Möhrenbeet",
            Feature::Festival => "Festplatz",
            Feature::Dam => "Damm",
        }
    }

    /// Ob auf dem Feld weiter Gras zum Fressen wächst.
    /// Seitenlänge in Feldern: Große Bauten belegen ein Quadrat um ihr Mittelfeld.
    pub fn size(self) -> i32 {
        match self {
            Feature::Festival => 3,
            _ => 1,
        }
    }

    pub fn keeps_grass(self) -> bool {
        matches!(self, Feature::Herbs | Feature::SunSpot | Feature::Playground | Feature::Carrots)
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
    /// Ein alter, halb verfallener Steinwall; darin wohnen Marder.
    #[serde(default)]
    pub stones: bool,
    /// Offener Sand in einer Sandgrube.
    #[serde(default)]
    pub sand: bool,
    /// Ein Trampelpfad: Hier hoppeln Kaninchen schneller, das Gras wächst trotzdem.
    #[serde(default)]
    pub trail: bool,
    /// Gehört zu einem großen Bau: das Mittelfeld, auf dem er steht.
    #[serde(default)]
    pub part_of: Option<TilePos>,
    /// Der Bach: Hier kommt kein Kaninchen durch, außer an einer Furt.
    #[serde(default)]
    pub water: bool,
    /// Trittsteine im Bach.
    #[serde(default)]
    pub ford: bool,
    /// Aue: Hier wächst das Gras schneller, aber bei Hochwasser steht sie unter Wasser.
    #[serde(default)]
    pub floodplain: bool,
    /// Gerade überschwemmt.
    #[serde(default)]
    pub flooded: bool,
}

impl SurfaceTile {
    /// Wie viel ein Bissen von diesem Feld wert ist, verglichen mit Gras.
    pub fn nutrition(&self) -> f32 {
        if self.feature == Some(Feature::Carrots) {
            CARROT_NUTRITION
        } else if self.clover {
            CLOVER_NUTRITION
        } else {
            1.0
        }
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
    /// Fels: Hier lässt sich gar nicht graben.
    Rock,
}

impl Soil {
    pub fn label(self) -> &'static str {
        match self {
            Soil::Sand => "Sand",
            Soil::Earth => "Erde",
            Soil::Roots => "Wurzelerde",
            Soil::Rock => "Fels",
        }
    }

    /// So viele Felder breit darf ein offener Bereich hier höchstens sein, ohne einzustürzen.
    pub fn max_width(self) -> i32 {
        match self {
            Soil::Sand => 1,
            Soil::Earth => 2,
            Soil::Roots => 4,
            Soil::Rock => 0,
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
    /// Sekunden, bis der vollgelaufene Gang wieder trocken ist; 0 = trocken.
    #[serde(default)]
    pub wet: f32,
}

#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct Grid {
    surface: Vec<SurfaceTile>,
    underground: Vec<UndergroundTile>,
    /// Schneedecke auf der ganzen Wiese, 0 bis 1.
    pub snow: f32,
    /// Wie stark gerade die Dürre ist, 0 bis 1.
    #[serde(skip)]
    pub drought: f32,
    /// Solange ein Marder im Bau ist, sind alle Erdpfropfen zu.
    #[serde(skip)]
    pub plugs_closed: bool,
    /// Die Grundstücke, die der Familie gehören; nur dort wird gegrast und gebaut.
    #[serde(default = "Plot::all")]
    pub owned: Vec<Plot>,
}

/// Seitenlänge eines Grundstücks in Feldern.
pub const PLOT_SIZE: i32 = 10;
const PLOTS_X: i32 = MAP_WIDTH / PLOT_SIZE;
const PLOTS_Y: i32 = MAP_HEIGHT / PLOT_SIZE;

/// Ein Grundstück der Karte, wie in Cities: Skylines: Man kauft es zum Land der Familie dazu.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub struct Plot {
    pub x: i32,
    pub y: i32,
}

impl Plot {
    pub fn of(p: TilePos) -> Self {
        Self { x: p.x.div_euclid(PLOT_SIZE), y: p.y.div_euclid(PLOT_SIZE) }
    }

    pub fn all() -> Vec<Plot> {
        (0..PLOTS_Y).flat_map(|y| (0..PLOTS_X).map(move |x| Plot { x, y })).collect()
    }

    /// Die vier Grundstücke in der Mitte, auf denen jedes Kapitel beginnt.
    pub fn starting() -> Vec<Plot> {
        vec![Plot { x: 1, y: 1 }, Plot { x: 2, y: 1 }, Plot { x: 1, y: 2 }, Plot { x: 2, y: 2 }]
    }

    pub fn first_tile(self) -> TilePos {
        TilePos::new(self.x * PLOT_SIZE, self.y * PLOT_SIZE)
    }

    pub fn last_tile(self) -> TilePos {
        TilePos::new(self.x * PLOT_SIZE + PLOT_SIZE - 1, self.y * PLOT_SIZE + PLOT_SIZE - 1)
    }

    fn touches(self, other: Plot) -> bool {
        (self.x - other.x).abs() + (self.y - other.y).abs() == 1
    }
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
            && self.owns(p)
            && match layer {
                Layer::Surface => {
                    let field = self.surface(p);
                    !field.water || field.ford
                }
                Layer::Underground => {
                    let tile = self.under(p);
                    tile.dug.is_some() && !(tile.plug && self.plugs_closed) && tile.wet <= 0.0
                }
            }
    }

    /// Ob hier ein Gang hinpasst. Gibt sonst den Boden zurück, der für die Breite zu schwach ist:
    /// Ein offenes Quadrat darf nicht breiter werden, als sein lockerster Boden trägt.
    /// Gehört das Feld der Familie?
    pub fn owns(&self, p: TilePos) -> bool {
        self.owned.contains(&Plot::of(p))
    }

    /// Ein Grundstück lässt sich kaufen, wenn es noch niemandem gehört und an eigenes Land grenzt.
    pub fn can_buy(&self, plot: Plot) -> bool {
        !self.owned.contains(&plot) && self.owned.iter().any(|o| o.touches(plot))
    }

    /// Liegt das Feld am Rand des eigenen Landes? Dort verschwinden Fortziehende.
    pub fn on_border(&self, p: TilePos) -> bool {
        self.owns(p)
            && (!p.in_bounds() || p.neighbors().any(|q| !q.in_bounds() || !self.owns(q)))
    }

    #[cfg(test)]
    pub fn too_wide(&self, p: TilePos) -> Option<Soil> {
        self.too_wide_with(p, 0)
    }

    /// Wie `too_wide`, aber jeder Boden trägt `bonus` Felder mehr in der Breite (Stützwurzeln).
    pub fn too_wide_with(&self, p: TilePos, bonus: i32) -> Option<Soil> {
        // In Fels geht gar nichts, auch kein einzelner Gang.
        if self.under(p).soil == Soil::Rock {
            return Some(Soil::Rock);
        }
        let open = |q: TilePos| q == p || (q.in_bounds() && (self.under(q).dug.is_some() || self.under(q).planned.is_some()));
        for size in 2..=WIDEST + bonus + 1 {
            for top in p.y - size + 1..=p.y {
                for left in p.x - size + 1..=p.x {
                    let corner = TilePos::new(left, top);
                    let far = TilePos::new(left + size - 1, top + size - 1);
                    if !tiles_between(corner, far).all(open) {
                        continue;
                    }
                    let weakest = tiles_between(corner, far).map(|q| self.under(q).soil).min_by_key(|soil| soil.max_width());
                    if let Some(soil) = weakest.filter(|soil| size > soil.max_width() + bonus) {
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
            // Vollgelaufene Kammern sind nicht zu gebrauchen, bis sie trocken sind.
            .filter(|t| t.chamber == Some(chamber) && t.dug.is_some() && t.wet <= 0.0)
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

    /// Die Wiese mit einem alten Steinwall am rechten Rand. Unter dem Wall und in ein paar
    /// Flecken ringsum ist Fels; im Wall wohnen Marder.
    pub fn stone_wall() -> Self {
        let mut grid = Self::starting_meadow();
        for p in Self::positions() {
            let wall = (WALL_X..WALL_X + 2).contains(&p.x) && (3..MAP_HEIGHT - 3).contains(&p.y);
            if wall {
                let tile = grid.surface_mut(p);
                tile.stones = true;
                tile.fertility = 0.2;
                tile.grass = 0.1;
            }
            let near_wall = (p.x - WALL_X).abs() <= 3 && noise(TilePos::new(p.x, p.y + 57)) > 0.55;
            let patch = (p.x - 22).abs() + (p.y - 15).abs() > 5 && smooth_noise(p, 5, 333) > 0.72;
            if wall || near_wall || patch {
                grid.under_mut(p).soil = Soil::Rock;
            }
        }
        grid
    }

    /// Die Wiese an einer alten Sandgrube: Fast überall ist Sand, nur wenige feste Stellen.
    /// In der Grube selbst wächst kaum Gras.
    pub fn sand_pit() -> Self {
        let mut grid = Self::starting_meadow();
        for p in Self::positions() {
            let start_area = (p.x - 22).abs() <= 2 && (p.y - 15).abs() <= 2;
            if !start_area {
                grid.under_mut(p).soil = match smooth_noise(p, 5, 777) {
                    v if v > 0.8 => Soil::Roots,
                    v if v > 0.66 => Soil::Earth,
                    _ => Soil::Sand,
                };
            }
            // Die Grube: ein Oval links oben mit nacktem Sand.
            let dx = (p.x - 9) as f32 / 7.0;
            let dy = (p.y - 21) as f32 / 5.0;
            if dx * dx + dy * dy < 1.0 {
                let tile = grid.surface_mut(p);
                tile.fertility = 0.15;
                tile.grass = 0.0;
                tile.sand = true;
            }
        }
        grid
    }

    /// Die Wiese am Bach: Links vom Bau schlängelt sich ein Bach von Süd nach Nord, zwei Felder breit.
    /// Hinüber geht es nur an den Furten. Daneben liegt die Aue mit saftigem Gras.
    pub fn brook() -> Self {
        let mut grid = Self::starting_meadow();
        let bend = |y: i32| (13.0 + 2.2 * (y as f32 * 0.38 + 1.0).sin()).round() as i32;
        for y in 0..MAP_HEIGHT {
            // Mit der Biegung der Reihe darunter verbunden, damit der Bach keine Lücke hat.
            let below = bend((y - 1).max(0));
            for x in bend(y).min(below)..=bend(y).max(below) + 1 {
                let p = TilePos::new(x, y);
                let field = grid.surface_mut(p);
                field.water = true;
                field.ford = BROOK_FORDS.contains(&y);
                field.grass = 0.0;
                field.fertility = 0.0;
                grid.under_mut(p).soil = Soil::Rock;
            }
        }
        for (p, distance) in grid.flood_distances(|_| false) {
            if distance > 0 {
                grid.surface_mut(p).floodplain = true;
            }
        }
        grid
    }

    /// Die Trockenwiese: magerer Boden und dünnes Gras, nur ein kleiner Tümpel mit ein paar Bäumen.
    /// Im Sommer kommt die Dürre; dann grünt es nur noch im Schatten und am Wasser.
    pub fn dry_meadow() -> Self {
        let mut grid = Self::starting_meadow();
        for p in Self::positions() {
            let tile = grid.surface_mut(p);
            tile.fertility = 0.7 + 0.2 * smooth_noise(p, 4, 909);
            tile.grass = tile.grass.min(tile.fertility) * 0.8;
            // Der Tümpel: ein kleines Oval rechts unterhalb vom Bau.
            let dx = (p.x - 27) as f32 / 2.6;
            let dy = (p.y - 21) as f32 / 2.0;
            let reach = dx * dx + dy * dy;
            if reach < 1.0 {
                tile.water = true;
                tile.grass = 0.0;
                tile.fertility = 0.0;
                grid.under_mut(p).soil = Soil::Rock;
            } else if reach < 2.6 && noise(TilePos::new(p.x + 41, p.y - 13)) > 0.62 {
                tile.tree = true;
            }
        }
        // Ein paar alte Hecken stehen schon da.
        for (x, y) in [(12, 9), (13, 9), (14, 9), (31, 13), (31, 14)] {
            grid.surface_mut(TilePos::new(x, y)).feature = Some(Feature::Hedge);
        }
        grid
    }

    /// Eine Zufallskarte für das freie Spiel mit allem, was die Kapitel zeigen: Wald am oberen Rand,
    /// ein Bach mit Furten und Aue auf einer Seite, ein Steinwall mit Fels auf der anderen, eine
    /// Sandgrube, ein Tümpel mit Bäumen, ein paar Hecken, magere Flecken und wechselnde Böden.
    /// Rund um den Startbau bleibt immer feste Erde und Wiese.
    pub fn random(seed: u64) -> Self {
        let mut grid = Self::starting_meadow();
        let mut dice = Dice(seed | 1);
        // Verschiebt das feste Rauschen, damit jede Karte anders aussieht.
        let (sx, sy) = (dice.between(0, 9000), dice.between(0, 9000));
        let shifted = move |p: TilePos, extra: i32| TilePos::new(p.x + sx + extra, p.y + sy - extra);
        let seed_noise = dice.between(0, 9000);
        let start = |p: TilePos| (p.x - 22).abs() <= 5 && (p.y - 15).abs() <= 4;

        // Böden und magere Flecken.
        for p in Self::positions() {
            let tile = grid.surface_mut(p);
            tile.grass = 0.5 + 0.5 * noise(shifted(p, 0));
            tile.fertility = (0.75 + 0.35 * smooth_noise(p, 5, seed_noise)).min(1.0);
            tile.grass = tile.grass.min(tile.fertility);
            if !start(p) {
                grid.under_mut(p).soil = match smooth_noise(p, 6, seed_noise + 101) {
                    v if v < 0.33 => Soil::Sand,
                    v if v > 0.64 => Soil::Roots,
                    _ => Soil::Earth,
                };
            }
        }

        // Wald am oberen Rand mit welligem Saum.
        let forest_from = FOREST_FROM + dice.between(-1, 1);
        for p in Self::positions() {
            let edge = forest_from + (smooth_noise(TilePos::new(p.x, 0), 4, seed_noise + 7) * 3.0) as i32 - 1;
            if p.y >= edge {
                let deep = (p.y - edge) as f32 / (MAP_HEIGHT - edge).max(1) as f32;
                let tile = grid.surface_mut(p);
                tile.forest = true;
                tile.fertility = 0.5 - 0.2 * deep;
                tile.grass = tile.grass.min(tile.fertility);
                tile.tree = p.y > edge && noise(shifted(p, 3)) > 0.45 - 0.25 * deep;
                grid.under_mut(p).soil = Soil::Roots;
            }
        }

        // Der Bach links oder rechts vom Bau, von unten bis in den Wald.
        let left = dice.roll() < 0.5;
        let middle = if left { dice.between(6, 11) } else { dice.between(30, 33) } as f32;
        let (sway, phase) = (1.5 + dice.roll() * 1.2, dice.roll() * 6.0);
        let bend = |y: i32| (middle + sway * (y as f32 * 0.38 + phase).sin()).round() as i32;
        let fords = [dice.between(2, 7), dice.between(11, 17), dice.between(20, 24)];
        for y in 0..MAP_HEIGHT {
            let below = bend((y - 1).max(0));
            for x in bend(y).min(below)..=bend(y).max(below) + 1 {
                let p = TilePos::new(x, y);
                let field = grid.surface_mut(p);
                field.water = true;
                field.ford = fords.contains(&y);
                field.forest = false;
                field.tree = false;
                field.grass = 0.0;
                field.fertility = 0.0;
                grid.under_mut(p).soil = Soil::Rock;
            }
        }
        for (p, distance) in grid.flood_distances(|_| false) {
            if distance > 0 && !grid.surface(p).forest {
                grid.surface_mut(p).floodplain = true;
            }
        }

        // Auf der anderen Seite ein alter Steinwall mit Fels darunter.
        let wall_x = if left { dice.between(30, 33) } else { dice.between(5, 8) };
        let wall_from = dice.between(3, 8);
        let wall_to = (wall_from + dice.between(8, 13)).min(forest_from - 2);
        for p in Self::positions() {
            let wall = (wall_x..wall_x + 2).contains(&p.x) && (wall_from..=wall_to).contains(&p.y);
            if wall {
                let tile = grid.surface_mut(p);
                tile.stones = true;
                tile.fertility = 0.2;
                tile.grass = 0.1;
            }
            let near_wall = (p.x - wall_x).abs() <= 3
                && (wall_from - 2..=wall_to + 2).contains(&p.y)
                && noise(shifted(p, 57)) > 0.55;
            if (wall || near_wall) && !grid.surface(p).water {
                grid.under_mut(p).soil = Soil::Rock;
            }
        }

        // Unten eine Sandgrube mit Sand darunter, über dem Wall ein Tümpel mit Bäumen.
        let pit = Vec2::new(dice.between(15, 25) as f32, dice.between(3, 6) as f32);
        let pit_size = Vec2::new(dice.between(4, 6) as f32, dice.between(2, 3) as f32);
        let pond = Vec2::new((wall_x + if left { -6 } else { 7 }) as f32, (wall_to + 4).min(forest_from - 3) as f32);
        for p in Self::positions() {
            if grid.surface(p).water {
                continue;
            }
            let at = Vec2::new(p.x as f32, p.y as f32);
            let in_pit = ((at - pit) / pit_size).length_squared();
            if in_pit < 1.0 {
                let tile = grid.surface_mut(p);
                tile.fertility = 0.15;
                tile.grass = 0.0;
                tile.sand = true;
            }
            if in_pit < 2.2 && !start(p) {
                grid.under_mut(p).soil = Soil::Sand;
            }
            let in_pond = ((at - pond) / Vec2::new(2.6, 2.0)).length_squared();
            let tile = grid.surface_mut(p);
            if in_pond < 1.0 && !tile.stones && !start(p) {
                tile.water = true;
                tile.grass = 0.0;
                tile.fertility = 0.0;
                tile.forest = false;
                tile.tree = false;
                grid.under_mut(p).soil = Soil::Rock;
            } else if in_pond < 2.6 && !tile.stones && !start(p) && noise(shifted(p, 41)) > 0.6 {
                tile.tree = true;
            }
        }

        // Ein paar kurze Hecken auf freier Wiese.
        for _ in 0..dice.between(3, 5) {
            let (x, y, length) = (dice.between(2, 37), dice.between(2, forest_from - 3), dice.between(2, 4));
            let along_x = dice.roll() < 0.5;
            for i in 0..length {
                let p = if along_x { TilePos::new(x + i, y) } else { TilePos::new(x, y + i) };
                if !p.in_bounds() || start(p) {
                    continue;
                }
                let tile = grid.surface(p);
                if !(tile.water || tile.stones || tile.sand || tile.forest || tile.tree) {
                    grid.surface_mut(p).feature = Some(Feature::Hedge);
                }
            }
        }
        grid
    }

    /// Liegt das Feld im Schatten einer Hecke oder eines Baums oder nah am Wasser?
    /// Dort verdorrt das Gras in der Dürre nicht.
    pub fn sheltered(&self, p: TilePos) -> bool {
        let reach = SHADE_REACH.max(WATER_REACH);
        (-reach..=reach).any(|dy| {
            (-reach..=reach).any(|dx| {
                let q = TilePos::new(p.x + dx, p.y + dy);
                if !q.in_bounds() {
                    return false;
                }
                let near = dx.abs().max(dy.abs());
                let tile = self.surface(q);
                (near <= WATER_REACH && tile.water)
                    || (near <= SHADE_REACH && (tile.tree || tile.feature == Some(Feature::Hedge)))
            })
        })
    }

    /// Lässt das Gras eine Weile wachsen; `growth` ist, was auf gesunder Wiese nachwüchse.
    /// In der Dürre wächst offene Wiese kaum und verdorrt, im Schatten wächst es weiter.
    pub fn grow(&mut self, growth: f32, elapsed: f32) {
        let drought = self.drought;
        let shade: Vec<bool> = if drought > 0.0 {
            Self::positions().map(|p| self.sheltered(p)).collect()
        } else {
            Vec::new()
        };
        for (i, tile) in self.surface.iter_mut().enumerate() {
            tile.rest += elapsed;
            // Lange nicht beweidet: Der Boden erholt sich bis zur gesunden Wiese.
            if tile.rest > RECOVERY_AFTER_SECONDS && tile.fertility < 1.0 {
                tile.fertility = (tile.fertility + RECOVERY_PER_SECOND * elapsed).min(1.0);
            }
            if tile.water {
                continue;
            }
            let dry = if drought > 0.0 && !shade[i] {
                if tile.grass > WILT_FLOOR {
                    tile.grass = (tile.grass - WILT_PER_SECOND * drought * elapsed).max(WILT_FLOOR);
                }
                1.0 - (1.0 - DROUGHT_GROWTH_FACTOR) * drought
            } else {
                1.0 - (1.0 - SHADE_GROWTH_FACTOR) * drought
            };
            // Ausgelaugter Boden trägt weniger Gras, und es wächst langsamer.
            if !tile.entrance && tile.grass < tile.fertility {
                let speed = if tile.feature == Some(Feature::Carrots) {
                    CARROT_GROWTH_FACTOR
                } else if tile.clover {
                    CLOVER_GROWTH_FACTOR
                } else if tile.floodplain {
                    FLOODPLAIN_GROWTH_FACTOR
                } else {
                    1.0
                };
                tile.grass = (tile.grass + growth * dry * speed * tile.fertility).min(tile.fertility);
            }
        }
    }

    /// Wie weit das Hochwasser vom Bach aus bis zu jedem Feld laufen muss, höchstens `FLOOD_REACH`.
    /// Felder, für die `blocked` zutrifft (Dämme), hält es auf. Der Bach selbst hat Abstand 0.
    pub fn flood_distances(&self, blocked: impl Fn(TilePos) -> bool) -> HashMap<TilePos, i32> {
        let mut distances: HashMap<TilePos, i32> =
            Self::positions().filter(|p| self.surface(*p).water).map(|p| (p, 0)).collect();
        let mut queue: VecDeque<TilePos> = distances.keys().copied().collect();
        while let Some(p) = queue.pop_front() {
            let distance = distances[&p];
            if distance >= FLOOD_REACH {
                continue;
            }
            for q in p.neighbors() {
                if distances.contains_key(&q) || blocked(q) {
                    continue;
                }
                distances.insert(q, distance + 1);
                queue.push_back(q);
            }
        }
        distances
    }

    /// Alle Felder unter dem Steinwall, z. B. um Marder dort auftauchen zu lassen.
    pub fn wall_tiles(&self) -> Vec<TilePos> {
        Self::positions().filter(|p| self.surface(*p).stones).collect()
    }

    /// Wiese mit leicht ungleichmäßigem Gras und einem kleinen Startbau:
    /// ein Eingang, ein kurzer Gang und eine Schlafkammer.
    pub fn starting_meadow() -> Self {
        let count = (MAP_WIDTH * MAP_HEIGHT) as usize;
        let mut grid = Self {
            surface: vec![SurfaceTile::default(); count],
            underground: vec![UndergroundTile::default(); count],
            snow: 0.0,
            drought: 0.0,
            plugs_closed: false,
            owned: Plot::starting(),
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

/// Würfel für die Zufallskarte: aus einer Zahl immer dieselbe Folge.
struct Dice(u64);

impl Dice {
    /// Gleichverteilt in 0..1.
    fn roll(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Eine ganze Zahl von `lo` bis einschließlich `hi`.
    fn between(&mut self, lo: i32, hi: i32) -> i32 {
        (lo + (self.roll() * (hi - lo + 1) as f32) as i32).min(hi)
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
    grid.grow(GRASS_GROWTH_PER_SECOND * night * season * elapsed, elapsed);
}

/// Farbe eines Oberflächenfeldes: Gras je nach Bewuchs, Markierungen für Aufträge und Posten.
pub fn surface_color(grid: &Grid, p: TilePos) -> Color {
    let tile = grid.surface(p);
    // Eingänge malt ein eigenes Bild; der Boden drumherum bleibt, wie er ist.
    // Kein Schachbrett mehr: Die Textur zeigt die Felder, und Nachbarn gehen weich ineinander über.
    let checker = 1.0;
    if tile.water {
        // Wasser mit leichtem Wellenmuster; Furten mit grauen Trittsteinen.
        let ripple = 0.94 + 0.08 * noise(TilePos::new(p.x * 3 + 11, p.y * 5 - 7));
        let rgb = if tile.ford { [0.55, 0.62, 0.66] } else { [0.30, 0.52, 0.74] };
        return Color::srgb_from_array(unexplored_haze(grid, p, rgb.map(|c| c * ripple)));
    }
    let mut rgb = mix([0.79, 0.70, 0.48], [0.56, 0.76, 0.37], tile.grass.min(1.0)).map(|c| c * checker);
    if grid.drought > 0.0 && !tile.forest && !grid.sheltered(p) {
        // In der Dürre wird die offene Wiese strohgelb.
        rgb = mix(rgb, [0.86 * checker, 0.74 * checker, 0.44 * checker], grid.drought * 0.45);
    }
    if tile.floodplain {
        // Die Aue ist etwas kräftiger grün.
        rgb = mix(rgb, [0.40 * checker, 0.66 * checker, 0.36 * checker], 0.2);
    }
    if tile.stones {
        rgb = mix(rgb, [0.58 * checker, 0.57 * checker, 0.52 * checker], 0.8);
    } else if tile.sand {
        rgb = mix(rgb, [0.90 * checker, 0.80 * checker, 0.56 * checker], 0.9);
    } else if tile.forest {
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
        // Das Fluchtloch bringt seinen kleinen Erdwall selbst mit; der Boden drumherum bleibt, wie er ist.
        Some(Feature::BoltHole) => {}
        Some(Feature::SunSpot) => rgb = mix(rgb, [0.95, 0.85, 0.50], 0.45),
        Some(Feature::Herbs) => rgb = mix(rgb, [0.45, 0.62, 0.40], 0.4),
        Some(Feature::Playground) => rgb = mix(rgb, [0.70, 0.82, 0.45], 0.4),
        Some(Feature::Carrots) => rgb = mix(rgb, [0.52, 0.40, 0.24], 0.45),
        Some(Feature::Festival) => rgb = mix(rgb, [0.86, 0.74, 0.52], 0.5),
        Some(Feature::Dam) => rgb = mix(rgb, [0.50, 0.38, 0.24], 0.7),
        None => {}
    }
    // Gänge darunter scheinen leicht durch.
    if grid.under(p).dug.is_some() {
        rgb = rgb.map(|c| c * 0.9);
    }
    if tile.flooded {
        rgb = mix(rgb, [0.36, 0.56, 0.76], 0.75);
    }
    // Schnee deckt alles zu, nur ein Hauch der Farbe darunter bleibt.
    if grid.snow > 0.0 {
        rgb = mix(rgb, [checker, checker, 1.0], grid.snow * 0.88);
    }
    Color::srgb_from_array(unexplored_haze(grid, p, rgb))
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
            Soil::Rock => [0.55, 0.55, 0.54],
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
    if tile.wet > 0.0 && tile.dug.is_some() {
        // Vollgelaufen: blau, beim Abtrocknen schlammig.
        let water = (tile.wet / 30.0).min(1.0);
        rgb = mix(rgb, [0.30, 0.48, 0.70], 0.35 + 0.4 * water);
    }
    Color::srgb_from_array(unexplored_haze(grid, p, rgb))
}

/// Land, das der Familie noch nicht gehört, ist blasser und dunkler, damit man die Grenze sieht.
fn unexplored_haze(grid: &Grid, p: TilePos, rgb: [f32; 3]) -> [f32; 3] {
    if grid.owns(p) {
        rgb
    } else {
        mix(rgb, [0.36, 0.38, 0.36], 0.6)
    }
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
    fn support_roots_widen_every_soil() {
        let mut grid = grid_of(Soil::Sand);
        for p in tiles_between(TilePos::new(5, 5), TilePos::new(6, 6)) {
            assert_eq!(grid.too_wide_with(p, 1), None);
            grid.under_mut(p).dug = Some(Depth::Medium);
        }
        assert_eq!(grid.too_wide_with(TilePos::new(7, 5), 1), None);
        // Ein 3×3-Quadrat ist auch mit Stützwurzeln zu viel für Sand.
        for (x, y) in [(7, 5), (7, 6), (5, 7), (6, 7)] {
            grid.under_mut(TilePos::new(x, y)).dug = Some(Depth::Medium);
        }
        assert_eq!(grid.too_wide_with(TilePos::new(7, 7), 1), Some(Soil::Sand));
    }

    #[test]
    fn rock_never_digs() {
        let grid = grid_of(Soil::Rock);
        assert_eq!(grid.too_wide_with(TilePos::new(5, 5), 3), Some(Soil::Rock));
    }

    #[test]
    fn every_random_map_has_all_landscapes_and_a_safe_start() {
        for seed in 1..40u64 {
            let grid = Grid::random(seed * 7919);
            let any = |f: fn(&SurfaceTile) -> bool| Grid::positions().any(|p| f(grid.surface(p)));
            assert!(any(|t| t.forest && t.tree), "Wald fehlt bei {seed}");
            assert!(any(|t| t.water && t.ford), "Furt fehlt bei {seed}");
            assert!(any(|t| t.floodplain), "Aue fehlt bei {seed}");
            assert!(any(|t| t.stones), "Steinwall fehlt bei {seed}");
            assert!(any(|t| t.sand), "Sandgrube fehlt bei {seed}");
            assert!(any(|t| t.feature == Some(Feature::Hedge)), "Hecke fehlt bei {seed}");
            // Der Startbau liegt auf fester Erde, trocken und frei.
            for p in tiles_between(TilePos::new(19, 13), TilePos::new(25, 17)) {
                let tile = grid.surface(p);
                assert!(!tile.water && !tile.stones && !tile.forest, "Start belegt bei {seed}: {p:?}");
                assert_eq!(grid.under(p).soil, Soil::Earth, "Startboden bei {seed}: {p:?}");
            }
            assert!(grid.surface(TilePos::new(20, 15)).entrance);
        }
    }

    #[test]
    fn in_a_drought_grass_only_stays_green_in_the_shade() {
        let mut grid = Grid::starting_meadow();
        let open = TilePos::new(5, 5);
        let shaded = TilePos::new(30, 5);
        grid.surface_mut(TilePos::new(31, 5)).feature = Some(Feature::Hedge);
        for p in [open, shaded] {
            grid.surface_mut(p).grass = 0.5;
        }
        grid.drought = 1.0;
        for _ in 0..60 {
            grid.grow(GRASS_GROWTH_PER_SECOND, 1.0);
        }
        assert!(grid.surface(open).grass < 0.45, "offene Wiese verdorrt");
        assert!(grid.surface(shaded).grass > 0.8, "im Schatten wächst es weiter");
    }

    #[test]
    fn weakest_soil_decides() {
        let mut grid = grid_of(Soil::Roots);
        grid.under_mut(TilePos::new(6, 6)).soil = Soil::Sand;
        assert_eq!(dig_block(&mut grid, 2), 3);
    }
}
