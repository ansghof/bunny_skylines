//! Der Fuchs: lauert am Kartenrand, schleicht sich an, jagt Kaninchen an der Oberfläche und zieht wieder ab.
//! In den Bau kommt er nicht. Wer erwischt wird, verschwindet mit ihm im Gebüsch.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::behavior::{Activity, BehaviorSet, Reserved, current_node, is_hidden, release};
use crate::audio::{Sound, Sounds};
use crate::colony::{Messages, Rng};
use crate::daynight::{DayClock, figure_light};
use crate::map::{Feature, Grid, Layer, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, TilePos, ViewLayer};
use crate::rabbit::{Fur, OnLayer, Rabbit, z_for};
use crate::scenario::Scenario;
use crate::progress::{Progress, Tech};
use crate::camera::Orbit;
use crate::carry::{CarryPoint, carry_off};
use crate::ground::to_world3;
use crate::rig::{Avatar, AvatarLink, Part, RigRoot, RigSource, pose_part};

const RIG_PATH: &str = "figuren/fuchs/fuchs.rig.ron";
const RIG_FOLDER: &str = "figuren/fuchs";

const LURK_SECONDS: f32 = 5.0;
const SNEAK_SPEED: f32 = 55.0;
const CHASE_SPEED: f32 = 175.0;
const LEAVE_SPEED: f32 = 120.0;
/// Ab dieser Entfernung zu einem Kaninchen beginnt die Jagd.
const CHASE_DISTANCE: f32 = 200.0;
const CATCH_DISTANCE: f32 = 20.0;
/// Bei so vielen Kaninchen kommen Füchse doppelt so oft.
const FAMILY_PER_EXTRA_PACE: f32 = 25.0;
/// Je so viele Kaninchen darf ein Fuchs mehr gleichzeitig unterwegs sein …
const FAMILY_PER_EXTRA_FOX: usize = 25;
/// … aber nie mehr als so viele.
const MAX_FOXES: usize = 3;
/// So schnell ist der Fuchs noch in einer Dornenhecke.
const HEDGE_SLOWDOWN: f32 = 0.35;
/// Ein Fuchs bleibt höchstens so lange, dann gibt er auf.
const MAX_STAY_SECONDS: f32 = 70.0;
/// So lange wartet er, wenn kein Kaninchen draußen ist.
const PATIENCE_SECONDS: f32 = 10.0;

pub struct PredatorPlugin;

impl Plugin for PredatorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FoxSpawner>()
            .add_systems(Update, (spawn_foxes, hunt).chain().after(BehaviorSet))
            .add_systems(Update, (animate_foxes, fox_carry_point).after(hunt));
        #[cfg(debug_assertions)]
        app.add_systems(Update, debug_summon_fox.before(spawn_foxes));
    }
}

