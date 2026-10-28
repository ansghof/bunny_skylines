//! Kaninchen: Daten, Aussehen, Animation, Aufwachsen und Vermehrung.

use std::f32::consts::{PI, TAU};

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::behavior::{Activity, Carrying, DigTarget, Fear, Goal, Path, Reserved, current_node, is_hidden, path_to_edge, release};
use crate::effects::{DirtAssets, throw_dirt};
use crate::audio::{Sound, Sounds};
use crate::colony::{Colony, Messages, Rng};
use crate::map::{Chamber, Feature, Grid, Layer, TilePos, ViewLayer};
use crate::camera::{Orbit, figure_layer};
use crate::daynight::{DayClock, Season, YEAR_LENGTH, figure_light};
use crate::rig::{Avatar, AvatarLink, FigureShadow, Part, RigRoot, RigSource, pose_part};

/// Spielsekunden, bis ein Junges erwachsen ist.
pub const ADULT_AGE: f32 = 150.0;
/// Grundwahrscheinlichkeit pro Prüfung (alle 2 s), dass ein Weibchen trächtig wird.
const PREGNANCY_CHANCE: f32 = 0.15;
/// Jedes Kaninchen über den Schlafplätzen lässt nur so viel der Fortpflanzung übrig, bei der Chance
/// auf Trächtigkeit und bei der Pause bis zum nächsten Wurf: 5 zu viele lassen ein Viertel,
/// 10 zu viele nur noch ein Achtzehntel.
const CROWDING_FACTOR: f32 = 0.75;

/// Wie viel Fortpflanzung bei so vielen Kaninchen über den Schlafplätzen bleibt, 0 bis 1.
fn crowding(rabbits: usize, places: usize) -> f32 {
    CROWDING_FACTOR.powf(rabbits.saturating_sub(places) as f32)
}
/// Mehr Kaninchen gibt es nie, auch nicht mit noch so vielen Nestern.
pub const MAX_RABBITS: usize = 600;
/// So groß kann ein Wurf höchstens werden.
const MAX_LITTER: usize = 5;
/// Lebensdauer in Spieljahren, von bis.
const LIFESPAN_YEARS: (f32, f32) = (3.7, 4.3);
/// Ab diesem Alter in Spieljahren gilt ein Kaninchen als alt.
const OLD_AGE_YEARS: f32 = 3.0;
/// So lange hält ein völlig ausgehungertes Kaninchen durch, in Spielsekunden.
const STARVE_SECONDS: f32 = 60.0;
/// So zufrieden ist ein Kaninchen, wenn alles stimmt.
const CONTENT: f32 = 0.8;
/// Darunter zeigt es eine Regenwolke.
const UNHAPPY: f32 = 0.35;
/// Wer so unglücklich ist, denkt ans Auswandern …
const LEAVING: f32 = UNHAPPY;
/// … und tut es nach so vielen Spielsekunden am Stück, z. B. bei langem Hunger.
const LEAVE_AFTER_SECONDS: f32 = 90.0;
/// Nach so vielen Nächten hintereinander ohne Nest zieht ein Kaninchen fort …
const LEAVE_AFTER_BAD_NIGHTS: u32 = 2;
/// … mit Festplatz erst nach so vielen.
const LEAVE_AFTER_BAD_NIGHTS_FESTIVAL: u32 = 4;
/// So sehr drückt eine Nacht ohne Nest die Stimmung, ohne und mit Festplatz.
const NO_NEST_MOOD: f32 = 0.5;
const NO_NEST_MOOD_FESTIVAL: f32 = 0.3;

fn has_festival(grid: &Grid) -> bool {
    Grid::positions().any(|p| grid.surface(p).feature == Some(Feature::Festival))
}
/// Sonnenplatz in der Nähe und Spielwiesen heben die Stimmung.
const SUN_SPOT_MOOD: f32 = 0.15;
const PLAYGROUND_MOOD: f32 = 0.05;
const PLAYGROUND_MOOD_MAX: f32 = 0.15;
/// So viele Sekunden braucht die Stimmung ungefähr, um sich auf eine neue Lage einzustellen.
const MOOD_SECONDS: f32 = 30.0;
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
/// So tief (in Bildpixeln) sinkt ein Kaninchen ins Fluchtloch: Die Ohren beginnen dann im Loch.
const DUCK_HEIGHT: f32 = 126.0;
/// Ab diesem Anteil des Abtauchens ist vom Körper nichts mehr zu sehen.
const DUCK_HIDE_BODY: f32 = 0.5;
/// So schnell taucht es ab und wieder auf, in Anteilen pro Sekunde.
const DUCK_SPEED: f32 = 4.0;

