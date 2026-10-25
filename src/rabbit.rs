//! Kaninchen: Daten, Aussehen, Animation, Aufwachsen und Vermehrung.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::behavior::{Activity, Carrying, Path};
use crate::colony::{Colony, Messages, Rng};
use crate::map::{Grid, Layer, TilePos, ViewLayer};
use crate::rig::{Part, RigDefinition, RigRoot, spawn_rig};

/// Spielsekunden, bis ein Junges erwachsen ist.
pub const ADULT_AGE: f32 = 150.0;
const PREGNANCY_SECONDS: f32 = 40.0;
const BIRTH_COOLDOWN_SECONDS: f32 = 90.0;
const HOPS_PER_SECOND: f32 = 2.6;
/// Wie hoch ein Hopser geht, in Spieleinheiten.
const HOP_HEIGHT: f32 = 8.0;

const RIG_PATH: &str = "figuren/kaninchen/kaninchen.rig.ron";
const RIG_FOLDER: &str = "figuren/kaninchen";

const FUR_COLORS: [Color; 5] = [
    Color::srgb(1.0, 1.0, 1.0),
    Color::srgb(0.80, 0.62, 0.44),
    Color::srgb(0.68, 0.66, 0.64),
    Color::srgb(0.88, 0.76, 0.58),
    Color::srgb(0.52, 0.40, 0.32),
];

const BABY_NAMES: [&str; 30] = [
    "Kleeblatt", "Möhrchen", "Flocke", "Hoppel", "Wuschel", "Pünktchen", "Löwenzahn", "Krümel", "Mümmel",
    "Stups", "Butterblume", "Haselnuss", "Distel", "Moos", "Sternchen", "Pfötchen", "Gänseblümchen",
    "Kiesel", "Wolke", "Brombeere", "Rübchen", "Federchen", "Tupfen", "Schnuffel", "Kamille", "Eichel",
    "Fussel", "Heidi", "Zimt", "Bommel",
];

pub struct RabbitPlugin;

impl Plugin for RabbitPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (load_rig, spawn_founders))
            .add_systems(
                Update,
                (
                    reload_rigs,
                    attach_rigs,
                    grow_up,
                    start_pregnancies,
                    give_birth,
                    tick_birth_cooldown,
                    animate,
                ),
            );
    }
}

#[derive(Component)]
pub struct Rabbit {
    pub name: String,
    pub female: bool,
    /// Alter in Spielsekunden.
    pub age: f32,
}

impl Rabbit {
    pub fn is_adult(&self) -> bool {
        self.age >= ADULT_AGE
    }
}

#[derive(Component)]
pub struct Fur(pub Color);

/// 0 = satt, 1 = sehr hungrig.
#[derive(Component)]
pub struct Hunger(pub f32);

#[derive(Component)]
pub struct OnLayer(pub Layer);

/// Kleiner Versatz zur Feldmitte, damit nicht alle Kaninchen exakt aufeinander sitzen.
#[derive(Component)]
pub struct Offset(pub Vec2);

#[derive(Component)]
struct Pregnant(f32);

#[derive(Component)]
struct BirthCooldown(f32);

#[derive(Component)]
struct HasRig;

#[derive(Component)]
struct Anim {
    hop_phase: f32,
    blink_timer: f32,
    /// Pro Kaninchen verschieden, damit sich nicht alle im Gleichschritt bewegen.
    seed: f32,
    facing_left: bool,
    last_x: f32,
}

#[derive(Resource)]
struct RabbitRig(Handle<RigDefinition>);

pub fn rabbit_bundle(
    name: String,
    female: bool,
    age: f32,
    fur: Color,
    layer: Layer,
    position: Vec2,
    rng: &mut Rng,
) -> impl Bundle {
    (
        Rabbit { name, female, age },
        Fur(fur),
        Hunger(rng.range(0.1, 0.3)),
        OnLayer(layer),
        Offset(Vec2::new(rng.range(-14.0, 14.0), rng.range(-14.0, 14.0))),
        Activity::Idle(rng.range(0.5, 2.0)),
        Path::default(),
        Carrying::default(),
        Anim {
            hop_phase: 0.0,
            blink_timer: rng.range(1.0, 4.0),
            seed: rng.f32(),
            facing_left: false,
            last_x: position.x,
        },
        Transform::from_translation(position.extend(z_for(position))),
        Visibility::default(),
    )
}

/// Weiter unten stehende Kaninchen werden vor weiter oben stehenden gezeichnet.
pub fn z_for(position: Vec2) -> f32 {
    10.0 - position.y * 0.001
}

fn load_rig(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(RabbitRig(asset_server.load(RIG_PATH)));
}