/// Nur zum Testen: F8 schickt sofort einen Fuchs (F9 ist Laden).
#[cfg(debug_assertions)]
fn debug_summon_fox(keys: Res<ButtonInput<KeyCode>>, mut spawner: ResMut<FoxSpawner>) {
    if keys.just_pressed(KeyCode::F8) {
        spawner.next = 0.0;
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FoxState {
    /// Noch im Gebüsch am Rand, kaum zu sehen.
    Lurking(f32),
    Prowling,
    Chasing(Entity),
    Leaving,
}

#[derive(Component)]
pub struct Fox {
    pub state: FoxState,
    /// Eine Wache oder ein Kaninchen hat ihn gesehen: Solange er bleibt, bleiben alle im Bau.
    pub known: bool,
    stay: f32,
    no_prey: f32,
    velocity: Vec2,
    anim_phase: f32,
}

impl Fox {
    /// Ob der Fuchs schon aus dem Gebüsch heraus ist.
    pub fn is_out(&self) -> bool {
        !matches!(self.state, FoxState::Lurking(_))
    }

    /// Gesehen und noch nicht auf dem Rückzug: Draußen ist es nicht sicher.
    pub fn is_known_threat(&self) -> bool {
        self.known && self.state != FoxState::Leaving
    }
}

#[derive(Resource)]
pub struct FoxSpawner {
    next: f32,
}

impl FoxSpawner {
    /// Der erste Fuchs kommt nach so vielen Spielsekunden.
    pub fn starting(first: f32) -> Self {
        Self { next: first }
    }

    /// Gleich einen Fuchs schicken.
    pub fn summon(&mut self) {
        self.next = 0.0;
    }
}

impl Default for FoxSpawner {
    fn default() -> Self {
        // Der erste Fuchs kommt frühestens am zweiten Tag, damit die Familie Zeit zum Graben hat.
        Self { next: 300.0 }
    }
}

fn half_extent() -> Vec2 {
    Vec2::new(MAP_WIDTH as f32, MAP_HEIGHT as f32) * TILE_SIZE / 2.0
}

fn spawn_foxes(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<DayClock>,
    asset_server: Res<AssetServer>,
    mut spawner: ResMut<FoxSpawner>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    mut sounds: ResMut<Sounds>,
    scenario: Res<Scenario>,
    foxes: Query<(), With<Fox>>,
    rabbits: Query<(), With<Rabbit>>,
) {
    // In der Dämmerung kommen Füchse am häufigsten, nachts seltener.
    let pace = if clock.is_twilight() {
        2.0
    } else if clock.is_night() {
        0.5
    } else {
        1.0
    };
    // Eine große Familie lockt mehr Füchse an, und manche Gegenden sind fuchsreicher.
    let family = rabbits.iter().count();
    let pace = pace * scenario.chapter.dangers.fox_pace * (1.0 + family as f32 / FAMILY_PER_EXTRA_PACE);
    let most = (1 + family / FAMILY_PER_EXTRA_FOX).min(MAX_FOXES);
    spawner.next -= time.delta_secs() * pace;
    if spawner.next > 0.0 || foxes.iter().count() >= most || family == 0 {
        return;
    }
    spawner.next = rng.range(70.0, 130.0);

    // Ein zufälliger Punkt knapp innerhalb des Kartenrands; am Waldrand aus dem Wald.
    let half = half_extent() - TILE_SIZE * 0.5;
    let along = rng.range(-1.0, 1.0);
    let forest = scenario.chapter.terrain.has_forest();
    let position = match rng.below(4) {
        _ if forest => Vec2::new(along * half.x, half.y),
        0 => Vec2::new(-half.x, along * half.y),
        1 => Vec2::new(half.x, along * half.y),
        2 => Vec2::new(along * half.x, -half.y),
        _ => Vec2::new(along * half.x, half.y),
    };
    commands.spawn((
        Fox {
            state: FoxState::Lurking(LURK_SECONDS),
            known: false,
            stay: 0.0,
            no_prey: 0.0,
            velocity: Vec2::ZERO,
            anim_phase: 0.0,
        },
        RigSource {
            rig: asset_server.load(RIG_PATH),
            folder: RIG_FOLDER,
        },
        CarryPoint::default(),
        Transform::from_translation(position.extend(z_for(position))),
        Visibility::default(),
    ));
    messages.push(if forest { "Im Wald raschelt es …" } else { "Am Kartenrand raschelt es im Gebüsch …" });
    sounds.play(Sound::Fox);
}

fn hunt(
    mut commands: Commands,
    time: Res<Time>,
    mut messages: ResMut<Messages>,
    mut reserved: ResMut<Reserved>,
    mut sounds: ResMut<Sounds>,
    grid: Res<Grid>,
    progress: Res<Progress>,
    mut foxes: Query<(Entity, &mut Fox, &mut Transform)>,
    rabbits: Query<(Entity, &Rabbit, &Transform, &OnLayer, &Activity, &Fur), Without<Fox>>,
) {
    let dt = time.delta_secs();
    for (fox_entity, mut fox, mut transform) in &mut foxes {
        let position = transform.translation.truncate();
        fox.stay += dt;
        if fox.stay > MAX_STAY_SECONDS && fox.state != FoxState::Leaving {
            fox.state = FoxState::Leaving;
        }

        let nearest_outside = rabbits
            .iter()
            .filter(|(_, _, t, layer, _, _)| catchable(&grid, t, layer))
            .map(|(entity, _, t, _, _, _)| (entity, t.translation.truncate()))
            .min_by(|a, b| a.1.distance(position).total_cmp(&b.1.distance(position)));

        let (target, speed) = match fox.state {
            FoxState::Lurking(left) => {
                fox.state = if left - dt <= 0.0 {
                    FoxState::Prowling
                } else {
                    FoxState::Lurking(left - dt)
                };
                (None, 0.0)
            }
            FoxState::Prowling => match nearest_outside {
                Some((rabbit, at)) => {
                    fox.no_prey = 0.0;
                    if at.distance(position) < CHASE_DISTANCE {
                        fox.state = FoxState::Chasing(rabbit);
                    }
                    (Some(at), SNEAK_SPEED)
                }
                None => {
                    fox.no_prey += dt;
                    if fox.no_prey > PATIENCE_SECONDS {
                        fox.state = FoxState::Leaving;
                    }
                    (None, 0.0)
                }
            },
            FoxState::Chasing(rabbit) => match rabbits.get(rabbit) {
                Ok((_, prey, at, layer, activity, fur)) if catchable(&grid, at, layer) => {
                    let at = at.translation.truncate();
                    if at.distance(position) < CATCH_DISTANCE {
                        release(activity, &mut reserved);
                        commands.entity(rabbit).despawn();
                        carry_off(&mut commands, fox_entity, fur.0);
                        messages.push(format!("{} wurde vom Fuchs erwischt.", prey.name));
                        sounds.play(Sound::Caught);
                        fox.state = FoxState::Leaving;
                        (None, 0.0)
                    } else {
                        (Some(at), CHASE_SPEED)
                    }
                }
                // Entwischt oder schon im Bau: weiter umherschleichen.
                _ => {
                    fox.state = FoxState::Prowling;
                    (None, 0.0)
                }
            },
            FoxState::Leaving => {
                let exit = nearest_exit(position);
                if position.distance(exit) < 4.0 {
                    commands.entity(fox_entity).despawn();
                    // Wer ihn gesehen hat, bekam schon beim Abzug Entwarnung.
                    if !fox.known {
                        messages.push("Der Fuchs ist wieder fort.");
                    }
                    continue;
                }
                (Some(exit), LEAVE_SPEED)
            }
        };

        // Durch Dornenhecken kommt der Fuchs nur mühsam; mit Dornenwall gibt er dort ganz auf.
        let in_hedge = TilePos::from_world(position)
            .is_some_and(|p| grid.surface(p).feature == Some(Feature::Hedge));
        if in_hedge && progress.has(Tech::ThornWall) && fox.state != FoxState::Leaving {
            fox.state = FoxState::Leaving;
            messages.push("Der Fuchs hat sich im Dornenwall verfangen und gibt auf.");
        }
        let speed = if in_hedge { speed * HEDGE_SLOWDOWN } else { speed };
        fox.velocity = match target {
            Some(target) if target.distance(position) > 1.0 => (target - position).normalize() * speed,
            _ => Vec2::ZERO,
        };
        let next = position + fox.velocity * dt;
        transform.translation = next.extend(z_for(next));
    }
}

/// Die Beute hängt dem Fuchs vorn aus dem Maul.
fn fox_carry_point(orbit: Res<Orbit>, view: Res<ViewLayer>, mut foxes: Query<(&Fox, &Transform, &mut CarryPoint)>) {
    let right = to_world3(orbit.screen_right(), 0.0);
    for (fox, transform, mut point) in &mut foxes {
        let sideways = fox.velocity.dot(orbit.screen_right());
        if sideways.abs() > 0.1 {
            point.facing = sideways.signum();
        } else if point.facing == 0.0 {
            point.facing = 1.0;
        }
        point.at = to_world3(transform.translation.truncate(), 38.0) + right * point.facing * 36.0;
        point.visible = view.0 == Layer::Surface;
    }
}

/// Nur Kaninchen draußen und nicht im Fluchtloch sind erreichbar.
fn catchable(grid: &Grid, transform: &Transform, layer: &OnLayer) -> bool {
    layer.0 == Layer::Surface && !is_hidden(grid, current_node(transform, layer))
}

/// Der nächste Punkt knapp außerhalb der Karte.
fn nearest_exit(position: Vec2) -> Vec2 {
    let half = half_extent() + TILE_SIZE;
    let candidates = [
        Vec2::new(-half.x, position.y),
        Vec2::new(half.x, position.y),
        Vec2::new(position.x, -half.y),
        Vec2::new(position.x, half.y),
    ];
    candidates
        .into_iter()
        .min_by(|a, b| a.distance(position).total_cmp(&b.distance(position)))
        .unwrap()
}

fn animate_foxes(
    time: Res<Time>,
    view: Res<ViewLayer>,
    orbit: Res<Orbit>,
    clock: Res<DayClock>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut foxes: Query<(&mut Fox, &AvatarLink)>,
    avatars: Query<&Children, With<Avatar>>,
    mut roots: Query<(&RigRoot, &mut Transform, &Children), (Without<Fox>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Transform), (Without<RigRoot>, Without<Fox>)>,
) {
    let dt = time.delta_secs();
    let light = figure_light(&clock, Layer::Surface);
    for (mut fox, link) in &mut foxes {
        let Ok(children) = avatars.get(link.0) else {
            continue;
        };
        let speed = fox.velocity.length();
        let sneaking = fox.state == FoxState::Prowling;
        // Schritte pro Sekunde wachsen mit dem Tempo.
        fox.anim_phase = (fox.anim_phase + dt * (speed / 55.0).max(0.0)).fract();
        let swing = (fox.anim_phase * TAU).sin();
        let t = time.elapsed_secs();
        // Links oder rechts auf dem Bildschirm, je nachdem wie die Kamera gedreht ist.
        let sideways = fox.velocity.dot(orbit.screen_right());
        let facing = if sideways < -0.1 { -1.0 } else { 1.0 };
        let alpha = match (fox.state, view.0) {
            (_, Layer::Underground) => 0.3,
            (FoxState::Lurking(_), _) => 0.45,
            _ => 1.0,
        };

        for child in children.iter() {
            let Ok((root, mut root_transform, root_children)) = roots.get_mut(child) else {
                continue;
            };
            // Beim Anschleichen duckt er sich.
            let crouch = if sneaking { 0.9 } else { 1.0 };
            let bob = if speed > 1.0 { swing.abs() * 3.0 } else { 0.0 };
            root_transform.translation = Vec3::new(0.0, bob, 0.0);
            // Bleibt er stehen, behält er die letzte Blickrichtung.
            if sideways.abs() > 0.1 {
                root_transform.scale.x = facing * root.pixel_scale;
            } else if root_transform.scale.x == 0.0 {
                root_transform.scale.x = root.pixel_scale;
            }
            root_transform.scale.y = root.pixel_scale * crouch;
            root_transform.scale.z = root.pixel_scale;
            root_transform.rotation = orbit.billboard();

            for part_entity in root_children.iter() {
                let Ok((part, mut part_transform)) = parts.get_mut(part_entity) else {
                    continue;
                };
                let stride = if speed > 1.0 { 22.0 } else { 0.0 };
                let angle = match part.name.as_str() {
                    // Diagonale Beinpaare bewegen sich gemeinsam, wie beim Traben.
                    "bein_hinten_nah" | "bein_vorne_fern" => swing * stride,
                    "bein_vorne_nah" | "bein_hinten_fern" => -swing * stride,
                    "schwanz" => (t * 3.0).sin() * 6.0,
                    "ohr_nah" | "ohr_fern" if sneaking => -12.0,
                    _ => 0.0,
                };
                pose_part(
                    part,
                    &mut part_transform,
                    &mut materials,
                    Vec2::ZERO,
                    angle,
                    None,
                    Color::WHITE.with_alpha(alpha),
                    light,
                );
            }
        }
    }
}
