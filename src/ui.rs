//! Bildschirmanzeige: Status oben links, Meldungen oben rechts, Tastenhilfe unten.

use bevy::prelude::*;

use crate::behavior::Priorities;
use crate::colony::{Colony, Messages};
use crate::danger::AlarmButton;
use crate::daynight::DayClock;
use crate::map::{Grid, ViewLayer};
use crate::rabbit::{Hunger, Mood, Rabbit, SHOW_HUNGER};
use crate::scenario::Scenario;
use crate::speed;
use crate::toolbar::TOOLBAR_HEIGHT;
use crate::tools::{SelectionCost, Tool, ToolKind};

/// Werkzeuge stehen in der Werkzeugleiste; hier nur noch die Kamera und das Markieren.
const HELP: &str = "Linke Maustaste ziehen: Bereich markieren\n\
Mittlere Maustaste oder WASD: verschieben\n\
Rechte Maustaste: drehen und neigen · Mausrad: Zoom";

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui)
            .add_systems(Update, (update_status, update_messages, update_selection_cost));
    }
}

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct MessageText;

/// Kosten des gerade aufgezogenen Bereichs, rechts über der Werkzeugleiste.
#[derive(Component)]
struct SelectionText;

fn panel(left: Option<f32>, right: Option<f32>, top: Option<f32>, bottom: Option<f32>) -> impl Bundle {
    let px = |v: Option<f32>| v.map_or(Val::Auto, Val::Px);
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            right: px(right),
            top: px(top),
            bottom: px(bottom),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.22, 0.15, 0.10, 0.78)),
        TextColor(Color::srgb(0.98, 0.95, 0.88)),
        TextFont {
            font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
            ..TextFont::from_font_size(15.0)
        },
    )
}

fn spawn_ui(mut commands: Commands) {
    commands.spawn((Text::default(), StatusText, panel(Some(12.0), None, Some(12.0), None)));
    commands.spawn((Text::default(), MessageText, panel(None, Some(12.0), Some(12.0), None)));
    // Über der Werkzeugleiste, damit sich beide nicht überdecken.
    commands.spawn((Text::new(HELP), panel(Some(12.0), None, None, Some(TOOLBAR_HEIGHT + 20.0))));
    // Versteckt sich selbst, solange kein Bereich aufgezogen wird.
    commands.spawn((Text::default(), SelectionText, panel(None, Some(12.0), None, Some(TOOLBAR_HEIGHT + 20.0))));
}

fn update_status(
    time: Res<Time<Virtual>>,
    clock: Res<DayClock>,
    alarm: Res<AlarmButton>,
    view: Res<ViewLayer>,
    tool: Res<Tool>,
    grid: Res<Grid>,
    colony: Res<Colony>,
    priorities: Res<Priorities>,
    rabbits: Query<(&Rabbit, &Hunger, &Mood)>,
    scenario: Res<Scenario>,
    mut text: Single<&mut Text, With<StatusText>>,
) {
    let alarm = if alarm.cooldown > 0.0 {
        format!("noch {:.0} s", alarm.cooldown)
    } else {
        "bereit".into()
    };
    let total = rabbits.iter().count();
    let young = rabbits.iter().filter(|(r, _, _)| !r.is_adult()).count();
    let hungry = rabbits.iter().filter(|(_, h, _)| h.0 >= SHOW_HUNGER).count();
    let starving = rabbits.iter().filter(|(r, _, _)| r.is_starving()).count();
    // Eine leere Summe ist -0.0; ohne max stünde dort „-0 %“.
    let mood = (rabbits.iter().map(|(_, _, m)| m.value).sum::<f32>() / total.max(1) as f32).max(0.0);
    let unhappy = rabbits.iter().filter(|(_, _, m)| m.is_unhappy()).count();
    let mood = match unhappy {
        0 => format!("    Stimmung: {:.0} %", mood * 100.0),
        n => format!("    Stimmung: {:.0} %, {n} unglücklich", mood * 100.0),
    };
    let hunger = match (hungry, starving) {
        (0, _) => String::new(),
        (n, 0) => format!("    Hungrig: {n}"),
        (n, s) => format!("    Hungrig: {n}, davon {s} am Verhungern!"),
    };
    let depth = if tool.kind == ToolKind::Dig {
        format!(", Tiefe {} [R]", tool.depth.label())
    } else {
        String::new()
    };
    let price = tool.kind.cost_label().map_or(String::new(), |c| format!(" ({c})"));
    let gathering = if priorities.gather_first { "    Vorräte anlegen [B]" } else { "" };
    text.0 = format!(
        "Jahr {} · {} · Tag {} · {}    Ansicht: {} [Tab]    Tempo: {} [Leertaste, 1–3]\n\
         Werkzeug: {}{price}{depth}    Alarm [F]: {alarm}\n\
         Kaninchen: {total} (davon {young} Junge)    Schlafplätze: {}{hunger}\n\
         Futter: {:.0} / {:.0}    Erde: {:.0}{mood}{gathering}",
        clock.year(),
        clock.season().label(),
        clock.day_of_year(),
        clock.phase().label(),
        view.0.label(),
        speed::label(&time),
        tool.kind.label(),
        Colony::sleeping_places(&grid),
        colony.food,
        colony.food_capacity(&grid),
        colony.earth,
    );
    if let Some(goals) = scenario.goal_line(total, colony.food) {
        text.0 += &format!("
{goals}");
    }
}

fn update_selection_cost(
    selection: Res<SelectionCost>,
    mut panel: Single<(&mut Text, &mut Node), With<SelectionText>>,
) {
    if !selection.is_changed() {
        return;
    }
    let (text, node) = &mut *panel;
    match &selection.0 {
        Some(info) => {
            text.0 = info.clone();
            node.display = Display::Flex;
        }
        None => node.display = Display::None,
    }
}

fn update_messages(messages: Res<Messages>, mut text: Single<&mut Text, With<MessageText>>) {
    if messages.is_changed() {
        text.0 = messages.0.iter().cloned().collect::<Vec<_>>().join("\n");
    }
}
