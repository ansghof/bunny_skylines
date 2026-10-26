//! Der Marder: Nachts schleicht er zum nächsten Eingang und dringt in den Bau ein. Unter der Erde
//! sucht er schlafende Kaninchen. Eine Nachtwache stampft, sobald er in ihre Nähe kommt, und weckt
//! alle in den verbundenen Gängen; sie flüchten an ihm vorbei nach draußen. Erdpfropfen schließen
//! sich, solange er im Bau ist: Was dahinter liegt, erreicht er nicht.
//! Marder kommen erst, wenn die Familie zur „Siedlung“ gewachsen ist.

use std::collections::{HashMap, HashSet, VecDeque};

use bevy::prelude::*;

use crate::audio::{Sound, Sounds};
use crate::bird::Sky;
use crate::carry::{CarryPoint, carry_off};
use crate::behavior::{Activity, BehaviorSet, Fear, Goal, Path, Reserved, current_node, flee_from, release};
use crate::camera::{Orbit, figure_layer};
use crate::colony::{Messages, Rng};
use crate::daynight::DayClock;
use crate::ground::{floor_y, to_world3};
use crate::map::{Grid, Layer, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, TilePos, ViewLayer};
use crate::pathfinding::{Node, find_path, neighbors};
use crate::progress::{Progress, Stage};
use crate::rabbit::{Fur, OnLayer, Pregnant, Rabbit, Stomping};

const IMAGE: &str = "figuren/marder/marder.png";
const SIZE: Vec2 = Vec2::new(76.0, 38.0);
/// Etwas langsamer als ein hoppelndes Kaninchen: Wer gewarnt ist, entkommt.
const SPEED: f32 = 95.0;
const CATCH_DISTANCE: f32 = 22.0;
/// So oft sucht er sich im Bau ein neues Ziel.
const REPATH_SECONDS: f32 = 1.0;
/// Findet er so lange niemanden, gibt er auf.
const PATIENCE_SECONDS: f32 = 8.0;
/// Länger bleibt er nie.
const MAX_STAY_SECONDS: f32 = 60.0;
/// So viele Felder Gang weit hört und sieht die Nachtwache.
const WATCH_RANGE: usize = 10;
/// So weit läuft das Stampfen durch die Gänge.
const STOMP_RANGE: usize = 14;
/// Wache Kaninchen bemerken ihn selbst erst, wenn er so nah ist.
const NOTICE_DISTANCE: f32 = TILE_SIZE * 1.5;

pub struct MartenPlugin;

impl Plugin for MartenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MartenSpawner>()
            .add_systems(Startup, prepare_assets)
            .add_systems(
                Update,
                (assign_night_watch, spawn_martens, prowl, close_plugs, night_watch, notice_marten, show_martens)
                    .chain()
                    .after(BehaviorSet),
            );
        #[cfg(debug_assertions)]
        app.add_systems(Update, debug_summon_marten.before(spawn_martens));
    }
}

/// Nur zum Testen: F6 schickt sofort einen Marder, sobald es Nacht ist.
#[cfg(debug_assertions)]
fn debug_summon_marten(keys: Res<ButtonInput<KeyCode>>, mut spawner: ResMut<MartenSpawner>) {
    if keys.just_pressed(KeyCode::F6) {
        spawner.summon();
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hunt {
    /// Draußen auf dem Weg zum Eingang.
    Approaching,
    /// Im Bau auf der Suche.
    Inside,
    /// Auf dem Rückweg nach draußen und fort.
    Leaving,
}

#[derive(Component)]
pub struct Marten {
    hunt: Hunt,
    position: Vec2,
    layer: Layer,
    path: VecDeque<Node>,
    repath: f32,
    stay: f32,
    no_prey: f32,
    /// Ob eine Nachtwache ihn schon gemeldet hat.
    spotted: bool,
    velocity: Vec2,
}

impl Marten {
    fn node(&self) -> Node {
        Node::new(self.layer, TilePos::from_world_clamped(self.position))
    }
}

#[derive(Component)]
struct MartenImage(Entity);

#[derive(Resource)]
pub struct MartenSpawner {
    /// Sekunden Nacht bis zum nächsten Marder.
    next: f32,
    forced: bool,
    warned: bool,
}

impl Default for MartenSpawner {
    fn default() -> Self {
        Self { next: 60.0, forced: false, warned: false }
    }
}

impl MartenSpawner {
    /// In der nächsten Nacht sofort einen Marder schicken, auch wenn die Familie noch klein ist.
    pub fn summon(&mut self) {
        // Ein paar Sekunden Nacht, damit die Nachtwache noch ihren Platz beziehen kann.
        self.next = 6.0;
        self.forced = true;
    }
}

#[derive(Resource)]
struct MartenAssets {
    quad: Handle<Mesh>,
    image: Handle<StandardMaterial>,
}

fn prepare_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(MartenAssets {
        // Unterkante am Ursprung, damit er auf dem Gangboden steht.
        quad: meshes.add(Mesh::from(Rectangle::new(1.0, 1.0)).translated_by(Vec3::new(0.0, 0.5, 0.0))),
        image: materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load(IMAGE)),
            alpha_mode: AlphaMode::AlphaToCoverage,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
    });
}

