//! Kaninchen anklicken: Ein Infofeld zeigt Name, Alter, Eltern, Tätigkeit, Stimmung mit ihren
//! Gründen und einen kurzen Gedanken. So sieht man, warum jemand unglücklich ist.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::behavior::{Activity, Fear, Goal};
use crate::camera::MainCamera;
use crate::daynight::{DAY_LENGTH, DayClock, Season, YEAR_LENGTH};
use crate::ground::{floor_y, gizmo_circle, to_world3};
use crate::map::{Feature, Grid, TilePos, ViewLayer};
use crate::rabbit::{Hunger, Mood, OnLayer, Pregnant, Rabbit, SHOW_HUNGER, near_sun_spot};
use crate::toolbar::Toolbar;
use crate::tools::{Drag, apply_drag};

/// So weit darf sich die Maus zwischen Drücken und Loslassen bewegen, damit es ein Klick ist.
const CLICK_SLOP: f32 = 6.0;
/// So hoch ist ein Kaninchen ungefähr, für die Trefferprüfung.
pub const RABBIT_HEIGHT: f32 = 48.0;

pub struct InspectPlugin;

impl Plugin for InspectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selected>()
            .add_systems(Startup, spawn_panel)
            .add_systems(Update, (pick_rabbit.before(apply_drag), show_info, highlight).chain());
    }
}

/// Das angeklickte Kaninchen.
#[derive(Resource, Default)]
pub struct Selected(pub Option<Entity>);

#[derive(Component)]
struct InfoPanel;

#[derive(Component)]
struct InfoText;

fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            InfoPanel,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(134.0),
                width: Val::Px(330.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.22, 0.15, 0.10, 0.88)),
            BorderColor::all(Color::srgb(0.95, 0.80, 0.45)),
        ))
        .with_child((
            InfoText,
            Text::default(),
            TextColor(Color::srgb(0.98, 0.95, 0.88)),
            TextFont {
                font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
                ..TextFont::from_font_size(15.0)
            },
        ));
}

/// Ein Klick ohne Ziehen auf ein Kaninchen wählt es aus, statt das Werkzeug anzuwenden.
/// Ein Klick ins Leere oder Esc schließt das Infofeld wieder.
#[allow(clippy::too_many_arguments)]
pub fn pick_rabbit(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    view: Res<ViewLayer>,
    grid: Res<Grid>,
    rabbits: Query<(Entity, &Transform, &OnLayer, &Activity), With<Rabbit>>,
    toolbar: Query<&Hovered, With<Toolbar>>,
    mut drag: ResMut<Drag>,
    mut selected: ResMut<Selected>,
    mut pressed_at: Local<Option<Vec2>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        selected.0 = None;
    }
    let cursor = window.cursor_position();
    if buttons.just_pressed(MouseButton::Left) {
        *pressed_at = cursor;
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    let (Some(start), Some(cursor)) = (pressed_at.take(), cursor) else {
        return;
    };
    if start.distance(cursor) > CLICK_SLOP || toolbar.iter().any(|hovered| hovered.0) {
        return;
    }

    let (camera, camera_transform) = *camera;
    let on_screen = |point: Vec3| camera.world_to_viewport(camera_transform, point).ok();
    let mut best: Option<(Entity, f32)> = None;
    for (entity, transform, layer, activity) in &rabbits {
        if layer.0 != view.0 || matches!(activity, Activity::Gone) {
            continue;
        }
        let position = transform.translation.truncate();
        let floor = floor_y(&grid, layer.0, TilePos::from_world_clamped(position));
        let (Some(foot), Some(head)) = (
            on_screen(to_world3(position, floor)),
            on_screen(to_world3(position, floor + RABBIT_HEIGHT)),
        ) else {
            continue;
        };
        // Abstand zur Strecke von den Füßen bis zum Kopf, so trifft man die Figur aus jedem Winkel.
        let reach = (foot.distance(head) * 0.4).max(14.0);
        let distance = distance_to_segment(cursor, foot, head);
        if distance < reach && best.is_none_or(|(_, d)| distance < d) {
            best = Some((entity, distance));
        }
    }
    match best {
        Some((entity, _)) => {
            selected.0 = Some(entity);
            drag.cancel();
        }
        None => selected.0 = None,
    }
}