const RIG_PATH: &str = "figuren/kaninchen/kaninchen.rig.ron";
const RIG_FOLDER: &str = "figuren/kaninchen";

const BROWN: Color = Color::srgb(0.80, 0.62, 0.44);
const GREY: Color = Color::srgb(0.68, 0.66, 0.64);
const FUR_COLORS: [Color; 7] = [
    Color::srgb(1.0, 1.0, 1.0),
    BROWN,
    GREY,
    Color::srgb(0.88, 0.76, 0.58),
    Color::srgb(0.52, 0.40, 0.32),
    // Dunkelgrau und Schwarz.
    Color::srgb(0.40, 0.39, 0.39),
    Color::srgb(0.22, 0.21, 0.21),
];

const BABY_NAMES: &[&str] = &[
    "Kleeblatt", "Möhrchen", "Flocke", "Hoppel", "Wuschel", "Pünktchen", "Löwenzahn", "Krümel", "Mümmel", "Stups",
    "Butterblume", "Haselnuss", "Distel", "Moos", "Sternchen", "Pfötchen", "Gänseblümchen", "Kiesel", "Wolke",
    "Brombeere", "Rübchen", "Federchen", "Tupfen", "Schnuffel", "Kamille", "Eichel", "Fussel", "Heidi", "Zimt",
    "Bommel", "Ahorn", "Anis", "Apfel", "Aster", "Baldrian", "Beere", "Birke", "Blümchen", "Bohne", "Brösel",
    "Buche", "Butterkeks", "Dachs", "Dattel", "Dinkel", "Efeu", "Erbse", "Erdbeere", "Espe", "Farn", "Feige",
    "Fenchel", "Fichte", "Fliederchen", "Flitzer", "Flöckchen", "Frieda", "Funke", "Gerste", "Ginster", "Glöckchen",
    "Grashalm", "Graupe", "Hafer", "Hagebutte", "Halm", "Heide", "Himbeere", "Holunder", "Honig", "Hopsi", "Hummel",
    "Igelchen", "Ingwer", "Johannisbeere", "Kakao", "Kapuze", "Karotte", "Kastanie", "Keks", "Kerbel", "Kirsche",
    "Klee", "Knöpfchen", "Knubbel", "Knuffel", "Kohlrabi", "Kornblume", "Kräusel", "Kringel", "Kuller", "Kümmel",
    "Lavendel", "Lerche", "Linde", "Linse", "Lupine", "Mandel", "Margerite", "Matsch", "Melisse", "Minze", "Mohn",
    "Mohnblume", "Möhre", "Muffin", "Murmel", "Muskat", "Nebel", "Nelke", "Nougat", "Nuss", "Oregano", "Pastinake",
    "Petersilie", "Pfeffer", "Pflaume", "Pilz", "Pimpernell", "Plüsch", "Pusteblume", "Quaste", "Quendel",
    "Radieschen", "Raps", "Regentropfen", "Reisig", "Rosine", "Rotkehlchen", "Salbei", "Sauerampfer", "Schlehe",
    "Schlüsselblume", "Schneebeere", "Schnupperle", "Sellerie", "Senf", "Sesam", "Sonnenschein", "Spatz",
    "Spitzwegerich", "Stachelbeere", "Staubi", "Stöpsel", "Strubbel", "Tannenzapfen", "Tau", "Thymian", "Tröpfchen",
    "Trüffel", "Tulpe", "Veilchen", "Vergissmeinnicht", "Wacholder", "Waldmeister", "Walnuss", "Weide", "Wicke",
    "Wiesel", "Wirbel", "Wurzel", "Zaunkönig", "Zwiebel", "Zwirbel",
];

pub struct RabbitPlugin;

