//! Was Kaninchen tun: Hunger stillen, Aufträge graben, Futter in den Vorrat tragen, Wache halten,
//! nachts schlafen und bei Gefahr in den Bau flüchten.
//! Jedes Kaninchen entscheidet selbst, sobald es mit seiner aktuellen Tätigkeit fertig ist.

use std::collections::{HashMap, HashSet, VecDeque};

use bevy::prelude::*;

use crate::colony::{Colony, Rng, SLEEP_PER_TILE, cost};
use crate::daynight::{DayClock, Season};
use crate::map::{Chamber, Depth, Grid, Layer, TilePos};
use crate::pathfinding::{Node, find_path};
use crate::rabbit::{Hunger, Offset, OnLayer, Rabbit, z_for};

const HUNGER_PER_SECOND: f32 = 0.012;
const HUNGRY: f32 = 0.55;
/// So viele Kaninchen grasen höchstens gemeinsam auf einem Feld; am Vorrat genauso.
const MAX_PER_TILE: usize = 3;
/// So viel Hunger entsteht im Schlaf und in der Winterruhe im Vergleich zum Wachsein.
const RESTING_HUNGER_FACTOR: f32 = 0.35;
/// Futtereinheiten im Vorrat pro abgeerntetem Gras. Ein Futter stillt 0,6 Hunger,
/// frisch gefressenes Gras 1,3 pro Gras: Einlagern verdoppelt den Wert ungefähr.
const FOOD_PER_GATHERED_GRASS: f32 = 4.0;
const WALK_SPEED: f32 = 110.0;
const FLEE_SPEED: f32 = 165.0;
/// Ab so viel Gras lohnt sich ein Feld zum Fressen.
const EDIBLE_GRASS: f32 = 0.3;
const GUARD_SHIFT_SECONDS: f32 = 60.0;
/// So lange meiden Kaninchen nach einer Flucht die Oberfläche.
const FEAR_SECONDS: f32 = 12.0;

pub struct BehaviorPlugin;

impl Plugin for BehaviorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Reserved>()
            .init_resource::<Priorities>()
            .add_systems(Update, (tick_needs, decide, walk, work).chain().in_set(BehaviorSet));
    }
}

/// Andere Systeme, die Tätigkeiten unterbrechen (Alarm, Fuchs), laufen nach dem Verhalten.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BehaviorSet;

#[derive(Component)]
pub enum Activity {
    /// Wartet noch so viele Sekunden, dann entscheidet es neu.
    Idle(f32),
    Walking(Goal),
    Fleeing,
    Eating { tile: TilePos, left: f32 },
    Gathering { tile: TilePos, left: f32 },
    Digging { target: DigTarget, left: f32 },
    Guarding { post: TilePos, left: f32 },
    /// Schläft im Nest auf diesem Feld der Schlafkammer.
    Sleeping { bed: TilePos },
}

#[derive(Clone, Copy, Debug)]
pub enum Goal {
    Eat(TilePos),
    Gather(TilePos),
    Dig(DigTarget),
    Guard(TilePos),
    /// Futter auf diesem Feld der Vorratskammer abladen.
    Store(TilePos),
    /// Auf diesem Feld der Vorratskammer fressen.
    EatFromStore(TilePos),
    Sleep(TilePos),
    Wander,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DigTarget {
    /// Ein Untergrund-Feld, gegraben von einem Nachbarfeld aus.
    Tunnel(TilePos),
    /// Ein Eingang, gegraben von der Oberfläche aus.
    Entrance(TilePos),
}

#[derive(Component, Default)]
pub struct Path(pub VecDeque<Node>);

/// Getragenes Futter in Futtereinheiten.
#[derive(Component, Default)]
pub struct Carrying(pub f32);

/// Restliche Sekunden Angst nach einer Flucht.
#[derive(Component, Default)]
pub struct Fear(pub f32);

/// Was der Spieler gerade wichtig findet.
#[derive(Resource, Default)]
pub struct Priorities {
    /// Alle Erwachsenen sammeln zuerst Futter; Graben und Wache haben Pause.
    pub gather_first: bool,
}

/// Aufträge und Wachposten, die schon ein Kaninchen übernommen hat.
#[derive(Resource, Default)]
pub struct Reserved {
    dig: HashSet<DigTarget>,
    posts: HashSet<TilePos>,
    /// Belegte Schlafplätze pro Nest.
    beds: HashMap<TilePos, usize>,
    /// Kaninchen pro Feld beim Grasen, Sammeln und am Vorrat, unterwegs mitgezählt.
    spots: HashMap<TilePos, usize>,
}

impl Reserved {
    fn free_bed(&mut self, bed: TilePos) {
        if let Some(count) = self.beds.get_mut(&bed) {
            *count = count.saturating_sub(1);
        }
    }