fn half_extent() -> Vec2 {
    Vec2::new(MAP_WIDTH as f32, MAP_HEIGHT as f32) * TILE_SIZE / 2.0
}

/// Nachts zieht ein erwachsenes Kaninchen auf jeden freien Nachtwachen-Platz.
fn assign_night_watch(
    time: Res<Time>,
    clock: Res<DayClock>,
    grid: Res<Grid>,
    mut reserved: ResMut<Reserved>,
    mut rabbits: Query<(&Rabbit, &Transform, &OnLayer, &mut Activity, &mut Path), Without<Pregnant>>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer < 2.0 || !clock.is_night() {
        return;
    }
    *timer = 0.0;
    let watches: Vec<TilePos> = Grid::positions()
        .filter(|p| grid.under(*p).watch && grid.under(*p).dug.is_some() && !reserved.post_taken(*p))
        .collect();
    for post in watches {
        let goal = Node::new(Layer::Underground, post);
        let candidate = rabbits.iter_mut().find(|(rabbit, _, layer, activity, _)| {
            rabbit.is_adult()
                && layer.0 == Layer::Underground
                && matches!(**activity, Activity::Sleeping { .. } | Activity::Idle(_))
        });
        let Some((_, transform, layer, mut activity, mut path)) = candidate else {
            return;
        };
        let Some(steps) = find_path(&grid, current_node(transform, layer), |node| node == goal) else {
            continue;
        };
        release(&activity, &mut reserved);
        reserved.take_post(post);
        path.0 = steps.into();
        *activity = Activity::Walking(Goal::Guard(post));
    }
}

fn spawn_martens(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<DayClock>,
    progress: Res<Progress>,
    grid: Res<Grid>,
    assets: Res<MartenAssets>,
    mut spawner: ResMut<MartenSpawner>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    mut sounds: ResMut<Sounds>,
    martens: Query<(), With<Marten>>,
    rabbits: Query<&OnLayer, With<Rabbit>>,
) {
    let grown = progress.stage >= Stage::Settlement || spawner.forced;
    let anyone_inside = rabbits.iter().any(|layer| layer.0 == Layer::Underground);
    if !grown || !clock.is_night() || !martens.is_empty() || !anyone_inside {
        return;
    }
    spawner.next -= time.delta_secs();
    if spawner.next > 0.0 {
        return;
    }
    spawner.next = rng.range(60.0, 150.0);
    spawner.forced = false;

    // Von einem zufälligen Punkt am Rand aus zum nächsten Eingang.
    let half = half_extent() - TILE_SIZE * 0.5;
    let along = rng.range(-1.0, 1.0);
    let position = match rng.below(4) {
        0 => Vec2::new(-half.x, along * half.y),
        1 => Vec2::new(half.x, along * half.y),
        2 => Vec2::new(along * half.x, -half.y),
        _ => Vec2::new(along * half.x, half.y),
    };
    let start = Node::new(Layer::Surface, TilePos::from_world_clamped(position));
    let Some(steps) = find_path(&grid, start, |node| node.layer == Layer::Underground) else {
        return;
    };
    let marten = commands
        .spawn((Sky, CarryPoint::default(), Marten {
            hunt: Hunt::Approaching,
            position,
            layer: Layer::Surface,
            path: steps.into(),
            repath: 0.0,
            stay: 0.0,
            no_prey: 0.0,
            spotted: false,
            velocity: Vec2::ZERO,
        }))
        .id();
    commands.spawn((
        Sky,
        MartenImage(marten),
        Mesh3d(assets.quad.clone()),
        MeshMaterial3d(assets.image.clone()),
        Transform::default(),
        figure_layer(),
    ));
    sounds.play(Sound::Fox);
    if spawner.warned {
        messages.push("Etwas schleicht nachts um den Bau …");
    } else {
        spawner.warned = true;
        messages.push("Etwas schleicht nachts um den Bau … Nachtwachen und Erdpfropfen helfen gegen Marder.");
    }
}