fn distance_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 { ((point - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    point.distance(a + ab * t)
}

#[allow(clippy::too_many_arguments)]
fn show_info(
    mut selected: ResMut<Selected>,
    clock: Res<DayClock>,
    grid: Res<Grid>,
    rabbits: Query<(&Rabbit, &Hunger, &Fear, &Mood, &Activity, &Transform, &OnLayer, Has<Pregnant>)>,
    mut panel: Single<&mut Node, With<InfoPanel>>,
    mut text: Single<&mut Text, With<InfoText>>,
) {
    let found = selected.0.and_then(|entity| rabbits.get(entity).ok());
    let Some((rabbit, hunger, fear, mood, activity, transform, layer, pregnant)) = found else {
        // Gestorben, fortgezogen oder nichts ausgewählt.
        selected.0 = None;
        if panel.display != Display::None {
            panel.display = Display::None;
        }
        return;
    };
    if panel.display != Display::Flex {
        panel.display = Display::Flex;
    }

    let sun = layer.0 == crate::map::Layer::Surface && near_sun_spot(&grid, transform);
    let playground = Grid::positions().any(|p| grid.surface(p).feature == Some(Feature::Playground));
    let hungry = hunger.0 >= SHOW_HUNGER;
    let scared = fear.0 > 0.0 || matches!(activity, Activity::Fleeing);

    let mut lines = Vec::new();
    let sex = if rabbit.female { "♀" } else { "♂" };
    lines.push(format!("{} {sex}", rabbit.name));
    lines.push(format!("Alter: {}", age_text(rabbit)));
    lines.push(match &rabbit.parents {
        Some((mother, father)) => format!("Eltern: {mother} und {father}"),
        None if rabbit.female => "Gründerin der Familie Buddelberg".into(),
        None => "Gründer der Familie Buddelberg".into(),
    });
    let mut doing = activity_text(activity).to_string();
    if pregnant {
        doing += ", erwartet Nachwuchs";
    }
    lines.push(format!("Tut gerade: {doing}"));
    lines.push(format!("Hunger: {}", hunger_text(rabbit, hunger.0)));
    lines.push(format!("Stimmung: {:.0} % ({})", mood.value * 100.0, mood_word(mood.value)));

    let mut reasons = Vec::new();
    if playground {
        reasons.push("+ Spielwiese");
    }
    if sun {
        reasons.push("+ Sonnenplatz in der Nähe");
    }
    if mood.herbs > 0.0 {
        reasons.push("+ hat Kräuter gefressen");
    }
    if hungry {
        reasons.push("− hungrig");
    }
    if scared {
        reasons.push("− hat Angst");
    }
    if mood.slept_badly {
        reasons.push("− fand keinen Schlafplatz");
    }
    for reason in reasons {
        lines.push(format!("   {reason}"));
    }

    let thought = thought(rabbit, mood, activity, hungry, scared, sun, pregnant, clock.season());
    lines.push(String::new());
    lines.push(format!("„{thought}“"));
    lines.push(String::new());
    lines.push("[Esc] schließen".into());

    let content = lines.join("\n");
    if text.0 != content {
        text.0 = content;
    }
}

fn age_text(rabbit: &Rabbit) -> String {
    let stage = if !rabbit.is_adult() {
        "Junges"
    } else if rabbit.is_old() {
        "alt"
    } else {
        "erwachsen"
    };
    if rabbit.age < YEAR_LENGTH {
        match (rabbit.age / DAY_LENGTH).floor() as u32 {
            0 => format!("jünger als ein Tag, {stage}"),
            1 => format!("1 Tag, {stage}"),
            days => format!("{days} Tage, {stage}"),
        }
    } else {
        let years = (rabbit.age / YEAR_LENGTH).floor() as u32;
        let unit = if years == 1 { "Jahr" } else { "Jahre" };
        format!("{years} {unit}, {stage}")
    }
}

fn hunger_text(rabbit: &Rabbit, hunger: f32) -> &'static str {
    if rabbit.is_starving() {
        "verhungert fast!"
    } else if hunger >= SHOW_HUNGER {
        "sehr hungrig"
    } else if hunger >= 0.5 {
        "hungrig"
    } else {
        "satt"
    }
}

fn mood_word(value: f32) -> &'static str {
    match value {
        v if v >= 0.75 => "zufrieden",
        v if v >= 0.5 => "geht so",
        v if v >= 0.3 => "unglücklich",
        _ => "will fort",
    }
}

