//! Kaninchen: Daten, Aussehen, Animation, Aufwachsen und Vermehrung.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::behavior::{Activity, Carrying, Fear, Path};
use crate::colony::{Colony, Messages, Rng};
use crate::map::{Grid, Layer, TilePos, ViewLayer};
use crate::rig::{Part, RigRoot, RigSource, pose_part};

/// Spielsekunden, bis ein Junges erwachsen ist.
pub const ADULT_AGE: f32 = 150.0;
const PREGNANCY_SECONDS: f32 = 40.0;
const BIRTH_COOLDOWN_SECONDS: f32 = 90.0;
const HOPS_PER_SECOND: f32 = 2.6;
const FLEE_HOPS_PER_SECOND: f32 = 3.8;
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
        app.add_systems(Startup, spawn_founders).add_systems(
            Update,
            (grow_up, start_pregnancies, give_birth, tick_birth_cooldown, tick_stomping, animate),
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

/// Stampft gerade auf den Boden, noch so viele Sekunden.
#[derive(Component)]
pub struct Stomping(pub f32);

#[derive(Component)]
struct Pregnant(f32);

#[derive(Component)]
struct BirthCooldown(f32);

#[derive(Component)]
struct Anim {
    hop_phase: f32,
    blink_timer: f32,
    /// Pro Kaninchen verschieden, damit sich nicht alle im Gleichschritt bewegen.
    seed: f32,
    facing_left: bool,
    last_x: f32,
}

fn rabbit_bundle(
    asset_server: &AssetServer,
    name: String,
    female: bool,
    age: f32,
    fur: Color,
    layer: Layer,
    position: Vec2,
    rng: &mut Rng,
) -> impl Bundle {
    (
        (
            Rabbit { name, female, age },
            Fur(fur),
            Hunger(rng.range(0.1, 0.3)),
            Fear::default(),
            OnLayer(layer),
            Offset(Vec2::new(rng.range(-14.0, 14.0), rng.range(-14.0, 14.0))),
        ),
        (Activity::Idle(rng.range(0.5, 2.0)), Path::default(), Carrying::default()),
        Anim {
            hop_phase: 0.0,
            blink_timer: rng.range(1.0, 4.0),
            seed: rng.f32(),
            facing_left: false,
            last_x: position.x,
        },
        RigSource {
            rig: asset_server.load(RIG_PATH),
            folder: RIG_FOLDER,
        },
        Transform::from_translation(position.extend(z_for(position))),
        Visibility::default(),
    )
}

/// Weiter unten stehende Figuren werden vor weiter oben stehenden gezeichnet.
pub fn z_for(position: Vec2) -> f32 {
    10.0 - position.y * 0.001
}

fn spawn_founders(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
) {
    let near_entrance = TilePos::new(19, 15).center();
    for (name, female, fur, dx) in [("Schneeflocke", true, FUR_COLORS[0], -20.0), ("Lukas", false, FUR_COLORS[1], 20.0)] {
        let position = near_entrance + Vec2::new(dx, 0.0);
        commands.spawn(rabbit_bundle(
            &asset_server,
            name.into(),
            female,
            ADULT_AGE,
            fur,
            Layer::Surface,
            position,
            &mut rng,
        ));
    }
    messages.push("Schneeflocke und Lukas gründen die Familie Buddelberg.");
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
    asset_server: Res<AssetServer>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    mut mothers: Query<(Entity, &Rabbit, &Fur, &OnLayer, &Transform, &mut Pregnant)>,
    everyone: Query<&Rabbit>,
) {
    let mut taken: Vec<String> = everyone.iter().map(|r| r.name.clone()).collect();
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
            let name = unused_name(&taken, &mut rng);
            taken.push(name.clone());
            let fur = if rng.chance(0.4) {
                fur.0
            } else {
                FUR_COLORS[rng.below(FUR_COLORS.len())]
            };
            let female = rng.chance(0.5);
            let position = transform.translation.truncate();
            commands.spawn(rabbit_bundle(
                &asset_server,
                name.clone(),
                female,
                0.0,
                fur,
                layer.0,
                position,
                &mut rng,
            ));
            names.push(name);
        }
        messages.push(format!("{} hat {count} Junge bekommen: {}.", mother.name, names.join(", ")));
    }
}

/// Ein Name, den noch kein Kaninchen trägt; sind alle vergeben, mit Zahl dahinter („Moos 2“).
fn unused_name(taken: &[String], rng: &mut Rng) -> String {
    let free: Vec<&str> = BABY_NAMES
        .iter()
        .copied()
        .filter(|name| !taken.iter().any(|t| t == name))
        .collect();
    if !free.is_empty() {
        return free[rng.below(free.len())].to_string();
    }
    let base = BABY_NAMES[rng.below(BABY_NAMES.len())];
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !taken.contains(name))
        .unwrap()
}

fn tick_birth_cooldown(mut commands: Commands, time: Res<Time>, mut rabbits: Query<(Entity, &mut BirthCooldown)>) {
    for (entity, mut cooldown) in &mut rabbits {
        cooldown.0 -= time.delta_secs();
        if cooldown.0 <= 0.0 {
            commands.entity(entity).remove::<BirthCooldown>();
        }
    }
}