fn prowl(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    mut messages: ResMut<Messages>,
    mut sounds: ResMut<Sounds>,
    mut reserved: ResMut<Reserved>,
    mut martens: Query<(Entity, &mut Marten)>,
    rabbits: Query<(Entity, &Rabbit, &Transform, &OnLayer, &Activity, &Fur)>,
    images: Query<(Entity, &MartenImage)>,
) {
    let dt = time.delta_secs();
    for (entity, mut marten) in &mut martens {
        marten.stay += dt;
        if marten.stay > MAX_STAY_SECONDS && marten.hunt == Hunt::Inside {
            leave(&grid, &mut marten);
        }

        if marten.hunt == Hunt::Inside {
            // Erwischt er jemanden?
            let caught = rabbits.iter().find(|(_, _, t, layer, activity, _)| {
                layer.0 == Layer::Underground
                    && !matches!(activity, Activity::Gone)
                    && t.translation.truncate().distance(marten.position) < CATCH_DISTANCE
            });
            if let Some((prey, rabbit, _, _, activity, fur)) = caught {
                release(activity, &mut reserved);
                commands.entity(prey).despawn();
                carry_off(&mut commands, entity, fur.0);
                messages.push(format!("{} wurde vom Marder geholt.", rabbit.name));
                sounds.play(Sound::Caught);
                leave(&grid, &mut marten);
            } else {
                marten.repath -= dt;
                if marten.repath <= 0.0 {
                    marten.repath = REPATH_SECONDS;
                    let prey: HashSet<Node> = rabbits
                        .iter()
                        .filter(|(_, _, _, layer, activity, _)| {
                            layer.0 == Layer::Underground && !matches!(activity, Activity::Gone)
                        })
                        .map(|(_, _, t, layer, _, _)| current_node(t, layer))
                        .collect();
                    match find_path(&grid, marten.node(), |node| prey.contains(&node)) {
                        Some(steps) => {
                            marten.no_prey = 0.0;
                            marten.path = steps.into();
                        }
                        None => {
                            marten.no_prey += REPATH_SECONDS;
                            marten.path.clear();
                            if marten.no_prey > PATIENCE_SECONDS {
                                leave(&grid, &mut marten);
                            }
                        }
                    }
                }
            }
        }

        // Dem Weg folgen.
        let mut budget = SPEED * dt;
        let before = marten.position;
        while budget > 0.0 {
            let Some(next) = marten.path.front().copied() else {
                break;
            };
            let target = next.pos.center();
            let distance = target.distance(marten.position);
            if distance <= budget {
                marten.position = target;
                marten.layer = next.layer;
                marten.path.pop_front();
                budget -= distance;
            } else {
                let step = (target - marten.position) / distance * budget;
                marten.position += step;
                budget = 0.0;
            }
        }
        // Draußen und ohne Weg: geradewegs zum Kartenrand.
        if marten.hunt == Hunt::Leaving && marten.path.is_empty() && marten.layer == Layer::Surface {
            let exit = nearest_exit(marten.position);
            let step = (exit - marten.position).clamp_length_max(budget.max(SPEED * dt));
            marten.position += step;
            if marten.position.distance(exit) < 4.0 {
                for (image, owner) in &images {
                    if owner.0 == entity {
                        commands.entity(image).despawn();
                    }
                }
                commands.entity(entity).despawn();
                messages.push("Der Marder ist wieder fort.");
                continue;
            }
        }
        marten.velocity = (marten.position - before) / dt.max(1e-4);

        if marten.hunt == Hunt::Approaching && marten.layer == Layer::Underground {
            marten.hunt = Hunt::Inside;
            marten.repath = 0.0;
            messages.push("Ein Marder ist in den Bau eingedrungen!");
        }
    }
}

/// Auf dem kürzesten Weg nach draußen.
fn leave(grid: &Grid, marten: &mut Marten) {
    marten.hunt = Hunt::Leaving;
    marten.path = find_path(grid, marten.node(), |node| node.layer == Layer::Surface)
        .unwrap_or_default()
        .into();
}

fn nearest_exit(position: Vec2) -> Vec2 {
    let half = half_extent() + TILE_SIZE;
    [
        Vec2::new(-half.x, position.y),
        Vec2::new(half.x, position.y),
        Vec2::new(position.x, -half.y),
        Vec2::new(position.x, half.y),
    ]
    .into_iter()
    .min_by(|a, b| a.distance(position).total_cmp(&b.distance(position)))
    .unwrap()
}

/// Solange ein Marder im Bau ist, sind alle Erdpfropfen geschlossen.
fn close_plugs(mut grid: ResMut<Grid>, mut messages: ResMut<Messages>, martens: Query<&Marten>) {
    let inside = martens.iter().any(|m| m.hunt == Hunt::Inside);
    if grid.plugs_closed == inside {
        return;
    }
    let any_plug = Grid::positions().any(|p| grid.under(p).plug);
    grid.plugs_closed = inside;
    if any_plug {
        messages.push(if inside {
            "Die Erdpfropfen werden mit Erde verschlossen!"
        } else {
            "Die Erdpfropfen sind wieder offen."
        });
    }
}