fn activity_text(activity: &Activity) -> &'static str {
    match activity {
        Activity::Idle(_) => "schaut sich um",
        Activity::Fleeing => "flieht",
        Activity::Eating { .. } => "frisst",
        Activity::Gathering { .. } => "sammelt Gras",
        Activity::Digging { .. } => "gräbt",
        Activity::Guarding { .. } => "hält Wache",
        Activity::Sleeping { .. } => "schläft",
        Activity::Gone => "ist fort",
        Activity::Listening { .. } => "hört Geschichten zu",
        Activity::Walking(goal) => match goal {
            Goal::Eat(_) => "hoppelt zum Grasen",
            Goal::Gather(_) => "hoppelt zum Sammeln",
            Goal::Dig(_) => "hoppelt zum Graben",
            Goal::Guard(_) => "hoppelt zum Wachposten",
            Goal::Store(_) => "bringt Futter in den Vorrat",
            Goal::EatFromStore(_) => "hoppelt zum Vorrat",
            Goal::Sleep(_) => "hoppelt ins Nest",
            Goal::Wander => "hoppelt herum",
            Goal::Leave => "zieht fort",
            Goal::Listen(_) => "hoppelt zur Erzählkammer",
        },
    }
}

/// Was dem Kaninchen gerade durch den Kopf geht; das Dringendste zuerst.
#[allow(clippy::too_many_arguments)]
fn thought(
    rabbit: &Rabbit,
    mood: &Mood,
    activity: &Activity,
    hungry: bool,
    scared: bool,
    sun: bool,
    pregnant: bool,
    season: Season,
) -> &'static str {
    if matches!(activity, Activity::Walking(Goal::Leave)) {
        return "Hier halte ich es nicht mehr aus. Ich suche mir eine eigene Wiese.";
    }
    if rabbit.is_starving() {
        return "Mir ist ganz schwach vor Hunger …";
    }
    if scared {
        return "Gefahr! Schnell in Deckung!";
    }
    if hungry {
        return if season == Season::Winter {
            "Mein Bauch knurrt. Ist im Vorrat noch etwas?"
        } else {
            "Mein Bauch knurrt. Wo gibt es noch gutes Gras?"
        };
    }
    if mood.slept_badly {
        return "Letzte Nacht war kein Nest für mich frei. Wir brauchen mehr Schlafplätze!";
    }
    match activity {
        Activity::Sleeping { .. } => return "Zzz …",
        Activity::Listening { .. } => return "Die Geschichten der Alten sind so spannend!",
        _ => {}
    }
    if pregnant {
        return "Bald kommen meine Kleinen!";
    }
    if mood.herbs > 0.0 {
        return "Die Kräuter haben herrlich geschmeckt.";
    }
    if sun {
        return "Die Sonne wärmt so schön.";
    }
    match activity {
        Activity::Digging { .. } => return "Buddeln macht Spaß!",
        Activity::Guarding { .. } => return "Ich halte Wache. Niemand schleicht sich hier an!",
        Activity::Gathering { .. } | Activity::Walking(Goal::Store(_)) => return "Für den Winter muss der Vorrat voll sein.",
        _ => {}
    }
    if season == Season::Winter {
        return "Brr! Hoffentlich ist bald Frühling.";
    }
    if !rabbit.is_adult() {
        return "Wann bin ich endlich groß?";
    }
    if rabbit.is_old() {
        return "Früher war die Wiese noch viel größer.";
    }
    if mood.value >= 0.75 { "Mir geht es gut hier." } else { "Ach, es könnte besser sein." }
}

/// Ein Ring zu Füßen des ausgewählten Kaninchens.
fn highlight(
    mut gizmos: Gizmos,
    selected: Res<Selected>,
    grid: Res<Grid>,
    view: Res<ViewLayer>,
    rabbits: Query<(&Transform, &OnLayer)>,
) {
    let Some((transform, layer)) = selected.0.and_then(|entity| rabbits.get(entity).ok()) else {
        return;
    };
    if layer.0 != view.0 {
        return;
    }
    let position = transform.translation.truncate();
    let floor = floor_y(&grid, layer.0, TilePos::from_world_clamped(position));
    gizmo_circle(&mut gizmos, position, 20.0, floor, Color::srgb(0.98, 0.85, 0.45));
}
