//! Werkzeugleiste unten: Knöpfe mit Icons für Werkzeuge, Grabtiefe, Alarm, Tempo und Ansicht.
//! Fährt man mit der Maus darüber, erscheint ein Tooltip mit Name, Taste, Preis und Beschreibung.

use bevy::picking::Pickable;
use bevy::picking::hover::Hovered;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::behavior::Priorities;
use crate::danger::{ALARM_COOLDOWN_SECONDS, AlarmButton};
use crate::map::{Depth, Layer, ViewLayer};
use crate::tools::{TOOLS, Tool, ToolKind, select};

const BUTTON_SIZE: f32 = 52.0;
const ICON_SIZE: f32 = 40.0;
const PANEL: Color = Color::srgba(0.22, 0.15, 0.10, 0.85);
const BUTTON: Color = Color::srgb(0.36, 0.26, 0.18);
const BUTTON_HOVER: Color = Color::srgb(0.48, 0.36, 0.25);
/// Knöpfe, die gerade nichts bewirken (Grabtiefe ohne Graben, Alarm in der Sperrzeit).
const BUTTON_DIMMED: Color = Color::srgb(0.27, 0.20, 0.15);
const BORDER_ACTIVE: Color = Color::srgb(0.98, 0.80, 0.35);
const BORDER_HOVER: Color = Color::srgb(0.90, 0.84, 0.72);
const TEXT: Color = Color::srgb(0.98, 0.95, 0.88);

pub struct ToolbarPlugin;

impl Plugin for ToolbarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tooltip>()
            .add_systems(Startup, spawn_toolbar)
            .add_systems(Update, (toggle_gather_key, highlight_buttons, update_tooltip));
    }
}

/// Die Leiste selbst; `Hovered` sagt, ob die Maus gerade über ihr ist.
#[derive(Component)]
pub struct Toolbar;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Action {
    Tool(ToolKind),
    Depth(Depth),
    Alarm,
    /// Alle Erwachsenen sammeln zuerst.
    GatherFirst,
    /// `None` = Pause.
    Speed(Option<f32>),
    View(Layer),
}

#[derive(Component)]
struct ActionButton(Action);

#[derive(Component)]
struct TooltipPanel;

#[derive(Component)]
struct TooltipText;

/// Über welchem Knopf die Maus gerade ist.
#[derive(Resource, Default)]
pub(crate) struct Tooltip(pub(crate) Option<Action>);

impl Action {
    fn icon(self) -> Option<&'static str> {
        Some(match self {
            Action::Tool(ToolKind::Dig) => "graben",
            Action::Tool(ToolKind::Entrance) => "eingang",
            Action::Tool(ToolKind::Chamber(crate::map::Chamber::Sleeping)) => "schlafkammer",
            Action::Tool(ToolKind::Chamber(crate::map::Chamber::Storage)) => "vorratskammer",
            Action::Tool(ToolKind::Post) => "wachposten",
            Action::Tool(ToolKind::Clover) => "kleefeld",
            Action::Tool(ToolKind::Remove) => "entfernen",
            Action::Depth(Depth::Shallow) => "tiefe_flach",
            Action::Depth(Depth::Medium) => "tiefe_mittel",
            Action::Depth(Depth::Deep) => "tiefe_tief",
            Action::Alarm => "alarm",
            Action::GatherFirst => "sammeln",
            Action::View(Layer::Surface) => "ansicht_oben",
            Action::View(Layer::Underground) => "ansicht_unten",
            Action::Speed(_) => return None,
        })
    }

    /// Beschriftung für Knöpfe ohne Icon.
    fn caption(self) -> &'static str {
        match self {
            Action::Speed(None) => "II",
            Action::Speed(Some(s)) if s < 1.5 => "1×",
            Action::Speed(Some(s)) if s < 2.5 => "2×",
            _ => "3×",
        }
    }

    fn tooltip(self, alarm: &AlarmButton) -> String {
        match self {
            Action::Tool(kind) => {
                let key = TOOLS.iter().find(|(k, _, _)| *k == kind).map_or("", |(_, _, key)| *key);
                let cost = kind.cost_label().map_or(String::new(), |c| format!("\nKosten: {c}"));
                format!("{}  [{key}]\n{}{cost}", kind.label(), kind.description())
            }
            Action::Depth(depth) => format!(
                "Grabtiefe {}  [R wechselt]\nNeue Gänge werden {} gegraben, {:.1} s pro Feld.",
                depth.label(),
                depth.label(),
                depth.dig_seconds()
            ),
            Action::Alarm => {
                let state = if alarm.cooldown > 0.0 {
                    format!("\nWieder bereit in {:.0} s.", alarm.cooldown)
                } else {
                    String::new()
                };
                format!(
                    "Alarm  [F]\nAlle Kaninchen draußen flüchten sofort in den Bau. \
                     Danach {ALARM_COOLDOWN_SECONDS:.0} s gesperrt.{state}"
                )
            }
            Action::GatherFirst => "Vorräte anlegen  [B]
Alle Erwachsenen sammeln Gras für den Vorrat,                  Graben und Wache haben Pause. Nochmal klicken zum Beenden."
                .into(),
            Action::Speed(None) => "Pause  [Leertaste]".into(),
            Action::Speed(Some(speed)) => format!("Tempo {speed}×  [{speed}]"),
            Action::View(layer) => format!("Ansicht: {}  [Tab]", layer.label()),
        }
    }
}