    fn spot_free(&self, tile: TilePos) -> bool {
        self.spots.get(&tile).copied().unwrap_or(0) < MAX_PER_TILE
    }

    /// Nur für Testläufe: die meisten Kaninchen, die sich gerade ein Feld teilen.
    #[cfg(debug_assertions)]
    pub fn busiest_spot(&self) -> usize {
        self.spots.values().max().copied().unwrap_or(0)
    }

    fn claim_spot(&mut self, tile: TilePos) {
        *self.spots.entry(tile).or_default() += 1;
    }

    fn free_spot(&mut self, tile: TilePos) {
        if let Some(count) = self.spots.get_mut(&tile) {
            *count = count.saturating_sub(1);
        }
    }

    /// Belegt das Feld, auf das ein Ziel führt, falls es eins ist, das sich Kaninchen teilen.
    fn claim_goal(&mut self, goal: Goal) {
        if let Some(tile) = goal.spot() {
            self.claim_spot(tile);
        }
    }
}

impl Goal {
    /// Das Feld, auf dem höchstens `MAX_PER_TILE` Kaninchen gleichzeitig sein dürfen.
    fn spot(self) -> Option<TilePos> {
        match self {
            Goal::Eat(tile) | Goal::Gather(tile) | Goal::Store(tile) | Goal::EatFromStore(tile) => Some(tile),
            _ => None,
        }
    }
}

/// Gibt frei, was ein Kaninchen für seine Tätigkeit belegt hat, z. B. wenn es flüchtet.
pub fn release(activity: &Activity, reserved: &mut Reserved) {
    match activity {
        Activity::Walking(Goal::Dig(target)) | Activity::Digging { target, .. } => {
            reserved.dig.remove(target);
        }
        Activity::Walking(Goal::Guard(post)) | Activity::Guarding { post, .. } => {
            reserved.posts.remove(post);
        }
        Activity::Walking(Goal::Sleep(bed)) | Activity::Sleeping { bed } => reserved.free_bed(*bed),
        Activity::Walking(goal) => {
            if let Some(tile) = goal.spot() {
                reserved.free_spot(tile);
            }
        }
        Activity::Eating { tile, .. } | Activity::Gathering { tile, .. } => reserved.free_spot(*tile),
        _ => {}
    }
}

/// Schickt ein Kaninchen an der Oberfläche auf dem kürzesten Weg in den Bau.
/// Gibt `false` zurück, wenn es schon flüchtet, unter der Erde ist oder kein Eingang erreichbar ist.
pub fn start_flight(
    grid: &Grid,
    reserved: &mut Reserved,
    transform: &Transform,
    layer: &OnLayer,
    activity: &mut Activity,
    path: &mut Path,
    fear: &mut Fear,
) -> bool {
    if layer.0 != Layer::Surface || matches!(activity, Activity::Fleeing) {
        return false;
    }
    let start = current_node(transform, layer);
    let Some(steps) = find_path(grid, start, |node| node.layer == Layer::Underground) else {
        return false;
    };
    release(activity, reserved);
    path.0 = steps.into();
    *activity = Activity::Fleeing;
    fear.0 = FEAR_SECONDS;
    true
}

fn tick_needs(time: Res<Time>, mut rabbits: Query<(&Activity, &mut Hunger, &mut Fear)>) {
    let dt = time.delta_secs();
    for (activity, mut hunger, mut fear) in &mut rabbits {
        // Im Schlaf verbraucht ein Kaninchen weniger.
        // Im Schlaf und in der Winterruhe verbraucht ein Kaninchen viel weniger.
        let rate = if matches!(activity, Activity::Sleeping { .. }) {
            RESTING_HUNGER_FACTOR
        } else {
            1.0
        };
        hunger.0 = (hunger.0 + HUNGER_PER_SECOND * rate * dt).min(1.0);
        fear.0 = (fear.0 - dt).max(0.0);
    }
}

pub fn current_node(transform: &Transform, layer: &OnLayer) -> Node {
    Node::new(layer.0, TilePos::from_world_clamped(transform.translation.truncate()))
}

/// Was ein Kaninchen gerade über sich und die Welt weiß.
struct Situation<'a> {
    grid: &'a Grid,
    colony: &'a Colony,
    rabbit: &'a Rabbit,
    hunger: f32,
    afraid: bool,
    night: bool,
    /// Im Winter halten die Kaninchen Winterruhe in ihren Nestern.
    winter: bool,
    /// Im Herbst geht Sammeln vor Graben.
    autumn: bool,
    /// Der Spieler will Vorräte anlegen.
    gather_first: bool,
    food_space: f32,
    start: Node,
}