impl Plugin for RabbitPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_founders, prepare_hunger_icons))
            .add_systems(Update, show_hunger.after(animate))
            .add_systems(
            Update,
            (
                (grow_up, die, update_mood, emigrate, depart).chain(),
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
    /// Mutter und Vater; die Gründer haben keine.
    pub parents: Option<(String, String)>,
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

/// Stimmung 0 bis 1. Hunger, Angst und kein eigenes Nest drücken sie;
/// wer lange unglücklich ist, zieht fort.
#[derive(Component)]
pub struct Mood {
    pub value: f32,
    /// Wie lange die Stimmung schon unter der Grenze zum Auswandern liegt.
    gloomy_for: f32,
    /// Hat zur letzten Ruhezeit kein Nest gefunden; bleibt bis zum nächsten Schlaf im Nest.
    pub slept_badly: bool,
    /// So viele Nächte hintereinander ohne Nest.
    pub bad_nights: u32,
    /// Der Tag, dessen Nacht zuletzt als schlecht gezählt wurde.
    bad_night_day: u32,
    /// Hat vor Kurzem Kräuter gefressen, noch so viele Sekunden.
    pub herbs: f32,
}

impl Mood {
    pub fn is_unhappy(&self) -> bool {
        self.value < UNHAPPY
    }
}

/// Kleiner Versatz zur Feldmitte, damit nicht alle Kaninchen exakt aufeinander sitzen.
#[derive(Component)]
pub struct Offset(pub Vec2);

/// Stampft gerade auf den Boden, noch so viele Sekunden.
#[derive(Component)]
pub struct Stomping(pub f32);

#[derive(Component)]
pub struct Pregnant {
    left: f32,
    father: String,
}

#[derive(Component)]
pub struct BirthCooldown(f32);

#[derive(Component)]
struct Anim {
    hop_phase: f32,
    blink_timer: f32,
    /// Pro Kaninchen verschieden, damit sich nicht alle im Gleichschritt bewegen.
    seed: f32,
    facing_left: bool,
    last_position: Vec2,
    /// 0 = steht draußen, 1 = hockt im Fluchtloch, nur die Ohren schauen heraus.
    ducked: f32,
}

fn rabbit_bundle(
    asset_server: &AssetServer,
    name: String,
    female: bool,
    age: f32,
    fur: Color,
    layer: Layer,
    position: Vec2,
    parents: Option<(String, String)>,
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
                parents,
            },
            Fur(fur),
            Hunger(rng.range(0.1, 0.3)),
            Fear::default(),
            Mood {
                value: CONTENT,
                gloomy_for: 0.0,
                slept_badly: false,
                bad_nights: 0,
                bad_night_day: 0,
                herbs: 0.0,
            },
            OnLayer(layer),
            Offset(Vec2::new(rng.range(-20.0, 20.0), rng.range(-18.0, 18.0))),
        ),
        (Activity::Idle(rng.range(0.5, 2.0)), Path::default(), Carrying::default()),
        Anim {
            hop_phase: 0.0,
            blink_timer: rng.range(1.0, 4.0),
            seed: rng.f32(),
            facing_left: false,
            last_position: position,
            ducked: 0.0,
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
    // Schneeflocke ist braun mit weißer Blume (der Name kommt vom Schwanz), Lukas grau.
    for (name, female, fur, dx) in [("Schneeflocke", true, BROWN, -20.0), ("Lukas", false, GREY, 20.0)] {
        let position = near_entrance + Vec2::new(dx, 0.0);
        commands.spawn(rabbit_bundle(
            &asset_server,
            name.into(),
            female,
            ADULT_AGE,
            fur,
            Layer::Surface,
            position,
            None,
            &mut rng,
        ));
    }
    messages.push("Schneeflocke und Lukas gründen die Familie Buddelberg.");
}

fn grow_up(time: Res<Time>, grid: Res<Grid>, mut rabbits: Query<&mut Rabbit>, mut messages: ResMut<Messages>) {
    // Mit Kinderstube wachsen Junge doppelt so schnell heran.
    let nursery = grid.chamber_tiles(Chamber::Nursery) > 0;
    for mut rabbit in &mut rabbits {
        let was_adult = rabbit.is_adult();
        let speed = if nursery && !was_adult { 2.0 } else { 1.0 };
        rabbit.age += time.delta_secs() * speed;
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
    unhappy: Handle<StandardMaterial>,
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
    let unhappy = icon("unzufrieden.png");
    commands.insert_resource(HungerIcons {
        mesh: meshes.add(Mesh::from(Rectangle::new(64.0, 72.0)).translated_by(Vec3::new(0.0, 36.0, 0.0))),
        hungry,
        starving,
        unhappy,
    });
}

/// Hungrige Kaninchen zeigen eine Blase mit Möhre, verhungernde eine rote, pulsierende.
fn show_hunger(
    mut commands: Commands,
    time: Res<Time>,
    view: Res<ViewLayer>,
    orbit: Res<Orbit>,
    icons: Res<HungerIcons>,
    rabbits: Query<(Entity, &Rabbit, &Hunger, &Mood, &OnLayer, &Anim, &AvatarLink, Option<&HungerIconLink>)>,
    mut bubbles: Query<
        (&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>),
        With<HungerIcon>,
    >,
) {
    for (entity, rabbit, hunger, mood, layer, anim, avatar, link) in &rabbits {
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

        // Hunger geht vor; sonst zeigt eine Regenwolke, wer unglücklich ist.
        let hungry = hunger.0 >= SHOW_HUNGER;
        let show = (hungry || mood.is_unhappy()) && layer.0 == view.0;
        let wanted = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if !show {
            continue;
        }

        let starving = rabbit.is_starving();
        let image = match (hungry, starving) {
            (_, true) => &icons.starving,
            (true, false) => &icons.hungry,
            (false, _) => &icons.unhappy,
        };
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

/// Die Stimmung folgt langsam dem, was ein Kaninchen gerade erlebt.
fn update_mood(
    time: Res<Time>,
    clock: Res<DayClock>,
    grid: Res<Grid>,
    mut rabbits: Query<(&Hunger, &Fear, &Activity, &Transform, &OnLayer, &mut Mood)>,
) {
    let dt = time.delta_secs();
    let resting_time = clock.is_resting_time();
    // Jede Spielwiese hebt die Grundstimmung der ganzen Familie.
    let playgrounds = Grid::positions()
        .filter(|p| grid.surface(*p).feature == Some(Feature::Playground))
        .count();
    let content = CONTENT + (playgrounds as f32 * PLAYGROUND_MOOD).min(PLAYGROUND_MOOD_MAX);
    // Gibt es mehr Kaninchen als Schlafplätze, fehlt manchen ein Nest, auch wenn sie gerade fressen.
    let crowded = rabbits.iter().count() > Colony::sleeping_places(&grid);
    // Mit Festplatz trösten die Feste über ein fehlendes Nest hinweg.
    let no_nest_penalty = if has_festival(&grid) { NO_NEST_MOOD_FESTIVAL } else { NO_NEST_MOOD };
    for (hunger, fear, activity, transform, layer, mut mood) in &mut rabbits {
        let mut target = content;
        if layer.0 == Layer::Surface && near_sun_spot(&grid, transform) {
            target += SUN_SPOT_MOOD;
        }
        if hunger.0 >= SHOW_HUNGER {
            target -= 0.5;
        }
        if fear.0 > 0.0 {
            target -= 0.2;
        }
        // Zur Ruhezeit nicht im Nest: Es war wohl keins mehr frei. Wer nur zum Fressen aufgestanden
        // ist, zählt nicht, es sei denn, die Nester reichen ohnehin nicht für alle.
        let in_nest = matches!(activity, Activity::Sleeping { .. } | Activity::Walking(Goal::Sleep(_)));
        if resting_time && !in_nest && (hunger.0 < SHOW_HUNGER || crowded) {
            target -= no_nest_penalty;
            mood.slept_badly = true;
            // Jede Nacht (im Winter: jeder Ruhetag) zählt nur einmal.
            if mood.bad_night_day != clock.day {
                mood.bad_night_day = clock.day;
                mood.bad_nights += 1;
            }
        }
        if matches!(activity, Activity::Sleeping { .. }) {
            mood.slept_badly = false;
            mood.bad_nights = 0;
        }
        mood.herbs = (mood.herbs - dt).max(0.0);
        mood.value += (target - mood.value) * (dt / MOOD_SECONDS).min(1.0);
        mood.value = mood.value.clamp(0.0, 1.0);
    }
}

/// So viele Felder weit wärmt ein Sonnenplatz die Stimmung, auch schräg.
pub const SUN_SPOT_REACH: i32 = 2;

/// Liegt ein Sonnenplatz höchstens `SUN_SPOT_REACH` Felder entfernt?
pub fn near_sun_spot(grid: &Grid, transform: &Transform) -> bool {
    let here = TilePos::from_world_clamped(transform.translation.truncate());
    (-SUN_SPOT_REACH..=SUN_SPOT_REACH).any(|dy| {
        (-SUN_SPOT_REACH..=SUN_SPOT_REACH).any(|dx| {
            let p = TilePos::new(here.x + dx, here.y + dy);
            p.in_bounds() && grid.surface(p).feature == Some(Feature::SunSpot)
        })
    })
}

/// Wer lange unglücklich ist, macht sich auf den Weg zum Kartenrand.
fn emigrate(
    time: Res<Time>,
    grid: Res<Grid>,
    mut reserved: ResMut<Reserved>,
    mut messages: ResMut<Messages>,
    mut rabbits: Query<(&Rabbit, &Transform, &OnLayer, &mut Mood, &mut Activity, &mut Path)>,
) {
    let bad_nights_limit = if has_festival(&grid) { LEAVE_AFTER_BAD_NIGHTS_FESTIVAL } else { LEAVE_AFTER_BAD_NIGHTS };
    let dt = time.delta_secs();
    for (rabbit, transform, layer, mut mood, mut activity, mut path) in &mut rabbits {
        if matches!(*activity, Activity::Walking(Goal::Leave) | Activity::Gone) {
            continue;
        }
        if !rabbit.is_adult() {
            mood.gloomy_for = 0.0;
            continue;
        }
        if mood.value >= LEAVING {
            mood.gloomy_for = 0.0;
        } else {
            mood.gloomy_for += dt;
        }
        let homeless = mood.bad_nights >= bad_nights_limit;
        if mood.gloomy_for < LEAVE_AFTER_SECONDS && !homeless {
            continue;
        }
        let Some(steps) = path_to_edge(&grid, transform, layer) else {
            continue;
        };
        release(&activity, &mut reserved);
        path.0 = steps.into();
        *activity = Activity::Walking(Goal::Leave);
        let reason = if homeless { "findet keinen Schlafplatz" } else { "ist unzufrieden" };
        messages.push(format!("{} {reason} und zieht fort.", rabbit.name));
    }
}

/// Am Kartenrand angekommen, ist das Kaninchen fort.
fn depart(
    mut commands: Commands,
    mut messages: ResMut<Messages>,
    rabbits: Query<(Entity, &Rabbit, &Activity)>,
) {
    for (entity, rabbit, activity) in &rabbits {
        if matches!(activity, Activity::Gone) {
            commands.entity(entity).despawn();
            messages.push(format!("{} hat die Wiese verlassen.", rabbit.name));
        }
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

    let fathers: Vec<&str> = all.iter().filter(|r| r.is_adult() && !r.female).map(|r| r.name.as_str()).collect();
    if fathers.is_empty() {
        return;
    }
    // Erwarteten Nachwuchs mitzählen. Fehlen Schlafplätze, geht es trotzdem weiter, nur langsamer:
    // Wer keinen Platz findet, wird unglücklich, und es bleibt immer Druck, sich zu vergrößern.
    let mut expected = all.iter().count() + pregnant.iter().count() * 3;
    let places = Colony::sleeping_places(&grid);

    for (entity, rabbit, hunger) in &candidates {
        // Auch der größte Wurf darf die Höchstzahl nicht überschreiten.
        if expected + MAX_LITTER > MAX_RABBITS {
            break;
        }
        let chance = PREGNANCY_CHANCE * crowding(expected, places);
        if rabbit.female && rabbit.is_adult() && hunger.0 < 0.6 && rng.chance(chance) {
            let father = fathers[rng.below(fathers.len())].to_string();
            commands.entity(entity).insert(Pregnant { left: PREGNANCY_SECONDS, father });
            messages.push(format!("{} erwartet Nachwuchs.", rabbit.name));
            expected += 3;
        }
    }
}

fn give_birth(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    mut sounds: ResMut<Sounds>,
    mut mothers: Query<(Entity, &Rabbit, &Fur, &OnLayer, &Transform, &mut Pregnant)>,
    everyone: Query<&Rabbit>,
    grid: Res<Grid>,
) {
    let mut taken: Vec<String> = everyone.iter().map(|r| r.name.clone()).collect();
    // Bei Enge dauert es auch länger bis zum nächsten Wurf, im selben Maß wie die Chance sinkt.
    let cooldown = BIRTH_COOLDOWN_SECONDS / crowding(taken.len(), Colony::sleeping_places(&grid));
    for (entity, mother, fur, layer, transform, mut pregnant) in &mut mothers {
        pregnant.left -= time.delta_secs();
        if pregnant.left > 0.0 {
            continue;
        }
        commands.entity(entity).remove::<Pregnant>().insert(BirthCooldown(cooldown));

        // In einer Kinderstube kommt ein Junges mehr zur Welt.
        let count = 2 + rng.below(3) + usize::from(grid.chamber_tiles(Chamber::Nursery) > 0);
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
                Some((mother.name.clone(), pregnant.father.clone())),
                &mut rng,
            ));
            names.push(name);
        }
        messages.push(format!("{} hat {count} Junge bekommen: {}.", mother.name, names.join(", ")));
        sounds.play(Sound::Birth);
    }
}

/// Ein Name, den noch kein Kaninchen trägt. Sind alle vergeben, gibt es Doppelnamen
/// („Distel-Moos“), und erst wenn auch die nicht reichen, eine Zahl dahinter („Moos 2“).
fn unused_name(taken: &[String], rng: &mut Rng) -> String {
    let free: Vec<&str> = BABY_NAMES
        .iter()
        .copied()
        .filter(|name| !taken.iter().any(|t| t == name))
        .collect();
    if !free.is_empty() {
        return free[rng.below(free.len())].to_string();
    }
    for _ in 0..50 {
        let first = BABY_NAMES[rng.below(BABY_NAMES.len())];
        let second = BABY_NAMES[rng.below(BABY_NAMES.len())];
        let name = format!("{first}-{second}");
        if first != second && !taken.contains(&name) {
            return name;
        }
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

/// Weicher Übergang von 0 nach 1.
fn smooth(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
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
    grid: Res<Grid>,
    mut rabbits: Query<(
        &Rabbit,
        &Activity,
        &OnLayer,
        &Fur,
        &Transform,
        &Offset,
        &Path,
        &mut Anim,
        Has<Stomping>,
        &AvatarLink,
    )>,
    avatars: Query<(&Children, &Transform), (With<Avatar>, Without<Rabbit>)>,
    mut roots: Query<(&RigRoot, &mut Transform, &Children), (Without<Rabbit>, Without<Part>, Without<Avatar>)>,
    mut parts: Query<(&Part, &mut Transform), (Without<RigRoot>, Without<Rabbit>, Without<Avatar>)>,
    mut shadows: Query<&mut Visibility, With<FigureShadow>>,
) {
    let dt = time.delta_secs();
    let screen_right = orbit.screen_right();
    let billboard = orbit.billboard();

    for (rabbit, activity, layer, fur, transform, offset, path, mut anim, stomping, link) in &mut rabbits {
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
        // Im Fluchtloch hockt es sich hinein; wer nur durchhoppelt, taucht nicht ab.
        let in_hole = path.0.is_empty() && is_hidden(&grid, current_node(transform, layer));
        let duck_goal = if in_hole { 1.0 } else { 0.0 };
        anim.ducked += (duck_goal - anim.ducked).clamp(-dt * DUCK_SPEED, dt * DUCK_SPEED);
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
            if let Ok(mut visibility) = shadows.get_mut(child) {
                let wanted = if anim.ducked > DUCK_HIDE_BODY { Visibility::Hidden } else { Visibility::Inherited };
                visibility.set_if_neq(wanted);
            }
            let Ok((root, mut root_transform, root_children)) = roots.get_mut(child) else {
                continue;
            };
            let scale = root.pixel_scale * growth;
            // Sie sinkt entlang ihrer eigenen Hochachse, auf dem Bildschirm also gerade nach unten,
            // bis die Ohren dort beginnen, wo vorher die Füße standen. Dabei rückt sie in die Mitte des Lochs.
            let ducked = smooth(anim.ducked);
            let sink = billboard * Vec3::Y * DUCK_HEIGHT * scale * stretch_y * ducked;
            let to_middle = Vec3::new(-offset.0.x, 0.0, offset.0.y) * ducked;
            root_transform.translation = Vec3::new(0.0, lift * HOP_HEIGHT * growth, 0.0) - sink + to_middle;
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
                // Im Fluchtloch ist der Körper im Boden verschwunden; nur die Ohren schauen heraus.
                let ear = matches!(part.name.as_str(), "ohr_links" | "ohr_rechts");
                if !ear && anim.ducked > DUCK_HIDE_BODY {
                    part_transform.scale = Vec3::ZERO;
                }
            }
        }
    }
}

/// Erwachsene Kinder der Gründer, die mit umziehen: abwechselnd Töchter und Söhne,
/// damit die Familie auch nach einem Verlust weiter wachsen kann.
pub fn spawn_relatives(
    In(count): In<usize>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut rng: ResMut<Rng>,
    rabbits: Query<&Rabbit>,
) {
    let mut taken: Vec<String> = rabbits.iter().map(|r| r.name.clone()).collect();
    let near_entrance = TilePos::new(19, 15).center();
    for i in 0..count {
        let name = unused_name(&taken, &mut rng);
        taken.push(name.clone());
        let position = near_entrance + Vec2::new(rng.range(-60.0, 60.0), rng.range(-50.0, 50.0));
        let fur = FUR_COLORS[rng.below(FUR_COLORS.len())];
        let parents = Some(("Schneeflocke".to_string(), "Lukas".to_string()));
        commands.spawn(rabbit_bundle(
            &asset_server,
            name,
            i % 2 == 0,
            ADULT_AGE * 2.0,
            fur,
            Layer::Surface,
            position,
            parents,
            &mut rng,
        ));
    }
}

/// Ein zusätzliches erwachsenes Kaninchen, z. B. für Testläufe.
#[cfg(debug_assertions)]
pub fn spawn_extra(commands: &mut Commands, asset_server: &AssetServer, rng: &mut Rng, name: String, layer: Layer, position: Vec2) {
    let female = rng.chance(0.5);
    let fur = FUR_COLORS[rng.below(FUR_COLORS.len())];
    commands.spawn(rabbit_bundle(asset_server, name, female, ADULT_AGE * 2.0, fur, layer, position, None, rng));
}

/// Ein Kaninchen, wie es im Spielstand steht. Was es gerade tut, wird nicht gespeichert:
/// Nach dem Laden entscheidet es neu.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SavedRabbit {
    name: String,
    female: bool,
    age: f32,
    lifespan: f32,
    starving: f32,
    parents: Option<(String, String)>,
    fur: [f32; 4],
    hunger: f32,
    mood: f32,
    slept_badly: bool,
    layer: Layer,
    position: (f32, f32),
    carrying: f32,
    pregnant: Option<(f32, String)>,
    birth_cooldown: Option<f32>,
}

impl SavedRabbit {
    pub fn capture(
        rabbit: &Rabbit,
        fur: &Fur,
        hunger: &Hunger,
        mood: &Mood,
        layer: &OnLayer,
        transform: &Transform,
        carrying: &Carrying,
        pregnant: Option<&Pregnant>,
        cooldown: Option<&BirthCooldown>,
    ) -> Self {
        let position = transform.translation.truncate();
        Self {
            name: rabbit.name.clone(),
            female: rabbit.female,
            age: rabbit.age,
            lifespan: rabbit.lifespan,
            starving: rabbit.starving,
            parents: rabbit.parents.clone(),
            fur: fur.0.to_srgba().to_f32_array(),
            hunger: hunger.0,
            mood: mood.value,
            slept_badly: mood.slept_badly,
            layer: layer.0,
            position: (position.x, position.y),
            carrying: carrying.0,
            pregnant: pregnant.map(|p| (p.left, p.father.clone())),
            birth_cooldown: cooldown.map(|c| c.0),
        }
    }

    pub fn spawn(self, commands: &mut Commands, asset_server: &AssetServer, rng: &mut Rng) {
        let position = Vec2::new(self.position.0, self.position.1);
        let fur = Color::srgba(self.fur[0], self.fur[1], self.fur[2], self.fur[3]);
        let mut entity = commands.spawn(rabbit_bundle(
            asset_server,
            self.name.clone(),
            self.female,
            self.age,
            fur,
            self.layer,
            position,
            self.parents.clone(),
            rng,
        ));
        entity.insert((
            Rabbit {
                name: self.name,
                female: self.female,
                age: self.age,
                // Ältere Spielstände kannten noch längere Leben.
                lifespan: self.lifespan.min(LIFESPAN_YEARS.1 * YEAR_LENGTH),
                starving: self.starving,
                parents: self.parents,
            },
            Hunger(self.hunger),
            Mood {
                value: self.mood,
                gloomy_for: 0.0,
                slept_badly: self.slept_badly,
                bad_nights: 0,
                bad_night_day: 0,
                herbs: 0.0,
            },
            Carrying(self.carrying),
        ));
        if let Some((left, father)) = self.pregnant {
            entity.insert(Pregnant { left, father });
        }
        if let Some(left) = self.birth_cooldown {
            entity.insert(BirthCooldown(left));
        }
    }
}
