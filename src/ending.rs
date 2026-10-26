//! Das Ende einer Familie: Ist kein Kaninchen mehr übrig, hält das Spiel an und zeigt
//! eine kleine Chronik. Ein Knopf beginnt mit neuen Gründern von vorn.

use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::behavior::{Priorities, Reserved};
use crate::colony::{Colony, Messages};
use crate::danger::AlarmButton;
use crate::daynight::DayClock;
use crate::map::{Grid, Layer, ViewLayer};
use crate::predator::{Fox, FoxSpawner};
use crate::progress::Progress;
use crate::rabbit::{Rabbit, spawn_founders};

const PANEL: Color = Color::srgba(0.22, 0.15, 0.10, 0.94);
const BUTTON: Color = Color::srgb(0.45, 0.33, 0.22);
const TEXT: Color = Color::srgb(0.98, 0.95, 0.88);

pub struct EndingPlugin;

impl Plugin for EndingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Chronicle>()
            .add_systems(Update, (keep_chronicle, notice_extinction).chain());
    }
}

/// Was über die Familie festgehalten wird.
#[derive(Resource, Default)]
struct Chronicle {
    largest_family: usize,
    over: bool,
}

#[derive(Component)]
pub(crate) struct EndScreen;

fn keep_chronicle(mut chronicle: ResMut<Chronicle>, rabbits: Query<(), With<Rabbit>>) {
    chronicle.largest_family = chronicle.largest_family.max(rabbits.iter().count());
}

fn notice_extinction(
    mut commands: Commands,
    clock: Res<DayClock>,
    mut chronicle: ResMut<Chronicle>,
    mut time: ResMut<Time<Virtual>>,
    rabbits: Query<(), With<Rabbit>>,
) {
    // Erst wenn es überhaupt einmal Kaninchen gab, sonst endet das Spiel vor dem ersten Bild.
    if chronicle.over || chronicle.largest_family == 0 || !rabbits.is_empty() {
        return;
    }
    chronicle.over = true;
    time.pause();

    let text = (
        TextColor(TEXT),
        TextFont {
            font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
            ..TextFont::from_font_size(18.0)
        },
    );
    let screen = commands
        .spawn((
            EndScreen,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.04, 0.03, 0.55)),
            // Über allem anderen, auch über der Werkzeugleiste.
            GlobalZIndex(10),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(28.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            ChildOf(screen),
        ))
        .id();
    commands.spawn((
        Text::new("Die Familie Buddelberg ist ausgestorben."),
        TextColor(TEXT),
        TextFont {
            font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
            ..TextFont::from_font_size(26.0)
        },
        ChildOf(panel),
    ));
    commands.spawn((
        Text::new(format!(
            "Sie lebte bis zum {} in Jahr {}.\nIn ihrer besten Zeit waren es {} Kaninchen.",
            clock.season().label(),
            clock.year(),
            chronicle.largest_family
        )),
        TextLayout::justify(Justify::Center),
        text.clone(),
        ChildOf(panel),
    ));
    let button = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(22.0), Val::Px(10.0)),
                margin: UiRect::top(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(BUTTON),
            ChildOf(panel),
        ))
        .observe(|click: On<PointerClick>, mut commands: Commands| {
            if click.button == PointerButton::Primary {
                commands.run_system_cached(restart);
            }
        })
        .id();
    commands.spawn((Text::new("Neu beginnen"), text, ChildOf(button)));
}

/// Alles zurück auf Anfang: neue Wiese, neue Gründer, erster Frühlingstag.
pub(crate) fn restart(world: &mut World) {
    let mut doomed = world.query_filtered::<Entity, Or<(With<Rabbit>, With<Fox>, With<EndScreen>)>>();
    let doomed: Vec<Entity> = doomed.iter(world).collect();
    for entity in doomed {
        world.despawn(entity);
    }

    world.insert_resource(Grid::starting_meadow());
    world.insert_resource(Colony::default());
    world.insert_resource(DayClock::default());
    world.insert_resource(Reserved::default());
    world.insert_resource(Priorities::default());
    world.insert_resource(FoxSpawner::default());
    world.insert_resource(AlarmButton::default());
    world.insert_resource(Chronicle::default());
    world.insert_resource(Progress::default());
    world.insert_resource(ViewLayer(Layer::Surface));
    world.resource_mut::<Messages>().0.clear();
    let mut time = world.resource_mut::<Time<Virtual>>();
    time.set_relative_speed(1.0);
    time.unpause();

    world.run_system_cached(spawn_founders).ok();
}
