//! Der Bussard: Tagsüber kreist er hoch über der Wiese, sein Schatten im Sonnenlicht verrät ihn. Dann stößt er
//! auf ein Kaninchen im Freien herab. Sicher ist, wer im Bau, in einem Fluchtloch oder unter einer
//! Hecke sitzt. Er kommt erst, wenn die Familie zum „Bau“ gewachsen ist.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::behavior::{Activity, BehaviorSet, Fear, Path, Reserved, current_node, is_hidden, release, start_flight};
use crate::camera::Orbit;
use crate::carry::{CarryPoint, carry_off};
use crate::audio::{Sound, Sounds};
use crate::colony::{Messages, Rng};
use crate::daynight::DayClock;
use crate::danger::{Alarms, raise_alarm};
use crate::ground::to_world3;
use crate::map::{Feature, Grid, Layer, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, ViewLayer};
use crate::progress::{Progress, Stage};
use crate::rabbit::{Fur, OnLayer, Rabbit, Stomping};

const IMAGE: &str = "figuren/bussard/bussard.png";
/// Größe des Vogels in der Welt.
const SIZE: Vec2 = Vec2::new(110.0, 55.0);
/// So hoch kreist er.
const CIRCLE_HEIGHT: f32 = 200.0;
const CIRCLE_RADIUS: f32 = TILE_SIZE * 2.5;
/// Eine Runde dauert so lange.
const CIRCLE_SECONDS: f32 = 7.0;
/// So lange kreist er, bevor er zustößt.
const CIRCLING_SECONDS: (f32, f32) = (12.0, 20.0);
/// Am Boden zurückgelegte Strecke pro Sekunde im Sturzflug.
const DIVE_SPEED: f32 = 240.0;
const DIVE_SECONDS_MIN: f32 = 1.6;
const LEAVE_SPEED: f32 = 160.0;
/// So weit sucht er von seinem Kreis aus nach Beute.
const HUNT_RANGE: f32 = TILE_SIZE * 9.0;
/// So weit sieht eine Wache einen kreisenden Bussard.
const GUARD_SIGHT: f32 = TILE_SIZE * 7.0;
/// So weit hört man das Stampfen der Wache.
const STOMP_RADIUS: f32 = TILE_SIZE * 7.0;
/// Wer so nah am Ziel ist, bemerkt den Sturzflug vielleicht.
const NOTICE_RADIUS: f32 = TILE_SIZE * 3.0;
const NOTICE_CHANCE: f32 = 0.5;
/// Nach so vielen leeren Runden zieht er weiter.
const MAX_TRIES: u32 = 3;

pub struct BirdPlugin;

impl Plugin for BirdPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuzzardSpawner>()
            .add_systems(Startup, prepare_assets)
            .add_systems(Update, (spawn_buzzards, fly, guards_watch_sky, show_buzzards).chain().after(BehaviorSet));
        #[cfg(debug_assertions)]
        app.add_systems(Update, debug_summon_buzzard.before(spawn_buzzards));
    }
}

