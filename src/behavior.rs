//! Was Kaninchen tun: Hunger stillen, Aufträge graben, Futter in den Vorrat tragen, herumhoppeln.
//! Jedes Kaninchen entscheidet selbst, sobald es mit seiner aktuellen Tätigkeit fertig ist.

use std::collections::{HashSet, VecDeque};

use bevy::prelude::*;

use crate::colony::{Colony, Rng};
use crate::map::{Chamber, Depth, Grid, Layer, TilePos};
use crate::pathfinding::{Node, find_path};
use crate::rabbit::{Hunger, Offset, OnLayer, Rabbit, z_for};

const HUNGER_PER_SECOND: f32 = 0.012;
const HUNGRY: f32 = 0.55;
const WALK_SPEED: f32 = 110.0;
/// Ab so viel Gras lohnt sich ein Feld zum Fressen.
const EDIBLE_GRASS: f32 = 0.3;

pub struct BehaviorPlugin;

impl Plugin for BehaviorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Reserved>()
            .add_systems(Update, (get_hungry, decide, walk, work).chain());
    }
}

#[derive(Component)]
pub enum Activity {
    /// Wartet noch so viele Sekunden, dann entscheidet es neu.
    Idle(f32),
    Walking(Goal),
    Eating { tile: TilePos, left: f32 },
    Gathering { tile: TilePos, left: f32 },
    Digging { target: DigTarget, left: f32 },
}