fn spawn_toolbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    let groups: [Vec<Action>; 5] = [
        TOOLS.iter().map(|(kind, _, _)| Action::Tool(*kind)).collect(),
        [Depth::Shallow, Depth::Medium, Depth::Deep].map(Action::Depth).to_vec(),
        vec![Action::GatherFirst, Action::Alarm],
        vec![
            Action::Speed(None),
            Action::Speed(Some(1.0)),
            Action::Speed(Some(2.0)),
            Action::Speed(Some(3.0)),
        ],
        vec![Action::View(Layer::Surface), Action::View(Layer::Underground)],
    ];

    // Volle Breite nur zum Zentrieren; sie selbst fängt keine Mausklicks ab.
    let row = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let bar = commands
        .spawn((
            Toolbar,
            Hovered::default(),
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            ChildOf(row),
        ))
        .id();

    for (index, group) in groups.into_iter().enumerate() {
        if index > 0 {
            commands.spawn((
                Node {
                    width: Val::Px(2.0),
                    height: Val::Px(BUTTON_SIZE - 12.0),
                    margin: UiRect::horizontal(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(BUTTON_DIMMED),
                Pickable::IGNORE,
                ChildOf(bar),
            ));
        }
        for action in group {
            spawn_button(&mut commands, &asset_server, bar, action);
        }
    }

    // Tooltip über der Leiste, ebenfalls über die volle Breite zentriert.
    let tooltip_row = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(BUTTON_SIZE + 36.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let panel = commands
        .spawn((
            TooltipPanel,
            Node {
                display: Display::None,
                max_width: Val::Px(460.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            Pickable::IGNORE,
            ChildOf(tooltip_row),
        ))
        .id();
    commands.spawn((TooltipText, Text::default(), text_style(15.0), Pickable::IGNORE, ChildOf(panel)));
}

fn text_style(size: f32) -> impl Bundle {
    (
        TextColor(TEXT),
        TextFont {
            font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
            ..TextFont::from_font_size(size)
        },
    )
}

fn spawn_button(commands: &mut Commands, asset_server: &AssetServer, bar: Entity, action: Action) {
    let button = commands
        .spawn((
            ActionButton(action),
            Hovered::default(),
            Node {
                width: Val::Px(BUTTON_SIZE),
                height: Val::Px(BUTTON_SIZE),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(BUTTON),
            BorderColor::all(Color::NONE),
            ChildOf(bar),
        ))
        .observe(move |click: On<PointerClick>, mut world: ActionTargets| {
            if click.button == PointerButton::Primary {
                world.perform(action);
            }
        })
        .observe(move |_: On<PointerOver>, mut tooltip: ResMut<Tooltip>| tooltip.0 = Some(action))
        .observe(move |_: On<PointerOut>, mut tooltip: ResMut<Tooltip>| {
            if tooltip.0 == Some(action) {
                tooltip.0 = None;
            }
        })
        .id();

    match action.icon() {
        Some(icon) => commands.spawn((
            ImageNode::new(asset_server.load(format!("ui/{icon}.png"))),
            Node {
                width: Val::Px(ICON_SIZE),
                height: Val::Px(ICON_SIZE),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(button),
        )),
        None => commands.spawn((
            Text::new(action.caption()),
            text_style(18.0),
            Pickable::IGNORE,
            ChildOf(button),
        )),
    };
}

/// Alles, was ein Knopf verändern kann.
#[derive(bevy::ecs::system::SystemParam)]
struct ActionTargets<'w> {
    tool: ResMut<'w, Tool>,
    view: ResMut<'w, ViewLayer>,
    time: ResMut<'w, Time<Virtual>>,
    alarm: ResMut<'w, AlarmButton>,
    priorities: ResMut<'w, Priorities>,
}

impl ActionTargets<'_> {
    fn perform(&mut self, action: Action) {
        match action {
            Action::Tool(kind) => select(kind, &mut self.tool, &mut self.view),
            // Eine Grabtiefe zu wählen heißt graben wollen.
            Action::Depth(depth) => {
                self.tool.depth = depth;
                select(ToolKind::Dig, &mut self.tool, &mut self.view);
            }
            Action::Alarm => self.alarm.requested = true,
            Action::GatherFirst => self.priorities.gather_first = !self.priorities.gather_first,
            Action::Speed(None) => {
                if self.time.is_paused() {
                    self.time.unpause();
                } else {
                    self.time.pause();
                }
            }
            Action::Speed(Some(speed)) => {
                self.time.set_relative_speed(speed);
                self.time.unpause();
            }
            Action::View(layer) => self.view.0 = layer,
        }
    }
}

fn toggle_gather_key(keys: Res<ButtonInput<KeyCode>>, mut priorities: ResMut<Priorities>) {
    if keys.just_pressed(KeyCode::KeyB) {
        priorities.gather_first = !priorities.gather_first;
    }
}

fn highlight_buttons(
    tool: Res<Tool>,
    view: Res<ViewLayer>,
    time: Res<Time<Virtual>>,
    alarm: Res<AlarmButton>,
    priorities: Res<Priorities>,
    mut buttons: Query<(&ActionButton, &Hovered, &mut BackgroundColor, &mut BorderColor)>,
) {
    for (button, hovered, mut background, mut border) in &mut buttons {
        let (active, dimmed) = match button.0 {
            Action::Tool(kind) => (tool.kind == kind, false),
            Action::Depth(depth) => (tool.depth == depth, tool.kind != ToolKind::Dig),
            Action::Alarm => (false, alarm.cooldown > 0.0),
            Action::GatherFirst => (priorities.gather_first, false),
            Action::Speed(None) => (time.is_paused(), false),
            Action::Speed(Some(speed)) => (!time.is_paused() && time.relative_speed() == speed, false),
            Action::View(layer) => (view.0 == layer, false),
        };
        let fill = match (hovered.0, dimmed) {
            (true, _) => BUTTON_HOVER,
            (false, true) => BUTTON_DIMMED,
            (false, false) => BUTTON,
        };
        let edge = if active {
            BORDER_ACTIVE
        } else if hovered.0 {
            BORDER_HOVER
        } else {
            Color::NONE
        };
        if background.0 != fill {
            background.0 = fill;
        }
        if border.top != edge {
            *border = BorderColor::all(edge);
        }
    }
}

fn update_tooltip(
    tooltip: Res<Tooltip>,
    alarm: Res<AlarmButton>,
    mut text: Single<&mut Text, With<TooltipText>>,
    mut panel: Single<&mut Node, With<TooltipPanel>>,
) {
    let Some(action) = tooltip.0 else {
        if panel.display != Display::None {
            panel.display = Display::None;
        }
        return;
    };
    if panel.display != Display::Flex {
        panel.display = Display::Flex;
    }
    // Jedes Bild neu, damit die Sperrzeit des Alarms mitläuft.
    let content = action.tooltip(&alarm);
    if text.0 != content {
        text.0 = content;
    }
}