/// Nur zum Testen: F7 schickt sofort einen Bussard, auch wenn die Familie noch klein ist.
#[cfg(debug_assertions)]
fn debug_summon_buzzard(keys: Res<ButtonInput<KeyCode>>, mut spawner: ResMut<BuzzardSpawner>) {
    if keys.just_pressed(KeyCode::F7) {
        spawner.summon();
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Flight {
    Circling { left: f32 },
    Diving { target: Entity },
    Leaving,
}

/// Feinde mit allem, was zu ihnen gehört (Bussard, Marder und ihre Bilder); wird beim Neubeginn weggeräumt.
#[derive(Component)]
pub struct Sky;

#[derive(Component)]
pub struct Buzzard {
    flight: Flight,
    /// Wo er gerade über dem Boden ist.
    position: Vec2,
    height: f32,
    center: Vec2,
    angle: f32,
    tries: u32,
    /// Ob die Wachen ihn schon gemeldet haben.
    spotted: bool,
}

#[derive(Component)]
struct BuzzardImage(Entity);

#[derive(Resource)]
pub struct BuzzardSpawner {
    next: f32,
    /// Auch ohne große Familie kommen lassen (nur für Tests).
    forced: bool,
    warned: bool,
}

impl Default for BuzzardSpawner {
    fn default() -> Self {
        Self { next: 120.0, forced: false, warned: false }
    }
}

impl BuzzardSpawner {
    /// Gleich einen Bussard schicken, unabhängig von der Größe der Familie.
    pub fn summon(&mut self) {
        self.next = 0.0;
        self.forced = true;
    }
}

#[derive(Resource)]
struct BirdAssets {
    quad: Handle<Mesh>,
    image: Handle<StandardMaterial>,
}

fn prepare_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(BirdAssets {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
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

/// Wo ein Kaninchen sicher vor Angriffen aus der Luft ist.
fn covered(grid: &Grid, transform: &Transform, layer: &OnLayer) -> bool {
    let node = current_node(transform, layer);
    layer.0 != Layer::Surface || is_hidden(grid, node) || grid.surface(node.pos).feature == Some(Feature::Hedge)
}

fn spawn_buzzards(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<DayClock>,
    progress: Res<Progress>,
    assets: Res<BirdAssets>,
    mut spawner: ResMut<BuzzardSpawner>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    mut sounds: ResMut<Sounds>,
    buzzards: Query<(), With<Buzzard>>,
    rabbits: Query<(&Transform, &OnLayer), With<Rabbit>>,
) {
    // Bussarde jagen am hellen Tag, und erst, wenn die Familie größer geworden ist.
    let grown = progress.stage >= Stage::Warren || spawner.forced;
    let daylight = !clock.is_night() && !clock.is_twilight();
    if !grown || !daylight || !buzzards.is_empty() || rabbits.is_empty() {
        return;
    }
    spawner.next -= time.delta_secs();
    if spawner.next > 0.0 {
        return;
    }
    spawner.next = rng.range(150.0, 260.0);
    spawner.forced = false;

    // Er kreist dort, wo Kaninchen draußen sind, sonst über der Kartenmitte.
    let outside: Vec<Vec2> = rabbits
        .iter()
        .filter(|(_, layer)| layer.0 == Layer::Surface)
        .map(|(t, _)| t.translation.truncate())
        .collect();
    let center = if outside.is_empty() {
        Vec2::ZERO
    } else {
        outside[rng.below(outside.len())]
    };
    // Er kommt vom Rand hereingeflogen.
    let position = Vec2::new(-half_extent().x - TILE_SIZE, center.y);
    let buzzard = commands
        .spawn((
            Sky,
            CarryPoint::default(),
            Buzzard {
                flight: Flight::Circling { left: rng.range(CIRCLING_SECONDS.0, CIRCLING_SECONDS.1) },
                position,
                height: CIRCLE_HEIGHT,
                center,
                angle: 0.0,
                tries: 0,
                spotted: false,
            },
        ))
        .id();
    commands.spawn((
        Sky,
        BuzzardImage(buzzard),
        Mesh3d(assets.quad.clone()),
        MeshMaterial3d(assets.image.clone()),
        Transform::default(),
    ));
    sounds.play(Sound::Buzzard);
    if spawner.warned {
        messages.push("Ein Bussard kreist über der Wiese!");
    } else {
        spawner.warned = true;
        messages.push("Ein Bussard kreist über der Wiese! In Hecken, Fluchtlöchern und im Bau sind die Kaninchen sicher.");
    }
}

fn fly(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    mut reserved: ResMut<Reserved>,
    mut sounds: ResMut<Sounds>,
    mut buzzards: Query<(Entity, &mut Buzzard)>,
    mut rabbits: Query<
        (Entity, &Rabbit, &Transform, &OnLayer, &mut Activity, &mut Path, &mut Fear, &Fur),
        Without<Buzzard>,
    >,
    images: Query<(Entity, &BuzzardImage)>,
) {
    let dt = time.delta_secs();
    for (entity, mut bird) in &mut buzzards {
        match bird.flight {
            Flight::Circling { left } => {
                bird.angle = (bird.angle + dt * TAU / CIRCLE_SECONDS).rem_euclid(TAU);
                let goal = bird.center + Vec2::from_angle(bird.angle) * CIRCLE_RADIUS;
                // Gleitet auf seinen Kreis zu, statt hinzuspringen.
                let step = (goal - bird.position).clamp_length_max(LEAVE_SPEED * dt);
                bird.position += step;
                bird.height += (CIRCLE_HEIGHT - bird.height).clamp(-60.0 * dt, 60.0 * dt);
                let left = left - dt;
                if left > 0.0 {
                    bird.flight = Flight::Circling { left };
                    continue;
                }
                // Beute suchen: das nächste ungeschützte Kaninchen in Reichweite.
                let prey = rabbits
                    .iter()
                    .filter(|(_, _, t, layer, ..)| !covered(&grid, t, layer))
                    .map(|(e, _, t, ..)| (e, t.translation.truncate().distance(bird.position)))
                    .filter(|(_, d)| *d < HUNT_RANGE)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                match prey {
                    Some((target, _)) => {
                        bird.flight = Flight::Diving { target };
                        sounds.play(Sound::Buzzard);
                        warn_neighbors(&grid, &mut reserved, &mut rng, &mut rabbits, target);
                    }
                    None => give_up_or_circle(&mut bird, &mut rng, &rabbits),
                }
            }
            Flight::Diving { target } => {
                let Ok((_, prey, at, layer, .., fur)) = rabbits.get(target) else {
                    give_up_or_circle(&mut bird, &mut rng, &rabbits);
                    continue;
                };
                let at_ground = at.translation.truncate();
                let distance = at_ground.distance(bird.position);
                // Höhe und Strecke kommen gemeinsam auf null.
                let time_left = (distance / DIVE_SPEED).max(DIVE_SECONDS_MIN * bird.height / CIRCLE_HEIGHT).max(dt);
                let share = (dt / time_left).min(1.0);
                let step = (at_ground - bird.position) * share;
                bird.position += step;
                let drop = bird.height * share;
                bird.height -= drop;
                if bird.height > 8.0 {
                    continue;
                }
                if covered(&grid, at, layer) {
                    messages.push("Der Bussard greift ins Leere.");
                    give_up_or_circle(&mut bird, &mut rng, &rabbits);
                } else {
                    let name = prey.name.clone();
                    let fur = fur.0;
                    if let Ok((_, _, _, _, activity, ..)) = rabbits.get(target) {
                        release(activity, &mut reserved);
                    }
                    commands.entity(target).despawn();
                    carry_off(&mut commands, entity, fur);
                    messages.push(format!("{name} wurde vom Bussard geschnappt."));
                    sounds.play(Sound::Caught);
                    bird.flight = Flight::Leaving;
                }
            }
            Flight::Leaving => {
                bird.height += 80.0 * dt;
                let exit = Vec2::new(half_extent().x + TILE_SIZE * 3.0, bird.position.y);
                let step = (exit - bird.position).clamp_length_max(LEAVE_SPEED * dt);
                bird.position += step;
                if bird.position.distance(exit) < 4.0 {
                    for (image, owner) in &images {
                        if owner.0 == entity {
                            commands.entity(image).despawn();
                        }
                    }
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

/// Wer nah am Ziel ist, sieht vielleicht den Schatten herabstoßen und rennt los.
fn warn_neighbors(
    grid: &Grid,
    reserved: &mut Reserved,
    rng: &mut Rng,
    rabbits: &mut Query<(Entity, &Rabbit, &Transform, &OnLayer, &mut Activity, &mut Path, &mut Fear, &Fur), Without<Buzzard>>,
    target: Entity,
) {
    let Ok((_, _, aim, ..)) = rabbits.get(target) else {
        return;
    };
    let aim = aim.translation.truncate();
    for (_, _, transform, layer, mut activity, mut path, mut fear, _) in rabbits.iter_mut() {
        if transform.translation.truncate().distance(aim) < NOTICE_RADIUS && rng.chance(NOTICE_CHANCE) {
            start_flight(grid, reserved, transform, layer, &mut activity, &mut path, &mut fear);
        }
    }
}

/// Nichts erwischt: noch eine Runde woanders, oder nach einigen Versuchen weiterziehen.
fn give_up_or_circle(
    bird: &mut Buzzard,
    rng: &mut Rng,
    rabbits: &Query<(Entity, &Rabbit, &Transform, &OnLayer, &mut Activity, &mut Path, &mut Fear, &Fur), Without<Buzzard>>,
) {
    bird.tries += 1;
    if bird.tries >= MAX_TRIES {
        bird.flight = Flight::Leaving;
        return;
    }
    let outside: Vec<Vec2> = rabbits
        .iter()
        .filter(|(_, _, _, layer, ..)| layer.0 == Layer::Surface)
        .map(|(_, _, t, ..)| t.translation.truncate())
        .collect();
    if !outside.is_empty() {
        bird.center = outside[rng.below(outside.len())];
    }
    bird.flight = Flight::Circling { left: rng.range(CIRCLING_SECONDS.0, CIRCLING_SECONDS.1) * 0.6 };
}

/// Wachen sehen den kreisenden Bussard und stampfen; alle in Hörweite rennen in Deckung.
fn guards_watch_sky(
    mut commands: Commands,
    mut alarms: ResMut<Alarms>,
    mut messages: ResMut<Messages>,
    mut buzzards: Query<&mut Buzzard>,
    guards: Query<(Entity, &Rabbit, &Transform, &Activity)>,
) {
    for mut bird in &mut buzzards {
        if bird.spotted || bird.flight == Flight::Leaving {
            continue;
        }
        let watcher = guards.iter().find(|(_, _, t, activity)| {
            matches!(activity, Activity::Guarding { .. }) && t.translation.truncate().distance(bird.position) < GUARD_SIGHT
        });
        if let Some((guard, rabbit, transform, _)) = watcher {
            bird.spotted = true;
            raise_alarm(&mut alarms, transform.translation.truncate(), STOMP_RADIUS);
            commands.entity(guard).insert(Stomping(0.6));
            messages.push(format!("{} stampft: Bussard am Himmel!", rabbit.name));
        }
    }
}

/// Der Vogel schaut immer zur Kamera; seinen Schatten wirft die Sonne von selbst.
fn show_buzzards(
    time: Res<Time>,
    orbit: Res<Orbit>,
    view: Res<ViewLayer>,
    mut buzzards: Query<(&Buzzard, &mut CarryPoint)>,
    mut images: Query<(&BuzzardImage, &mut Transform, &mut Visibility)>,
) {
    let above = view.0 == Layer::Surface;
    let visibility = if above { Visibility::Inherited } else { Visibility::Hidden };
    for (image, mut transform, mut shown) in &mut images {
        let Ok((bird, mut point)) = buzzards.get_mut(image.0) else {
            continue;
        };
        // Die Beute hängt in den Fängen unter ihm.
        point.at = to_world3(bird.position, bird.height + 4.0);
        point.facing = 1.0;
        point.visible = above;
        // Im Sturzflug sind die Flügel angelegt, sonst schlagen sie gemächlich.
        let diving = matches!(bird.flight, Flight::Diving { .. });
        let wings = if diving { 0.55 } else { 1.0 + 0.06 * (time.elapsed_secs() * 3.0).sin() };
        transform.translation = to_world3(bird.position, bird.height + SIZE.y * 0.5);
        transform.rotation = orbit.billboard();
        transform.scale = Vec3::new(SIZE.x * wings, SIZE.y, 1.0);
        *shown = visibility;
    }
}

