//! Hochwasser am Bach: Jeden Frühling zur Schneeschmelze und manchmal nach einem Gewitter
//! tritt der Bach über die Ufer. Das Wasser breitet sich Ring für Ring über die Aue aus, Dämme
//! halten es auf. Erreicht es einen Eingang, läuft es in den Bau und sammelt sich zuerst in den
//! tiefsten Gängen. Verschlossene Erdpfropfen halten es dort auf. Vollgelaufene Gänge trocknen
//! nach einer Weile wieder.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet, VecDeque};

use bevy::prelude::*;

use crate::audio::{Sound, Sounds};
use crate::behavior::{Activity, BehaviorSet, FEAR_SECONDS, Fear, Path, Reserved, release};
use crate::colony::{Colony, Messages, Rng};
use crate::daynight::{DayClock, Season};
use crate::map::{Chamber, Depth, FLOOD_REACH, Feature, Grid, Layer, TilePos};
use crate::rabbit::{OnLayer, Rabbit, z_for};
use crate::scenario::{Scenario, Terrain};

/// So lange vorher warnt das Spiel, in Sekunden.
const WARNING_SECONDS: f32 = 30.0;
/// So schnell steigt das Wasser: alle so viele Sekunden ein Feld weiter.
const RING_SECONDS: f32 = 2.0;
/// So lange steht die Aue unter Wasser, nachdem es am weitesten gekommen ist.
const HIGH_WATER_SECONDS: f32 = 40.0;
/// So viele Gangfelder füllt das Wasser, das durch einen Eingang hineinläuft.
const WATER_PER_ENTRANCE: usize = 8;
/// So lange braucht ein vollgelaufener Gang zum Trocknen; tiefe Gänge länger.
const WET_SECONDS: f32 = 120.0;
const DEEP_WET_FACTOR: f32 = 1.5;
/// Zur Schneeschmelze kommt das Hochwasser, wenn so viel vom Frühling vorbei ist.
const SPRING_FLOOD_AT: f32 = 0.35;
/// Wahrscheinlichkeit für ein Gewitterhochwasser an einem Sommer- oder Herbsttag.
const STORM_CHANCE: f32 = 0.3;

pub struct FloodPlugin;