fn decide(
    time: Res<Time>,
    grid: Res<Grid>,
    colony: Res<Colony>,
    clock: Res<DayClock>,
    priorities: Res<Priorities>,
    mut rng: ResMut<Rng>,
    mut reserved: ResMut<Reserved>,
    mut rabbits: Query<(&Rabbit, &Hunger, &Fear, &Transform, &OnLayer, &mut Activity, &mut Path, &mut Carrying)>,
) {
    let food_space = Colony::food_capacity(&grid) - colony.food;

    for (rabbit, hunger, fear, transform, layer, mut activity, mut path, mut carrying) in &mut rabbits {
        let Activity::Idle(wait) = activity.as_mut() else {
            continue;
        };
        *wait -= time.delta_secs();
        if *wait > 0.0 {
            continue;
        }

        let situation = Situation {
            grid: &grid,
            colony: &colony,
            rabbit,
            hunger: hunger.0,
            afraid: fear.0 > 0.0,
            night: clock.is_night(),
            winter: clock.season() == Season::Winter,
            autumn: clock.season() == Season::Autumn,
            gather_first: priorities.gather_first,
            food_space,
            start: current_node(transform, layer),
        };
        match choose(&situation, &mut rng, &mut reserved, &mut carrying) {
            Some((goal, steps)) => {
                reserved.claim_goal(goal);
                path.0 = steps.into();
                *activity = Activity::Walking(goal);
            }
            None => *activity = Activity::Idle(rng.range(1.0, 2.5)),
        }
    }
}

fn choose(
    s: &Situation,
    rng: &mut Rng,
    reserved: &mut Reserved,
    carrying: &mut Carrying,
) -> Option<(Goal, Vec<Node>)> {
    let grid = s.grid;
    let start = s.start;

    if s.hunger > HUNGRY {
        // Im Bau, mit Angst und als Junges zuerst aus dem Vorrat; draußen lieber frisches Gras.
        let prefer_store = s.afraid || !s.rabbit.is_adult() || start.layer == Layer::Underground;
        let from_store = || {
            (s.colony.food >= 1.0)
                .then(|| find_storage(grid, reserved, start))
                .flatten()
                .map(|(tile, steps)| (Goal::EatFromStore(tile), steps))
        };
        if prefer_store && let Some(plan) = from_store() {
            return Some(plan);
        }
        if !s.afraid
            && let Some((tile, steps)) = find_grass(grid, reserved, start)
        {
            return Some((Goal::Eat(tile), steps));
        }
        if let Some(plan) = from_store() {
            return Some(plan);
        }
    }

    if (s.night || s.winter)
        && let Some((bed, steps)) = find_bed(grid, &reserved.beds, start)
    {
        *reserved.beds.entry(bed).or_default() += 1;
        return Some((Goal::Sleep(bed), steps));
    }

    if s.rabbit.is_adult() {
        if carrying.0 > 0.0 {
            if let Some((tile, steps)) = find_storage(grid, reserved, start) {
                return Some((Goal::Store(tile), steps));
            }
            // Kein Vorrat erreichbar: Futter fallen lassen.
            carrying.0 = 0.0;
        }
        if s.gather_first {
            if !s.afraid
                && s.food_space >= 1.0
                && let Some((tile, steps)) = find_grass(grid, reserved, start)
            {
                return Some((Goal::Gather(tile), steps));
            }
        } else if !s.afraid
            && !s.night
            && let Some((post, steps)) = find_free_post(grid, &reserved.posts, start)
        {
            reserved.posts.insert(post);
            return Some((Goal::Guard(post), steps));
        }
        if s.autumn
            && !s.afraid
            && s.food_space >= 1.0
            && let Some((tile, steps)) = find_grass(grid, reserved, start)
        {
            return Some((Goal::Gather(tile), steps));
        }
        if !s.gather_first
            && let Some((target, steps)) = find_dig_job(grid, &reserved.dig, start, s.afraid)
        {
            reserved.dig.insert(target);
            return Some((Goal::Dig(target), steps));
        }
        if !s.afraid
            && s.food_space >= 1.0
            && let Some((tile, steps)) = find_grass(grid, reserved, start)
        {
            return Some((Goal::Gather(tile), steps));
        }
    }

    // Nichts zu tun: ein Stück in der Nähe herumhoppeln, mit Angst nur unter der Erde.
    find_path(grid, start, |node| {
        node != start && (!s.afraid || node.layer == Layer::Underground) && rng.chance(0.08)
    })
    .map(|steps| (Goal::Wander, steps))
}

