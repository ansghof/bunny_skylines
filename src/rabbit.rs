//! Kaninchen: Daten, Aussehen, Animation, Aufwachsen und Vermehrung.

use std::f32::consts::{PI, TAU};

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::behavior::{Activity, Carrying, DigTarget, Fear, Path, Reserved, release};
use crate::effects::{DirtAssets, throw_dirt};
use crate::colony::{Colony, Messages, Rng};
use crate::map::{Grid, Layer, TilePos, ViewLayer};
use crate::camera::{Orbit, figure_layer};
use crate::daynight::{DayClock, Season, YEAR_LENGTH, figure_light};
use crate::rig::{Avatar, AvatarLink, Part, RigRoot, RigSource, pose_part};

/// Spielsekunden, bis ein Junges erwachsen ist.
pub const ADULT_AGE: f32 = 150.0;
/// Lebensdauer in Spieljahren, von bis.
const LIFESPAN_YEARS: (f32, f32) = (8.0, 9.5);
/// Ab diesem Alter in Spieljahren gilt ein Kaninchen als alt.
const OLD_AGE_YEARS: f32 = 6.0;
/// So lange hält ein völlig ausgehungertes Kaninchen durch, in Spielsekunden.
const STARVE_SECONDS: f32 = 60.0;
/// Ab diesem Hunger zeigt ein Kaninchen eine Gedankenblase.
pub const SHOW_HUNGER: f32 = 0.8;
/// Gedankenblasen sind in vierfacher Spielgröße gemalt.
const ICON_SCALE: f32 = 0.4;
/// Höhe der Blase über den Füßen eines erwachsenen Kaninchens.
const ICON_HEIGHT: f32 = 64.0;
/// So weit sitzt der Kopf vor der Körpermitte.
const ICON_AHEAD: f32 = 12.0;
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
        app.add_systems(Startup, (spawn_founders, prepare_hunger_icons))
            .add_systems(Update, show_hunger.after(animate))
            .add_systems(
            Update,
            (
                (grow_up, die).chain(),
                start_pregnancies,
                give_birth,
                tick_birth_cooldown,
                tick_stomping,
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
    /// So alt wird dieses Kaninchen, in Spielsekunden.
    lifespan: f32,
    /// Wie lange es schon völlig ausgehungert ist, in Spielsekunden.
    starving: f32,
}

impl Rabbit {
    pub fn is_adult(&self) -> bool {
        self.age >= ADULT_AGE
    }

    /// Alte Kaninchen sind langsamer.
    pub fn is_old(&self) -> bool {
        self.age >= OLD_AGE_YEARS * YEAR_LENGTH
    }

    /// Völlig ausgehungert; ohne Futter stirbt es bald.
    pub fn is_starving(&self) -> bool {
        self.starving > 0.0
    }

    /// Junge sind kleiner und wachsen bis zum Erwachsenenalter heran.
    fn growth(&self) -> f32 {
        0.55 + 0.45 * (self.age / ADULT_AGE).min(1.0)
    }

    /// Nur für Testläufe: Das Kaninchen hat noch so viele Spielsekunden zu leben.
    #[cfg(debug_assertions)]
    pub fn set_remaining_life(&mut self, seconds: f32) {
        self.age = (self.lifespan - seconds).max(0.0);
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
    last_position: Vec2,
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
            Rabbit {
                name,
                female,
                age,
                // Nicht alle Gleichaltrigen sollen am selben Tag sterben.
                lifespan: rng.range(LIFESPAN_YEARS.0, LIFESPAN_YEARS.1) * YEAR_LENGTH,
                starving: 0.0,
            },
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
            last_position: position,
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

pub fn spawn_founders(
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

#[derive(Resource)]
struct HungerIcons {
    /// Unten in der Mitte verankert, damit die Blase auf dem Kopf sitzt.
    mesh: Handle<Mesh>,
    hungry: Handle<StandardMaterial>,
    starving: Handle<StandardMaterial>,
}

/// Die Gedankenblase eines Kaninchens.
#[derive(Component)]
struct HungerIcon;

/// Verweis vom Kaninchen auf seine Gedankenblase.
#[derive(Component)]
struct HungerIconLink(Entity);

fn prepare_hunger_icons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut icon = |file: &str| {
        materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load(format!("symbole/{file}"))),
            alpha_mode: AlphaMode::AlphaToCoverage,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    };
    let hungry = icon("hunger.png");
    let starving = icon("hungersnot.png");
    commands.insert_resource(HungerIcons {
        mesh: meshes.add(Mesh::from(Rectangle::new(64.0, 72.0)).translated_by(Vec3::new(0.0, 36.0, 0.0))),
        hungry,
        starving,
    });
}

/// Hungrige Kaninchen zeigen eine Blase mit Möhre, verhungernde eine rote, pulsierende.
fn show_hunger(
    mut commands: Commands,
    time: Res<Time>,
    view: Res<ViewLayer>,
    orbit: Res<Orbit>,
    icons: Res<HungerIcons>,
    rabbits: Query<(Entity, &Rabbit, &Hunger, &OnLayer, &Anim, &AvatarLink, Option<&HungerIconLink>)>,
    mut bubbles: Query<
        (&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>),
        With<HungerIcon>,
    >,
) {
    for (entity, rabbit, hunger, layer, anim, avatar, link) in &rabbits {
        let Some(link) = link else {
            let bubble = commands
                .spawn((
                    HungerIcon,
                    Mesh3d(icons.mesh.clone()),
                    MeshMaterial3d(icons.hungry.clone()),
                    NotShadowCaster,
                    figure_layer(),
                    Transform::default(),
                    Visibility::Hidden,
                    ChildOf(avatar.0),
                ))
                .id();
            commands.entity(entity).insert(HungerIconLink(bubble));
            continue;
        };
        // Ist der Aufsteller neu gebaut worden, ist die alte Blase mit ihm verschwunden.
        let Ok((mut transform, mut visibility, mut material)) = bubbles.get_mut(link.0) else {
            commands.entity(entity).remove::<HungerIconLink>();
            continue;
        };

        let show = hunger.0 >= SHOW_HUNGER && layer.0 == view.0;
        let wanted = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if !show {
            continue;
        }

        let starving = rabbit.is_starving();
        let image = if starving { &icons.starving } else { &icons.hungry };
        if material.0 != *image {
            material.0 = image.clone();
        }
        let t = time.elapsed_secs();
        let growth = rabbit.growth();
        let pulse = if starving { 1.0 + (t * 6.0).sin() * 0.12 } else { 1.0 };
        let bob = (t * 2.0).sin() * 1.5;
        // Über den Kopf, der in Blickrichtung vor der Körpermitte sitzt.
        let right = orbit.screen_right();
        let facing = if anim.facing_left { -1.0 } else { 1.0 };
        let ahead = Vec3::new(right.x, 0.0, -right.y) * facing * ICON_AHEAD * growth;
        *transform = Transform {
            translation: ahead + Vec3::new(0.0, ICON_HEIGHT * growth + bob, 0.0),
            rotation: orbit.billboard(),
            scale: Vec3::splat(ICON_SCALE * pulse),
        };
    }
}

/// Kaninchen sterben im hohen Alter oder wenn sie zu lange nichts zu fressen finden.
fn die(
    mut commands: Commands,
    time: Res<Time>,
    mut messages: ResMut<Messages>,
    mut reserved: ResMut<Reserved>,
    mut rabbits: Query<(Entity, &mut Rabbit, &Hunger, &Activity)>,
) {
    let dt = time.delta_secs();
    for (entity, mut rabbit, hunger, activity) in &mut rabbits {
        let years = rabbit.age / YEAR_LENGTH;
        let message = if rabbit.age >= rabbit.lifespan {
            format!("{} ist mit {years:.0} Jahren friedlich eingeschlafen.", rabbit.name)
        } else if hunger.0 >= 1.0 {
            // Warnen, sobald der Hunger zu zählen beginnt; im ersten Bild ist dt oft 0.
            let was_starving = rabbit.starving > 0.0;
            rabbit.starving += dt;
            if !was_starving && rabbit.starving > 0.0 {
                messages.push(format!("{} hat großen Hunger und wird schwach!", rabbit.name));
            }
            if rabbit.starving < STARVE_SECONDS {
                continue;
            }
            format!("{} hat den Hunger nicht überstanden.", rabbit.name)
        } else {
            rabbit.starving = 0.0;
            continue;
        };
        release(activity, &mut reserved);
        commands.entity(entity).despawn();
        messages.push(message);
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
    clock: Res<DayClock>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer < 2.0 {
        return;
    }
    *timer = 0.0;
    // Nachwuchs gibt es nur im Frühling und Sommer, damit die Jungen vor dem Winter groß sind.
    if !matches!(clock.season(), Season::Spring | Season::Summer) {
        return;
    }

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
    mut commands: Commands,
    time: Res<Time>,
    view: Res<ViewLayer>,
    orbit: Res<Orbit>,
    clock: Res<DayClock>,
    dirt: Res<DirtAssets>,
    mut rng: ResMut<Rng>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rabbits: Query<(&Rabbit, &Activity, &OnLayer, &Fur, &Transform, &mut Anim, Has<Stomping>, &AvatarLink)>,
    avatars: Query<(&Children, &Transform), (With<Avatar>, Without<Rabbit>)>,
    mut roots: Query<(&RigRoot, &mut Transform, &Children), (Without<Rabbit>, Without<Part>, Without<Avatar>)>,
    mut parts: Query<(&Part, &mut Transform), (Without<RigRoot>, Without<Rabbit>, Without<Avatar>)>,
) {
    let dt = time.delta_secs();
    let screen_right = orbit.screen_right();
    let billboard = orbit.billboard();

    for (rabbit, activity, layer, fur, transform, mut anim, stomping, link) in &mut rabbits {
        let Ok((children, avatar_transform)) = avatars.get(link.0) else {
            continue;
        };
        let light = figure_light(&clock, layer.0);
        let t = time.elapsed_secs() + anim.seed * 10.0;
        let fleeing = matches!(activity, Activity::Fleeing);
        let moving = fleeing || matches!(activity, Activity::Walking(_));
        let eating = matches!(activity, Activity::Eating { .. } | Activity::Gathering { .. });
        let digging = matches!(activity, Activity::Digging { .. });
        let guarding = matches!(activity, Activity::Guarding { .. });
        let sleeping = matches!(activity, Activity::Sleeping { .. });
        let starving = rabbit.is_starving();

        // Links oder rechts auf dem Bildschirm, je nachdem wie die Kamera gedreht ist.
        // Beim Graben schaut das Kaninchen zum Feld, das es gräbt.
        let position = transform.translation.truncate();
        let heading = match activity {
            Activity::Digging { target: DigTarget::Tunnel(p), .. } => p.center() - position,
            _ => position - anim.last_position,
        };
        let sideways = heading.dot(screen_right);
        if sideways.abs() > 0.01 {
            anim.facing_left = sideways < 0.0;
        }
        anim.last_position = position;

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

        let growth = rabbit.growth();
        let facing = if anim.facing_left { -1.0 } else { 1.0 };
        let alpha = if layer.0 == view.0 { 1.0 } else { 0.3 };
        // Positiv = nach hinten. Die Figur ist schon gespiegelt, wenn sie gedreht wird,
        // deshalb kippt sie nur mit der Blickrichtung multipliziert nach vorn.
        let lean = facing
            * if digging {
                -0.22 + (t * 14.0).sin() * 0.03
            } else if guarding {
                0.12
            } else {
                0.0
            };

        // Die Hinterbeine schleudern die gelockerte Erde hinter dem Kaninchen in die Luft.
        if digging && layer.0 == view.0 && rng.chance(dt * 12.0) {
            let right3 = Vec3::new(screen_right.x, 0.0, -screen_right.y);
            let behind = avatar_transform.translation - right3 * facing * 18.0 * growth + Vec3::Y * 4.0;
            throw_dirt(&mut commands, &dirt, &mut rng, behind, -right3 * facing);
        }

        for child in children.iter() {
            let Ok((root, mut root_transform, root_children)) = roots.get_mut(child) else {
                continue;
            };
            let scale = root.pixel_scale * growth;
            root_transform.translation = Vec3::new(0.0, lift * HOP_HEIGHT * growth, 0.0);
            root_transform.scale = Vec3::new(facing * scale * stretch_x, scale * stretch_y, scale);
            root_transform.rotation = billboard * Quat::from_rotation_z(lean);

            for part_entity in root_children.iter() {
                let Ok((part, mut part_transform)) = parts.get_mut(part_entity) else {
                    continue;
                };
                let head_part = matches!(part.name.as_str(), "kopf" | "auge" | "ohr_links" | "ohr_rechts");
                let angle = match part.name.as_str() {
                    "ohr_links" | "ohr_rechts" if sleeping => -35.0,
                    // Wer verhungert, lässt die Ohren hängen.
                    "ohr_links" | "ohr_rechts" if starving && !moving => -28.0,
                    // Ohren schwingen dem Hopser hinterher.
                    "ohr_links" | "ohr_rechts" if moving => (anim.hop_phase * TAU - 0.8).sin() * 12.0,
                    "ohr_links" | "ohr_rechts" if digging => -10.0,
                    // Wachen drehen die Ohren lauschend hin und her.
                    "ohr_links" | "ohr_rechts" if guarding => (t * 2.5).sin() * 8.0,
                    "ohr_links" | "ohr_rechts" => (t * 0.9).sin() * 3.0,
                    "bein_hinten" if stomping => (t * 30.0).sin().abs() * -28.0,
                    "bein_hinten" if moving => -(anim.hop_phase * TAU).sin() * 18.0,
                    // Ab und zu ein Tritt nach hinten, um die Erde wegzuschieben.
                    "bein_hinten" if digging => -(t * 7.0).sin().max(0.0).powi(3) * 22.0,
                    // Die Vorderpfoten scharren schnell vor und zurück.
                    "pfote_vorne" if digging => (t * 16.0).sin() * 30.0,
                    _ => 0.0,
                };
                let offset = if digging && head_part {
                    Vec2::new(4.0, -10.0)
                } else if digging && part.name == "pfote_vorne" {
                    let scrape = t * 16.0;
                    Vec2::new(scrape.sin() * 8.0, scrape.cos().max(0.0) * 5.0)
                } else if eating && head_part {
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
                    &mut materials,
                    offset,
                    angle,
                    variant,
                    fur.0.with_alpha(alpha),
                    light,
                );
            }
        }
    }
}
