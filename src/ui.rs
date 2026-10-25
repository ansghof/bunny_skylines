//! Bildschirmanzeige: Status oben links, Meldungen oben rechts, Tastenhilfe unten.

use bevy::prelude::*;

use crate::colony::{Colony, Messages};
use crate::danger::AlarmButton;
use crate::daynight::DayClock;
use crate::map::{Grid, ViewLayer};
use crate::rabbit::Rabbit;
use crate::speed;
use crate::tools::{Tool, ToolKind};

const HELP: &str = "G Graben · E Eingang · N Schlafkammer · V Vorratskammer · P Wachposten · X Entfernen · R Tiefe · F Alarm\n\
Linke Maustaste ziehen: Bereich markieren · Mittlere Maustaste oder WASD: verschieben · Rechte Maustaste: drehen und neigen · Mausrad: Zoom";

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui).add_systems(Update, (update_status, update_messages));
    }
}

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct MessageText;

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
    commands.spawn((Text::new(HELP), panel(Some(12.0), None, None, Some(12.0))));
}

fn update_status(
    time: Res<Time<Virtual>>,
    clock: Res<DayClock>,
    alarm: Res<AlarmButton>,
    view: Res<ViewLayer>,
    tool: Res<Tool>,
    grid: Res<Grid>,
    colony: Res<Colony>,
    rabbits: Query<&Rabbit>,
    mut text: Single<&mut Text, With<StatusText>>,
) {
    let alarm = if alarm.cooldown > 0.0 {
        format!("noch {:.0} s", alarm.cooldown)
    } else {
        "bereit".into()
    };
    let total = rabbits.iter().count();
    let young = rabbits.iter().filter(|r| !r.is_adult()).count();
    let depth = if tool.kind == ToolKind::Dig {
        format!(", Tiefe {} [R]", tool.depth.label())
    } else {
        String::new()
    };
    text.0 = format!(
        "Tag {} · {}    Ansicht: {} [Tab]    Tempo: {} [Leertaste, 1–3]\n\
         Werkzeug: {}{depth}    Alarm [F]: {alarm}\n\
         Kaninchen: {total} (davon {young} Junge)    Schlafplätze: {}    Vorrat: {:.0} / {:.0}",
        clock.day,
        clock.phase().label(),
        view.0.label(),
        speed::label(&time),
        tool.kind.label(),
        Colony::sleeping_places(&grid),
        colony.food,
        Colony::food_capacity(&grid),
    );
}

fn update_messages(messages: Res<Messages>, mut text: Single<&mut Text, With<MessageText>>) {
    if messages.is_changed() {
        text.0 = messages.0.iter().cloned().collect::<Vec<_>>().join("\n");
    }
}