fn tick_stomping(mut commands: Commands, time: Res<Time>, mut rabbits: Query<(Entity, &mut Stomping)>) {
    for (entity, mut stomping) in &mut rabbits {
        stomping.0 -= time.delta_secs();
        if stomping.0 <= 0.0 {
            commands.entity(entity).remove::<Stomping>();
        }
    }
}

fn animate(
    time: Res<Time>,
    view: Res<ViewLayer>,
    mut rabbits: Query<(&Rabbit, &Activity, &OnLayer, &Fur, &Transform, &mut Anim, Has<Stomping>, &Children)>,
    mut roots: Query<(&RigRoot, &mut Transform, &Children), (Without<Rabbit>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Transform, &mut Sprite), (Without<RigRoot>, Without<Rabbit>)>,
) {
    let dt = time.delta_secs();

    for (rabbit, activity, layer, fur, transform, mut anim, stomping, children) in &mut rabbits {
        let t = time.elapsed_secs() + anim.seed * 10.0;
        let fleeing = matches!(activity, Activity::Fleeing);
        let moving = fleeing || matches!(activity, Activity::Walking(_));
        let eating = matches!(activity, Activity::Eating { .. } | Activity::Gathering { .. });
        let digging = matches!(activity, Activity::Digging { .. });
        let guarding = matches!(activity, Activity::Guarding { .. });
        let sleeping = matches!(activity, Activity::Sleeping { .. });

        let dx = transform.translation.x - anim.last_x;
        if dx.abs() > 0.01 {
            anim.facing_left = dx < 0.0;
        }
        anim.last_x = transform.translation.x;

        if moving {
            let rate = if fleeing { FLEE_HOPS_PER_SECOND } else { HOPS_PER_SECOND };
            anim.hop_phase = (anim.hop_phase + dt * rate).fract();
        } else {
            anim.hop_phase = 0.0;
        }
        let lift = (anim.hop_phase * PI).sin();
        // Stauchen beim Aufkommen, Strecken in der Luft, sonst leichtes Atmen.
        let (stretch_x, stretch_y) = if sleeping {
            (1.04, 0.9 + (t * 1.2).sin() * 0.02)
        } else if !moving {
            (1.0, 1.0 + (t * 2.2).sin() * 0.015)
        } else if lift < 0.25 {
            (1.07, 0.93)
        } else {
            (0.96, 1.05)
        };

        anim.blink_timer -= dt;
        if anim.blink_timer < -0.12 {
            anim.blink_timer = 2.0 + anim.seed * 3.0;
        }
        let eyes_closed = sleeping || anim.blink_timer < 0.0;

        // Junge sind kleiner und wachsen bis zum Erwachsenenalter heran.
        let growth = 0.55 + 0.45 * (rabbit.age / ADULT_AGE).min(1.0);
        let facing = if anim.facing_left { -1.0 } else { 1.0 };
        let alpha = if layer.0 == view.0 { 1.0 } else { 0.3 };
        let lean = if digging {
            -0.15
        } else if guarding {
            0.12
        } else {
            0.0
        };

        for child in children.iter() {
            let Ok((root, mut root_transform, root_children)) = roots.get_mut(child) else {
                continue;
            };
            let scale = root.pixel_scale * growth;
            root_transform.translation = Vec3::new(0.0, lift * HOP_HEIGHT * growth, 0.0);
            root_transform.scale = Vec3::new(facing * scale * stretch_x, scale * stretch_y, 1.0);
            root_transform.rotation = Quat::from_rotation_z(lean);

            for part_entity in root_children.iter() {
                let Ok((part, mut part_transform, mut sprite)) = parts.get_mut(part_entity) else {
                    continue;
                };
                let head_part = matches!(part.name.as_str(), "kopf" | "auge" | "ohr_links" | "ohr_rechts");
                let angle = match part.name.as_str() {
                    "ohr_links" | "ohr_rechts" if sleeping => -35.0,
                    // Ohren schwingen dem Hopser hinterher.
                    "ohr_links" | "ohr_rechts" if moving => (anim.hop_phase * TAU - 0.8).sin() * 12.0,
                    "ohr_links" | "ohr_rechts" if digging => -10.0,
                    // Wachen drehen die Ohren lauschend hin und her.
                    "ohr_links" | "ohr_rechts" if guarding => (t * 2.5).sin() * 8.0,
                    "ohr_links" | "ohr_rechts" => (t * 0.9).sin() * 3.0,
                    "bein_hinten" if stomping => (t * 30.0).sin().abs() * -28.0,
                    "bein_hinten" if moving => -(anim.hop_phase * TAU).sin() * 18.0,
                    "bein_hinten" if digging => (t * 18.0).sin() * 25.0,
                    _ => 0.0,
                };
                let offset = if eating && head_part {
                    Vec2::new(0.0, -6.0 + (t * 9.0).sin() * 3.0)
                } else if sleeping && head_part {
                    Vec2::new(0.0, -10.0)
                } else {
                    Vec2::ZERO
                };
                let variant = (part.name == "auge" && eyes_closed).then_some("zu");
                pose_part(
                    part,
                    &mut part_transform,
                    &mut sprite,
                    offset,
                    angle,
                    variant,
                    fur.0.with_alpha(alpha),
                );
            }
        }
    }
}