fn path_end(start: Node, steps: &[Node]) -> TilePos {
    steps.last().map_or(start.pos, |node| node.pos)
}

/// Das nächste Feld mit genug Gras, auf dem noch Platz zum Grasen ist.
fn find_grass(grid: &Grid, reserved: &Reserved, start: Node) -> Option<(TilePos, Vec<Node>)> {
    let steps = find_path(grid, start, |node| {
        let tile = grid.surface(node.pos);
        node.layer == Layer::Surface
            && !tile.entrance
            && !tile.post
            && tile.grass >= EDIBLE_GRASS
            && reserved.spot_free(node.pos)
    })?;
    Some((path_end(start, &steps), steps))
}

/// Das nächste Feld der Vorratskammer, an dem noch Platz ist.
fn find_storage(grid: &Grid, reserved: &Reserved, start: Node) -> Option<(TilePos, Vec<Node>)> {
    let steps = find_path(grid, start, |node| {
        let tile = grid.under(node.pos);
        node.layer == Layer::Underground
            && tile.chamber == Some(Chamber::Storage)
            && tile.dug.is_some()
            && reserved.spot_free(node.pos)
    })?;
    Some((path_end(start, &steps), steps))
}

/// Das nächste Nest mit freiem Platz.
fn find_bed(grid: &Grid, beds: &HashMap<TilePos, usize>, start: Node) -> Option<(TilePos, Vec<Node>)> {
    let steps = find_path(grid, start, |node| {
        let tile = grid.under(node.pos);
        node.layer == Layer::Underground
            && tile.chamber == Some(Chamber::Sleeping)
            && tile.dug.is_some()
            && beds.get(&node.pos).copied().unwrap_or(0) < SLEEP_PER_TILE
    })?;
    Some((path_end(start, &steps), steps))
}

fn find_free_post(grid: &Grid, taken: &HashSet<TilePos>, start: Node) -> Option<(TilePos, Vec<Node>)> {
    let steps = find_path(grid, start, |node| {
        node.layer == Layer::Surface && grid.surface(node.pos).post && !taken.contains(&node.pos)
    })?;
    Some((path_end(start, &steps), steps))
}

/// Sucht den nächsten freien Grabauftrag: Eingänge betritt man, Gänge gräbt man vom Nachbarfeld aus.
/// Mit Angst nur Aufträge unter der Erde.
fn find_dig_job(
    grid: &Grid,
    taken: &HashSet<DigTarget>,
    start: Node,
    underground_only: bool,
) -> Option<(DigTarget, Vec<Node>)> {
    let mut found = None;
    let steps = find_path(grid, start, |node| {
        found = match node.layer {
            Layer::Surface if underground_only => None,
            Layer::Surface => Some(DigTarget::Entrance(node.pos))
                .filter(|target| grid.surface(node.pos).entrance_planned && !taken.contains(target)),
            Layer::Underground => node.pos.neighbors().map(DigTarget::Tunnel).find(|target| {
                let DigTarget::Tunnel(p) = *target else { unreachable!() };
                let tile = grid.under(p);
                tile.dug.is_none() && tile.planned.is_some() && !taken.contains(target)
            }),
        };
        found.is_some()
    })?;
    Some((found?, steps))
}