/// Wie weit jedes Gangfeld vom Start entfernt ist, höchstens `range` Felder.
fn tunnel_distances(grid: &Grid, start: Node, range: usize) -> HashMap<TilePos, usize> {
    let mut distance = HashMap::from([(start.pos, 0)]);
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        let d = distance[&node.pos];
        if d >= range {
            continue;
        }
        for next in neighbors(grid, node).filter(|n| n.layer == Layer::Underground) {
            if let std::collections::hash_map::Entry::Vacant(entry) = distance.entry(next.pos) {
                entry.insert(d + 1);
                queue.push_back(next);
            }
        }
    }
    distance
}

/// Kommt der Marder in Reichweite einer Nachtwache, stampft sie. Das Stampfen läuft durch die Gänge
/// und weckt alle in den verbundenen Kammern; sie flüchten an ihm vorbei hinaus.
fn night_watch(
    mut commands: Commands,
    grid: Res<Grid>,
    mut reserved: ResMut<Reserved>,
    mut messages: ResMut<Messages>,
    mut martens: Query<&mut Marten>,
    mut rabbits: Query<(Entity, &Rabbit, &Transform, &OnLayer, &mut Activity, &mut Path, &mut Fear)>,
) {
    for mut marten in &mut martens {
        if marten.spotted || marten.hunt != Hunt::Inside {
            continue;
        }
        let near = tunnel_distances(&grid, marten.node(), WATCH_RANGE);
        let watcher = rabbits.iter().find(|(_, _, t, layer, activity, ..)| {
            layer.0 == Layer::Underground
                && matches!(**activity, Activity::Guarding { .. })
                && near.contains_key(&current_node(t, layer).pos)
        });
        let Some((guard, rabbit, ..)) = watcher else {
            continue;
        };
        marten.spotted = true;
        commands.entity(guard).insert(Stomping(0.6));
        messages.push(format!("{} stampft: Ein Marder im Bau!", rabbit.name));
        let heard = tunnel_distances(&grid, marten.node(), STOMP_RANGE);
        for (_, _, transform, layer, mut activity, mut path, mut fear) in &mut rabbits {
            if layer.0 == Layer::Underground && heard.contains_key(&current_node(transform, layer).pos) {
                flee_from(&grid, &mut reserved, transform, layer, &mut activity, &mut path, &mut fear, marten.position);
            }
        }
    }
}

/// Wer wach ist, bemerkt den Marder, wenn er ganz nah ist. Schlafende merken nichts.
fn notice_marten(
    grid: Res<Grid>,
    mut reserved: ResMut<Reserved>,
    martens: Query<&Marten>,
    mut rabbits: Query<(&Transform, &OnLayer, &mut Activity, &mut Path, &mut Fear), With<Rabbit>>,
) {
    for marten in &martens {
        if marten.hunt != Hunt::Inside {
            continue;
        }
        for (transform, layer, mut activity, mut path, mut fear) in &mut rabbits {
            let awake = !matches!(*activity, Activity::Sleeping { .. });
            let close = transform.translation.truncate().distance(marten.position) < NOTICE_DISTANCE;
            if awake && close && layer.0 == Layer::Underground {
                flee_from(&grid, &mut reserved, transform, layer, &mut activity, &mut path, &mut fear, marten.position);
            }
        }
    }
}

fn show_martens(
    grid: Res<Grid>,
    orbit: Res<Orbit>,
    view: Res<ViewLayer>,
    mut martens: Query<(&Marten, &mut CarryPoint)>,
    mut images: Query<(&MartenImage, &mut Transform, &mut Visibility)>,
) {
    let right = to_world3(orbit.screen_right(), 0.0);
    for (image, mut transform, mut shown) in &mut images {
        let Ok((marten, mut point)) = martens.get_mut(image.0) else {
            continue;
        };
        let floor = floor_y(&grid, marten.layer, TilePos::from_world_clamped(marten.position));
        transform.translation = to_world3(marten.position, floor);
        transform.rotation = orbit.billboard();
        // Nach links oder rechts schauen, je nachdem wohin er auf dem Bildschirm läuft.
        let sideways = marten.velocity.dot(orbit.screen_right());
        let facing = if sideways < -0.1 { -1.0 } else if sideways > 0.1 { 1.0 } else { transform.scale.x.signum() };
        let facing = if facing == 0.0 { 1.0 } else { facing };
        transform.scale = Vec3::new(SIZE.x * facing, SIZE.y, 1.0);
        *shown = if marten.layer == view.0 { Visibility::Inherited } else { Visibility::Hidden };
        // Die Beute im Maul, vorn am Kopf.
        point.at = to_world3(marten.position, floor + 22.0) + right * facing * 34.0;
        point.facing = facing;
        point.visible = marten.layer == view.0;
    }
}