#[derive(Clone, Copy, Debug)]
pub enum Goal {
    Eat(TilePos),
    Gather(TilePos),
    Dig(DigTarget),
    Store,
    EatFromStore,
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

/// Grabaufträge, die schon ein Kaninchen übernommen hat.
#[derive(Resource, Default)]
struct Reserved(HashSet<DigTarget>);

fn get_hungry(time: Res<Time>, mut rabbits: Query<&mut Hunger>) {
    for mut hunger in &mut rabbits {
        hunger.0 = (hunger.0 + HUNGER_PER_SECOND * time.delta_secs()).min(1.0);
    }
}

fn current_node(transform: &Transform, layer: &OnLayer) -> Node {
    Node::new(layer.0, TilePos::from_world_clamped(transform.translation.truncate()))
}

fn decide(
    time: Res<Time>,
    grid: Res<Grid>,
    colony: Res<Colony>,
    mut rng: ResMut<Rng>,
    mut reserved: ResMut<Reserved>,
    mut rabbits: Query<(&Rabbit, &Hunger, &Transform, &OnLayer, &mut Activity, &mut Path, &mut Carrying)>,
) {
    let food_space = Colony::food_capacity(&grid) - colony.food;

    for (rabbit, hunger, transform, layer, mut activity, mut path, mut carrying) in &mut rabbits {
        let Activity::Idle(wait) = activity.as_mut() else {
            continue;
        };
        *wait -= time.delta_secs();
        if *wait > 0.0 {
            continue;
        }

        let start = current_node(transform, layer);
        let plan = choose(&grid, &colony, &mut rng, &mut reserved, rabbit, hunger.0, &mut carrying, food_space, start);
        match plan {
            Some((goal, steps)) => {
                path.0 = steps.into();
                *activity = Activity::Walking(goal);
            }
            None => *activity = Activity::Idle(rng.range(1.0, 2.5)),
        }
    }
}

fn choose(
    grid: &Grid,
    colony: &Colony,
    rng: &mut Rng,
    reserved: &mut Reserved,
    rabbit: &Rabbit,
    hunger: f32,
    carrying: &mut Carrying,
    food_space: f32,
    start: Node,
) -> Option<(Goal, Vec<Node>)> {
    if hunger > HUNGRY {
        if let Some((tile, steps)) = find_grass(grid, start) {
            return Some((Goal::Eat(tile), steps));
        }
        if colony.food >= 1.0
            && let Some(steps) = find_storage(grid, start)
        {
            return Some((Goal::EatFromStore, steps));
        }
    }

    if rabbit.is_adult() {
        if carrying.0 > 0.0 {
            if let Some(steps) = find_storage(grid, start) {
                return Some((Goal::Store, steps));
            }
            // Kein Vorrat erreichbar: Futter fallen lassen.
            carrying.0 = 0.0;
        }
        if let Some((target, steps)) = find_dig_job(grid, &reserved.0, start) {
            reserved.0.insert(target);
            return Some((Goal::Dig(target), steps));
        }
        if food_space >= 1.0
            && let Some((tile, steps)) = find_grass(grid, start)
        {
            return Some((Goal::Gather(tile), steps));
        }
    }

    // Nichts zu tun: ein Stück in der Nähe herumhoppeln.
    find_path(grid, start, |node| node != start && rng.chance(0.08)).map(|steps| (Goal::Wander, steps))
}

fn path_end(start: Node, steps: &[Node]) -> TilePos {
    steps.last().map_or(start.pos, |node| node.pos)
}

fn find_grass(grid: &Grid, start: Node) -> Option<(TilePos, Vec<Node>)> {
    let steps = find_path(grid, start, |node| {
        let tile = grid.surface(node.pos);
        node.layer == Layer::Surface && !tile.entrance && tile.grass >= EDIBLE_GRASS
    })?;
    Some((path_end(start, &steps), steps))
}

fn find_storage(grid: &Grid, start: Node) -> Option<Vec<Node>> {
    find_path(grid, start, |node| {
        let tile = grid.under(node.pos);
        node.layer == Layer::Underground && tile.chamber == Some(Chamber::Storage) && tile.dug.is_some()
    })
}

/// Sucht den nächsten freien Grabauftrag: Eingänge betritt man, Gänge gräbt man vom Nachbarfeld aus.
fn find_dig_job(grid: &Grid, reserved: &HashSet<DigTarget>, start: Node) -> Option<(DigTarget, Vec<Node>)> {
    let mut found = None;
    let steps = find_path(grid, start, |node| {
        found = match node.layer {
            Layer::Surface => Some(DigTarget::Entrance(node.pos))
                .filter(|target| grid.surface(node.pos).entrance_planned && !reserved.contains(target)),
            Layer::Underground => node.pos.neighbors().map(DigTarget::Tunnel).find(|target| {
                let DigTarget::Tunnel(p) = *target else { unreachable!() };
                let tile = grid.under(p);
                tile.dug.is_none() && tile.planned.is_some() && !reserved.contains(target)
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
    )>,
) {
    for (mut transform, mut layer, mut path, mut activity, mut hunger, mut carrying, offset) in &mut rabbits {
        let Activity::Walking(goal) = *activity else {
            continue;
        };

        // Sehr hungrige Kaninchen werden langsam.
        let speed = if hunger.0 > 0.95 { WALK_SPEED * 0.6 } else { WALK_SPEED };
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

        *activity = match goal {
            Goal::Eat(tile) => Activity::Eating { tile, left: 2.5 },
            Goal::Gather(tile) => Activity::Gathering { tile, left: 2.0 },
            Goal::Dig(target) if still_planned(&grid, target) => Activity::Digging {
                target,
                left: dig_seconds(&grid, target),
            },
            Goal::Dig(target) => {
                reserved.0.remove(&target);
                Activity::Idle(0.2)
            }
            Goal::Store => {
                let capacity = Colony::food_capacity(&grid);
                colony.food = (colony.food + carrying.0).min(capacity);
                carrying.0 = 0.0;
                Activity::Idle(0.5)
            }
            Goal::EatFromStore => {
                if colony.food >= 1.0 {
                    colony.food -= 1.0;
                    hunger.0 = (hunger.0 - 0.6).max(0.0);
                }
                Activity::Idle(1.0)
            }
            Goal::Wander => Activity::Idle(rng.range(1.0, 3.5)),
        };
    }
}

fn work(
    time: Res<Time>,
    mut grid: ResMut<Grid>,
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
                let grass = &mut grid.surface_mut(*tile).grass;
                let eaten = grass.min(0.5);
                *grass -= eaten;
                hunger.0 = (hunger.0 - eaten * 1.3).max(0.0);
                Activity::Idle(0.4)
            }
            Activity::Gathering { tile, left } => {
                *left -= dt;
                if *left > 0.0 {
                    continue;
                }
                let grass = &mut grid.surface_mut(*tile).grass;
                let taken = grass.min(0.5);
                *grass -= taken;
                carrying.0 += taken * 2.0;
                match find_storage(&grid, current_node(transform, layer)) {
                    Some(steps) => {
                        path.0 = steps.into();
                        Activity::Walking(Goal::Store)
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
                reserved.0.remove(&target);
                hunger.0 = (hunger.0 + 0.08).min(1.0);
                Activity::Idle(0.3)
            }
            Activity::Idle(_) | Activity::Walking(_) => continue,
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
            surface.grass = 0.0;
            let tile = grid.under_mut(p);
            tile.dug.get_or_insert(Depth::Shallow);
            tile.planned = None;
        }
    }
}