fn still_planned(grid: &Grid, target: DigTarget) -> bool {
    match target {
        DigTarget::Tunnel(p) => grid.under(p).planned.is_some() && grid.under(p).dug.is_none(),
        DigTarget::Entrance(p) => grid.surface(p).entrance_planned,
    }
}

fn dig_seconds(grid: &Grid, target: DigTarget) -> f32 {
    match target {
        DigTarget::Tunnel(p) => grid.under(p).planned.unwrap_or(Depth::Shallow).dig_seconds(),
        DigTarget::Entrance(_) => Depth::Shallow.dig_seconds(),
    }
}

fn walk(
    time: Res<Time>,
    grid: Res<Grid>,
    mut colony: ResMut<Colony>,
    mut rng: ResMut<Rng>,
    mut reserved: ResMut<Reserved>,
    mut rabbits: Query<(
        &mut Transform,
        &mut OnLayer,
        &mut Path,
        &mut Activity,
        &mut Hunger,
        &mut Carrying,
        &Offset,
        &Rabbit,
    )>,
) {
    for (mut transform, mut layer, mut path, mut activity, mut hunger, mut carrying, offset, rabbit) in &mut rabbits
    {
        let speed = match *activity {
            Activity::Fleeing => FLEE_SPEED,
            // Sehr hungrige Kaninchen werden langsam.
            Activity::Walking(_) if hunger.0 > 0.95 => WALK_SPEED * 0.6,
            Activity::Walking(_) => WALK_SPEED,
            _ => continue,
        };
        // Alte Kaninchen hoppeln gemächlicher, auch auf der Flucht.
        let speed = if rabbit.is_old() { speed * 0.8 } else { speed };

        let mut budget = speed * time.delta_secs();
        while budget > 0.0 {
            let Some(next) = path.0.front().copied() else {
                break;
            };
            let target = next.pos.center() + offset.0;
            let position = transform.translation.truncate();
            let to_target = target - position;
            let distance = to_target.length();
            if distance <= budget {
                transform.translation = target.extend(z_for(target));
                layer.0 = next.layer;
                path.0.pop_front();
                budget -= distance;
            } else {
                let step = position + to_target / distance * budget;
                transform.translation = step.extend(z_for(step));
                budget = 0.0;
            }
        }
        if !path.0.is_empty() {
            continue;
        }

        let Activity::Walking(goal) = *activity else {
            // Angekommen im Bau: erst einmal still sitzen.
            *activity = Activity::Idle(rng.range(3.0, 6.0));
            continue;
        };
        *activity = match goal {
            Goal::Eat(tile) => Activity::Eating { tile, left: 2.5 },
            Goal::Gather(tile) => Activity::Gathering { tile, left: 2.0 },
            Goal::Dig(target) if still_planned(&grid, target) => Activity::Digging {
                target,
                left: dig_seconds(&grid, target),
            },
            Goal::Dig(target) => {
                reserved.dig.remove(&target);
                Activity::Idle(0.2)
            }
            Goal::Guard(post) if grid.surface(post).post => Activity::Guarding {
                post,
                left: GUARD_SHIFT_SECONDS,
            },
            Goal::Guard(post) => {
                reserved.posts.remove(&post);
                Activity::Idle(0.2)
            }
            Goal::Store(tile) => {
                reserved.free_spot(tile);
                let capacity = Colony::food_capacity(&grid);
                colony.food = (colony.food + carrying.0).min(capacity);
                carrying.0 = 0.0;
                Activity::Idle(0.5)
            }
            Goal::EatFromStore(tile) => {
                reserved.free_spot(tile);
                if colony.food >= 1.0 {
                    colony.food -= 1.0;
                    hunger.0 = (hunger.0 - 0.6).max(0.0);
                }
                Activity::Idle(1.0)
            }
            Goal::Sleep(bed) => Activity::Sleeping { bed },
            Goal::Wander => Activity::Idle(rng.range(1.0, 3.5)),
        };
    }
}