fn spawn_founders(mut commands: Commands, mut rng: ResMut<Rng>, mut messages: ResMut<Messages>) {
    let near_entrance = TilePos::new(19, 15).center();
    for (name, female, fur, dx) in [("Schneeflocke", true, FUR_COLORS[0], -20.0), ("Lukas", false, FUR_COLORS[1], 20.0)] {
        let position = near_entrance + Vec2::new(dx, 0.0);
        commands.spawn(rabbit_bundle(name.into(), female, ADULT_AGE, fur, Layer::Surface, position, &mut rng));
    }
    messages.push("Schneeflocke und Lukas gründen die Familie Buddelberg.");
}

fn attach_rigs(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    rig: Res<RabbitRig>,
    rigs: Res<Assets<RigDefinition>>,
    rabbits: Query<Entity, (With<Rabbit>, Without<HasRig>)>,
) {
    let Some(definition) = rigs.get(&rig.0) else {
        return;
    };
    for rabbit in &rabbits {
        spawn_rig(&mut commands, &asset_server, definition, RIG_FOLDER, rabbit);
        commands.entity(rabbit).insert(HasRig);
    }
}

/// Wird die Rig-Datei gespeichert, bauen alle Kaninchen ihre Teile neu auf.
fn reload_rigs(
    mut commands: Commands,
    mut events: MessageReader<AssetEvent<RigDefinition>>,
    rig: Res<RabbitRig>,
    roots: Query<Entity, With<RigRoot>>,
    rabbits: Query<Entity, With<HasRig>>,
) {
    let modified = events
        .read()
        .any(|event| matches!(event, AssetEvent::Modified { id } if *id == rig.0.id()));
    if !modified {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    for rabbit in &rabbits {
        commands.entity(rabbit).remove::<HasRig>();
    }
}

fn grow_up(time: Res<Time>, mut rabbits: Query<&mut Rabbit>, mut messages: ResMut<Messages>) {
    for mut rabbit in &mut rabbits {
        let was_adult = rabbit.is_adult();
        rabbit.age += time.delta_secs();
        if !was_adult && rabbit.is_adult() {
            messages.push(format!("{} ist erwachsen geworden.", rabbit.name));
        }
    }
}

fn start_pregnancies(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    all: Query<&Rabbit>,
    pregnant: Query<(), With<Pregnant>>,
    candidates: Query<(Entity, &Rabbit, &Hunger), (Without<Pregnant>, Without<BirthCooldown>)>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer < 2.0 {
        return;
    }
    *timer = 0.0;

    if !all.iter().any(|r| r.is_adult() && !r.female) {
        return;
    }
    // Erwarteten Nachwuchs mitzählen, damit nicht mehr Junge kommen, als Platz ist.
    let expected = all.iter().count() + pregnant.iter().count() * 3;
    let mut free = Colony::sleeping_places(&grid).saturating_sub(expected);

    for (entity, rabbit, hunger) in &candidates {
        if free < 3 {
            break;
        }
        if rabbit.female && rabbit.is_adult() && hunger.0 < 0.6 && rng.chance(0.15) {
            commands.entity(entity).insert(Pregnant(PREGNANCY_SECONDS));
            messages.push(format!("{} erwartet Nachwuchs.", rabbit.name));
            free -= 3;
        }
    }
}

fn give_birth(
    mut commands: Commands,
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    mut mothers: Query<(Entity, &Rabbit, &Fur, &OnLayer, &Transform, &mut Pregnant)>,
) {
    for (entity, mother, fur, layer, transform, mut pregnant) in &mut mothers {
        pregnant.0 -= time.delta_secs();
        if pregnant.0 > 0.0 {
            continue;
        }
        commands
            .entity(entity)
            .remove::<Pregnant>()
            .insert(BirthCooldown(BIRTH_COOLDOWN_SECONDS));

        let count = 2 + rng.below(3);
        let mut names = Vec::new();
        for _ in 0..count {
            let name = BABY_NAMES[rng.below(BABY_NAMES.len())].to_string();
            let fur = if rng.chance(0.4) {
                fur.0
            } else {
                FUR_COLORS[rng.below(FUR_COLORS.len())]
            };
            let female = rng.chance(0.5);
            let position = transform.translation.truncate();
            commands.spawn(rabbit_bundle(name.clone(), female, 0.0, fur, layer.0, position, &mut rng));
            names.push(name);
        }
        messages.push(format!("{} hat {count} Junge bekommen: {}.", mother.name, names.join(", ")));
    }
}

fn tick_birth_cooldown(mut commands: Commands, time: Res<Time>, mut rabbits: Query<(Entity, &mut BirthCooldown)>) {
    for (entity, mut cooldown) in &mut rabbits {
        cooldown.0 -= time.delta_secs();
        if cooldown.0 <= 0.0 {
            commands.entity(entity).remove::<BirthCooldown>();
        }
    }
}

fn animate(
    time: Res<Time>,
    view: Res<ViewLayer>,
    rig: Res<RabbitRig>,
    rigs: Res<Assets<RigDefinition>>,
    mut rabbits: Query<(&Rabbit, &Activity, &OnLayer, &Fur, &Transform, &mut Anim, &Children)>,
    mut roots: Query<(&mut Transform, &Children), (With<RigRoot>, Without<Rabbit>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Transform, &mut Sprite), (Without<RigRoot>, Without<Rabbit>)>,
) {
    let Some(definition) = rigs.get(&rig.0) else {
        return;
    };
    let dt = time.delta_secs();

    for (rabbit, activity, layer, fur, transform, mut anim, children) in &mut rabbits {
        let t = time.elapsed_secs() + anim.seed * 10.0;
        let moving = matches!(activity, Activity::Walking(_));
        let eating = matches!(activity, Activity::Eating { .. } | Activity::Gathering { .. });
        let digging = matches!(activity, Activity::Digging { .. });

        let dx = transform.translation.x - anim.last_x;
        if dx.abs() > 0.01 {
            anim.facing_left = dx < 0.0;
        }
        anim.last_x = transform.translation.x;

        if moving {
            anim.hop_phase = (anim.hop_phase + dt * HOPS_PER_SECOND).fract();
        } else {
            anim.hop_phase = 0.0;
        }
        let lift = (anim.hop_phase * PI).sin();
        // Stauchen beim Aufkommen, Strecken in der Luft, sonst leichtes Atmen.
        let (stretch_x, stretch_y) = if !moving {
            let breath = (t * 2.2).sin() * 0.015;
            (1.0, 1.0 + breath)
        } else if lift < 0.25 {
            (1.07, 0.93)
        } else {
            (0.96, 1.05)
        };

        anim.blink_timer -= dt;
        if anim.blink_timer < -0.12 {
            anim.blink_timer = 2.0 + anim.seed * 3.0;
        }
        let eyes_closed = anim.blink_timer < 0.0;

        // Junge sind kleiner und wachsen bis zum Erwachsenenalter heran.
        let growth = 0.55 + 0.45 * (rabbit.age / ADULT_AGE).min(1.0);
        let facing = if anim.facing_left { -1.0 } else { 1.0 };
        let alpha = if layer.0 == view.0 { 1.0 } else { 0.3 };

        for child in children.iter() {
            let Ok((mut root_transform, root_children)) = roots.get_mut(child) else {
                continue;
            };
            let scale = definition.pixel_scale * growth;
            root_transform.translation = Vec3::new(0.0, lift * HOP_HEIGHT * growth, 0.0);
            root_transform.scale = Vec3::new(facing * scale * stretch_x, scale * stretch_y, 1.0);
            root_transform.rotation = Quat::from_rotation_z(if digging { -0.15 } else { 0.0 });

            for part_entity in root_children.iter() {
                let Ok((part, mut part_transform, mut sprite)) = parts.get_mut(part_entity) else {
                    continue;
                };
                let mut pose = part.base;
                let mut angle = 0.0_f32;
                let head_part = matches!(part.name.as_str(), "kopf" | "auge" | "ohr_links" | "ohr_rechts");
                match part.name.as_str() {
                    "ohr_links" | "ohr_rechts" => {
                        angle = if moving {
                            // Ohren schwingen dem Hopser hinterher.
                            (anim.hop_phase * TAU - 0.8).sin() * 12.0
                        } else if digging {
                            -10.0
                        } else {
                            (t * 0.9).sin() * 3.0
                        };
                    }
                    "bein_hinten" => {
                        angle = if moving {
                            -(anim.hop_phase * TAU).sin() * 18.0
                        } else if digging {
                            (t * 18.0).sin() * 25.0
                        } else {
                            0.0
                        };
                    }
                    _ => {}
                }
                if eating && head_part {
                    pose.translation.y += -6.0 + (t * 9.0).sin() * 3.0;
                }
                pose.rotation *= Quat::from_rotation_z(angle.to_radians());
                *part_transform = pose;

                let image = if part.name == "auge" && eyes_closed {
                    part.variants.get("zu").unwrap_or(&part.image)
                } else {
                    &part.image
                };
                if sprite.image != *image {
                    sprite.image = image.clone();
                }
                let color = if part.tint { fur.0 } else { Color::WHITE }.with_alpha(alpha);
                if sprite.color != color {
                    sprite.color = color;
                }
            }
        }
    }
}