impl Plugin for FloodPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Flood>().add_systems(
            Update,
            (schedule_floods, rise, wash_out, dry_tunnels).chain().before(BehaviorSet),
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FloodPhase {
    #[default]
    Calm,
    /// Der Bach schwillt an; gleich tritt er über die Ufer.
    Warning,
    /// Das Wasser breitet sich aus und steht dann eine Weile.
    Rising,
}

#[derive(Resource, Default)]
pub struct Flood {
    pub phase: FloodPhase,
    timer: f32,
    /// Wie weit das Wasser schon gekommen ist, in Feldern vom Bach.
    ring: i32,
    /// Abstand jedes erreichbaren Feldes vom Bach, mit Dämmen gerechnet.
    reach: HashMap<TilePos, i32>,
    /// In welchem Jahr die Schneeschmelze schon kam.
    spring_year: u32,
    /// An welchem Tag schon gewürfelt wurde, und wann heute ein Gewitter kommt.
    storm_day: u32,
    storm_at: Option<f32>,
    /// Kaninchen, die das Wasser bei diesem Hochwasser schon hinausgespült hat.
    washed_out: u32,
}

impl Flood {
    /// Solange Wasser droht, sind die Erdpfropfen zu.
    pub fn plugs_needed(&self) -> bool {
        self.phase != FloodPhase::Calm
    }

    /// Sofort warnen, z. B. für Tests.
    pub fn summon(&mut self) {
        self.phase = FloodPhase::Warning;
        self.timer = WARNING_SECONDS;
    }
}

fn schedule_floods(
    clock: Res<DayClock>,
    scenario: Res<Scenario>,
    mut flood: ResMut<Flood>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    mut sounds: ResMut<Sounds>,
) {
    if scenario.chapter.terrain != Terrain::Brook || flood.phase != FloodPhase::Calm {
        return;
    }
    let season = clock.season();
    let thaw = season == Season::Spring
        && clock.season_progress() >= SPRING_FLOOD_AT
        && flood.spring_year != clock.year();
    if clock.day != flood.storm_day {
        flood.storm_day = clock.day;
        let stormy = matches!(season, Season::Summer | Season::Autumn) && rng.chance(STORM_CHANCE);
        flood.storm_at = stormy.then(|| rng.range(0.15, 0.6));
    }
    let storm = flood.storm_at.is_some_and(|at| clock.time >= at);
    if !thaw && !storm {
        return;
    }
    flood.spring_year = clock.year();
    flood.storm_at = None;
    flood.summon();
    sounds.play(Sound::Flood);
    messages.push(if thaw {
        "Schneeschmelze: Der Bach schwillt an und tritt gleich über die Ufer! Dämme halten das Wasser auf."
    } else {
        "Ein Gewitter! Der Bach steigt und tritt gleich über die Ufer. Dämme halten das Wasser auf."
    });
}

fn rise(
    time: Res<Time>,
    mut flood: ResMut<Flood>,
    mut grid: ResMut<Grid>,
    mut colony: ResMut<Colony>,
    mut messages: ResMut<Messages>,
    mut sounds: ResMut<Sounds>,
) {
    if flood.phase == FloodPhase::Calm {
        // Nach einem Spielstand mitten im Hochwasser bleibt nichts unter Wasser stehen.
        if Grid::positions().any(|p| grid.surface(p).flooded) {
            recede(&mut grid);
        }
        return;
    }
    flood.timer -= time.delta_secs();
    if flood.timer > 0.0 {
        return;
    }
    match flood.phase {
        FloodPhase::Calm => {}
        FloodPhase::Warning => {
            flood.phase = FloodPhase::Rising;
            flood.ring = 0;
            flood.washed_out = 0;
            flood.timer = RING_SECONDS;
            flood.reach = grid.flood_distances(|p| grid.surface(p).feature == Some(Feature::Dam));
            sounds.play(Sound::Flood);
            messages.push("Hochwasser! Das Wasser steigt über die Aue.");
        }
        FloodPhase::Rising if flood.ring < FLOOD_REACH => {
            flood.ring += 1;
            flood.timer = RING_SECONDS;
            let ring = flood.ring;
            let reached: Vec<TilePos> =
                flood.reach.iter().filter(|(_, d)| **d == ring).map(|(p, _)| *p).collect();
            let mut food_lost = 0.0;
            let mut entrances = 0;
            for p in reached {
                grid.surface_mut(p).flooded = true;
                if grid.surface(p).entrance {
                    entrances += 1;
                    food_lost += pour_in(&mut grid, &mut colony, p);
                }
            }
            if entrances > 0 {
                messages.push("Das Wasser läuft durch einen Eingang in den Bau!");
            }
            if food_lost >= 1.0 {
                messages.push(format!("Im Vorratslager sind {food_lost:.0} Futter verdorben."));
            }
            if flood.ring == FLOOD_REACH {
                flood.timer = HIGH_WATER_SECONDS;
            }
        }
        FloodPhase::Rising => {
            flood.phase = FloodPhase::Calm;
            flood.reach.clear();
            recede(&mut grid);
            messages.push("Das Wasser geht zurück. Der Schlamm düngt die Aue.");
        }
    }
}

/// Das Wasser zieht ab und lässt fruchtbaren Schlamm zurück.
fn recede(grid: &mut Grid) {
    for p in Grid::positions() {
        let field = grid.surface_mut(p);
        if std::mem::take(&mut field.flooded) {
            field.fertility = field.fertility.max(1.0);
        }
    }
}

/// Wasser läuft durch den Eingang `entrance` in den Bau. Es füllt die tiefsten erreichbaren Gänge
/// zuerst, verschlossene Erdpfropfen halten es auf. Gibt zurück, wie viel Futter dabei verdirbt.
fn pour_in(grid: &mut Grid, colony: &mut Colony, entrance: TilePos) -> f32 {
    if grid.under(entrance).dug.is_none() {
        return 0.0;
    }
    // Wie weit jedes Gangfeld vom Eingang entfernt ist, durch Gänge gerechnet.
    let mut distance: HashMap<TilePos, u32> = HashMap::from([(entrance, 0)]);
    let mut queue = VecDeque::from([entrance]);
    while let Some(p) = queue.pop_front() {
        for q in p.neighbors() {
            let tile = grid.under(q);
            let blocked = tile.plug && grid.plugs_closed;
            if tile.dug.is_none() || blocked || distance.contains_key(&q) {
                continue;
            }
            distance.insert(q, distance[&p] + 1);
            queue.push_back(q);
        }
    }
    let depth_rank = |p: &TilePos| match grid.under(*p).dug {
        Some(Depth::Deep) => 0,
        Some(Depth::Medium) => 1,
        _ => 2,
    };
    let mut tiles: Vec<TilePos> = distance.keys().copied().collect();
    tiles.sort_by_key(|p| (depth_rank(p), distance[p], Reverse(p.y), p.x));
    tiles.truncate(WATER_PER_ENTRANCE);

    // Verdorben ist das Futter, das in den vollgelaufenen Vorratsfeldern liegt. Der Vorrat füllt
    // die trockenen Felder der Reihe nach, so wie er auch gezeichnet wird.
    let mut left = colony.food;
    let mut lost = 0.0;
    for p in Grid::positions() {
        let tile = grid.under(p);
        if tile.chamber != Some(Chamber::Storage) || tile.dug.is_none() || tile.wet > 0.0 {
            continue;
        }
        let here = left.min(colony.storage_per_tile);
        left -= here;
        if tiles.contains(&p) {
            lost += here;
        }
    }
    colony.food -= lost;

    for p in tiles {
        let tile = grid.under_mut(p);
        let seconds = if tile.dug == Some(Depth::Deep) { WET_SECONDS * DEEP_WET_FACTOR } else { WET_SECONDS };
        tile.wet = tile.wet.max(seconds);
    }
    lost
}

/// Wer in einem vollgelaufenen Gang ist, wird hinausgespült: auf das nächste trockene Feld draußen.
fn wash_out(
    grid: Res<Grid>,
    mut flood: ResMut<Flood>,
    mut reserved: ResMut<Reserved>,
    mut messages: ResMut<Messages>,
    mut rabbits: Query<(&mut Transform, &mut OnLayer, &mut Activity, &mut Path, &mut Fear), With<Rabbit>>,
) {
    let mut washed = 0;
    for (mut transform, mut layer, mut activity, mut path, mut fear) in &mut rabbits {
        let here = TilePos::from_world_clamped(transform.translation.truncate());
        if layer.0 != Layer::Underground || grid.under(here).wet <= 0.0 {
            continue;
        }
        let Some(dry) = nearest_dry(&grid, here) else {
            continue;
        };
        release(&activity, &mut reserved);
        let position = dry.center();
        transform.translation = position.extend(z_for(position));
        layer.0 = Layer::Surface;
        path.0.clear();
        *activity = Activity::Idle(2.0);
        fear.0 = FEAR_SECONDS;
        washed += 1;
    }
    if washed > 0 {
        flood.washed_out += washed;
        messages.push(if washed == 1 {
            "Das Wasser hat ein Kaninchen aus dem Bau gespült – nass, aber gesund.".to_string()
        } else {
            format!("Das Wasser hat {washed} Kaninchen aus dem Bau gespült – nass, aber gesund.")
        });
    }
}

/// Das trockene, begehbare Feld an der Oberfläche, das `from` am nächsten liegt.
fn nearest_dry(grid: &Grid, from: TilePos) -> Option<TilePos> {
    let mut seen = HashSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some(p) = queue.pop_front() {
        let field = grid.surface(p);
        if grid.walkable(Layer::Surface, p) && !field.flooded && !field.water {
            return Some(p);
        }
        for q in p.neighbors() {
            if seen.insert(q) {
                queue.push_back(q);
            }
        }
    }
    None
}

/// Vollgelaufene Gänge trocknen langsam wieder.
fn dry_tunnels(time: Res<Time>, mut grid: ResMut<Grid>, mut messages: ResMut<Messages>) {
    if !Grid::positions().any(|p| grid.under(p).wet > 0.0) {
        return;
    }
    let dt = time.delta_secs();
    let mut dried = false;
    for p in Grid::positions() {
        let tile = grid.under_mut(p);
        if tile.wet > 0.0 {
            tile.wet = (tile.wet - dt).max(0.0);
            dried |= tile.wet == 0.0;
        }
    }
    if dried && !Grid::positions().any(|p| grid.under(p).wet > 0.0) {
        messages.push("Der Bau ist wieder trocken.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::{Node, find_path};

    /// Ein Eingang mit einem flachen Gang nach rechts und einer tiefen Kammer am Ende.
    fn burrow() -> (Grid, TilePos) {
        let mut grid = Grid::brook();
        let entrance = TilePos::new(18, 25);
        grid.surface_mut(entrance).entrance = true;
        for x in 18..=27 {
            grid.under_mut(TilePos::new(x, 25)).dug = Some(Depth::Shallow);
        }
        for x in 26..=27 {
            grid.under_mut(TilePos::new(x, 26)).dug = Some(Depth::Deep);
        }
        (grid, entrance)
    }

    #[test]
    fn water_fills_the_deepest_tunnels_first() {
        let (mut grid, entrance) = burrow();
        pour_in(&mut grid, &mut Colony::default(), entrance);
        assert!(grid.under(TilePos::new(26, 26)).wet > 0.0, "die tiefe Kammer am Ende läuft voll");
        assert!(grid.under(TilePos::new(27, 26)).wet > 0.0);
        let wet = Grid::positions().filter(|p| grid.under(*p).wet > 0.0).count();
        assert_eq!(wet, WATER_PER_ENTRANCE);
        assert_eq!(grid.under(TilePos::new(24, 25)).wet, 0.0, "der Gang weiter hinten bleibt trocken");
    }

    #[test]
    fn a_closed_plug_holds_the_water_back() {
        let (mut grid, entrance) = burrow();
        grid.under_mut(TilePos::new(20, 25)).plug = true;
        grid.plugs_closed = true;
        pour_in(&mut grid, &mut Colony::default(), entrance);
        assert!(Grid::positions().filter(|p| p.x >= 20).all(|p| grid.under(p).wet == 0.0));
    }

    #[test]
    fn the_brook_can_only_be_crossed_at_a_ford() {
        let mut grid = Grid::brook();
        grid.owned = crate::map::Plot::all();
        let start = Node::new(Layer::Surface, TilePos::new(2, 18));
        let across = find_path(&grid, start, |node| node.pos == TilePos::new(30, 18)).expect("über die Furt");
        assert!(across.iter().any(|node| grid.surface(node.pos).ford));
    }
}