fn work(
    time: Res<Time>,
    clock: Res<DayClock>,
    mut grid: ResMut<Grid>,
    mut colony: ResMut<Colony>,
    mut rng: ResMut<Rng>,
    mut reserved: ResMut<Reserved>,
    mut rabbits: Query<(&Transform, &OnLayer, &mut Activity, &mut Path, &mut Hunger, &mut Carrying)>,
) {
    let dt = time.delta_secs();
    for (transform, layer, mut activity, mut path, mut hunger, mut carrying) in &mut rabbits {
        let next = match activity.as_mut() {
            Activity::Eating { tile, left } => {
                *left -= dt;
                if *left > 0.0 {
                    continue;
                }
                reserved.free_spot(*tile);
                let field = grid.surface_mut(*tile);
                let eaten = field.grass.min(0.5);
                field.grass -= eaten;
                hunger.0 = (hunger.0 - eaten * 1.3 * field.nutrition()).max(0.0);
                Activity::Idle(0.4)
            }
            Activity::Gathering { tile, left } => {
                *left -= dt;
                if *left > 0.0 {
                    continue;
                }
                reserved.free_spot(*tile);
                let field = grid.surface_mut(*tile);
                let taken = field.grass.min(0.5);
                field.grass -= taken;
                // Eingelagert und getrocknet ist Gras mehr wert als frisch gefressen.
                carrying.0 += taken * FOOD_PER_GATHERED_GRASS * field.nutrition();
                match find_storage(&grid, &reserved, current_node(transform, layer)) {
                    Some((store, steps)) => {
                        reserved.claim_spot(store);
                        path.0 = steps.into();
                        Activity::Walking(Goal::Store(store))
                    }
                    None => Activity::Idle(0.5),
                }
            }
            Activity::Digging { target, left } => {
                *left -= dt;
                if *left > 0.0 {
                    continue;
                }
                let target = *target;
                finish_digging(&mut grid, target);
                colony.earth += cost::EARTH_PER_TILE;
                reserved.dig.remove(&target);
                hunger.0 = (hunger.0 + 0.08).min(1.0);
                Activity::Idle(0.3)
            }
            Activity::Guarding { post, left } => {
                *left -= dt;
                // Wachen werden am Posten aus dem Vorrat versorgt, solange etwas da ist.
                if hunger.0 > HUNGRY && colony.food >= 1.0 {
                    colony.food -= 1.0;
                    hunger.0 = (hunger.0 - 0.6).max(0.0);
                }
                // Ist der Vorrat leer, beendet der Hunger die Schicht; Dunkelheit auch.
                if *left > 0.0 && !clock.is_night() && hunger.0 <= HUNGRY && grid.surface(*post).post {
                    continue;
                }
                reserved.posts.remove(post);
                Activity::Idle(0.2)
            }
            Activity::Sleeping { bed } => {
                // Hunger weckt jeden, sonst schlafen sie die Nacht und den Winter durch.
                let resting_time = clock.is_night() || clock.season() == Season::Winter;
                if resting_time && hunger.0 <= HUNGRY {
                    continue;
                }
                reserved.free_bed(*bed);
                if hunger.0 > HUNGRY {
                    Activity::Idle(0.0)
                } else {
                    // Nicht alle wachen gleichzeitig auf.
                    Activity::Idle(rng.range(0.0, 4.0))
                }
            }
            Activity::Idle(_) | Activity::Walking(_) | Activity::Fleeing => continue,
        };
        *activity = next;
    }
}

fn finish_digging(grid: &mut Grid, target: DigTarget) {
    match target {
        DigTarget::Tunnel(p) => {
            let tile = grid.under_mut(p);
            if tile.dug.is_none() {
                tile.dug = tile.planned.take();
            }
        }
        DigTarget::Entrance(p) => {
            let surface = grid.surface_mut(p);
            surface.entrance = true;
            surface.entrance_planned = false;
            surface.post = false;
            surface.grass = 0.0;
            let tile = grid.under_mut(p);
            tile.dug.get_or_insert(Depth::Shallow);
            tile.planned = None;
        }
    }
}
