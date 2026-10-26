//! Werkzeugleiste unten: oben die Bauwerkzeuge, darunter Grabtiefe, Vorräte, Alarm, Tempo,
//! Ansicht und der Forschungsbaum. Fährt man mit der Maus darüber, erscheint ein Tooltip mit
//! Name, Taste, Preis und Beschreibung. Noch nicht Erforschtes trägt ein Schloss.

use bevy::picking::Pickable;
use bevy::picking::hover::Hovered;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::behavior::Priorities;
use crate::colony::Messages;
use crate::danger::{ALARM_COOLDOWN_SECONDS, AlarmButton};
use crate::map::{Depth, Layer, ViewLayer};
use crate::menu::OpenMenu;
use crate::overlay::MapOverlay;
use crate::progress::{Blocked, Branch, Progress, Tech};
use crate::tools::{TOOLS, Tool, ToolKind, depth_available, locked_message, select};

const BUTTON_SIZE: f32 = 46.0;
const ICON_SIZE: f32 = 36.0;
const PANEL: Color = Color::srgba(0.22, 0.15, 0.10, 0.88);
const BUTTON: Color = Color::srgb(0.36, 0.26, 0.18);
const BUTTON_HOVER: Color = Color::srgb(0.48, 0.36, 0.25);
/// Knöpfe, die gerade nichts bewirken (Grabtiefe ohne Graben, Alarm in der Sperrzeit, Gesperrtes).
const BUTTON_DIMMED: Color = Color::srgb(0.27, 0.20, 0.15);
const BORDER_ACTIVE: Color = Color::srgb(0.98, 0.80, 0.35);
const BORDER_HOVER: Color = Color::srgb(0.90, 0.84, 0.72);
const TEXT: Color = Color::srgb(0.98, 0.95, 0.88);
const TEXT_QUIET: Color = Color::srgb(0.72, 0.66, 0.58);
/// Höhe der zweireihigen Leiste; andere Anzeigen sitzen darüber.
pub const TOOLBAR_HEIGHT: f32 = 2.0 * BUTTON_SIZE + 22.0;

pub struct ToolbarPlugin;

impl Plugin for ToolbarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tooltip>()
            .init_resource::<ResearchOpen>()
            .add_systems(Startup, (spawn_toolbar, spawn_research_panel))
            .add_systems(
                Update,
                (toolbar_keys, highlight_buttons, update_tooltip, show_research_panel, update_research_panel),
            );
    }
}

/// Bereiche der Bedienung; `Hovered` sagt, ob die Maus gerade darüber ist.
/// Klicks dort gehen nicht in die Welt.
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
    /// Den Forschungsbaum öffnen oder schließen.
    Research,
    /// Eine Übersichtskarte ein- oder ausschalten.
    Overlay(MapOverlay),
    /// Das Pausenmenü öffnen.
    Menu,
}

#[derive(Component)]
struct ActionButton(Action);

/// Das Schloss über einem gesperrten Knopf.
#[derive(Component)]
struct LockIcon;

#[derive(Component)]
struct TooltipPanel;

#[derive(Component)]
struct TooltipText;

/// Über welchem Knopf die Maus gerade ist.
#[derive(Resource, Default)]
pub(crate) struct Tooltip(pub(crate) Option<Action>);

/// Ob der Forschungsbaum offen ist.
#[derive(Resource, Default)]
pub(crate) struct ResearchOpen(pub(crate) bool);

impl Action {
    fn icon(self) -> Option<&'static str> {
        use crate::map::{Chamber, Feature};
        Some(match self {
            Action::Tool(ToolKind::Point) => "zeigen",
            Action::Tool(ToolKind::Dig) => "graben",
            Action::Tool(ToolKind::Entrance) => "eingang",
            Action::Tool(ToolKind::Chamber(Chamber::Sleeping)) => "schlafkammer",
            Action::Tool(ToolKind::Chamber(Chamber::Storage)) => "vorratskammer",
            Action::Tool(ToolKind::Chamber(Chamber::Story)) => "erzaehlkammer",
            Action::Tool(ToolKind::Chamber(Chamber::Nursery)) => "kinderstube",
            Action::Tool(ToolKind::Post) => "wachposten",
            Action::Tool(ToolKind::Clover) => "kleefeld",
            Action::Tool(ToolKind::Feature(Feature::Hedge)) => "hecke",
            Action::Tool(ToolKind::Feature(Feature::BoltHole)) => "fluchtloch",
            Action::Tool(ToolKind::Feature(Feature::SunSpot)) => "sonnenplatz",
            Action::Tool(ToolKind::Feature(Feature::Herbs)) => "kraeuter",
            Action::Tool(ToolKind::Feature(Feature::Playground)) => "spielwiese",
            Action::Tool(ToolKind::Remove) => "entfernen",
            Action::Depth(Depth::Shallow) => "tiefe_flach",
            Action::Depth(Depth::Medium) => "tiefe_mittel",
            Action::Depth(Depth::Deep) => "tiefe_tief",
            Action::Alarm => "alarm",
            Action::GatherFirst => "sammeln",
            Action::View(Layer::Surface) => "ansicht_oben",
            Action::View(Layer::Underground) => "ansicht_unten",
            Action::Research => "forschung",
            Action::Menu => "menue",
            Action::Overlay(MapOverlay::Mood) => "karte_stimmung",
            Action::Overlay(_) => "karte_weide",
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

    /// Die Forschung, ohne die dieser Knopf noch gesperrt ist.
    fn locked_by(self, progress: &Progress) -> Option<Tech> {
        let tech = match self {
            Action::Tool(kind) => kind.required_tech(),
            Action::Depth(depth) if !depth_available(depth, progress) => Some(Tech::DeepTunnels),
            _ => None,
        }?;
        (!progress.has(tech)).then_some(tech)
    }

    fn tooltip(self, alarm: &AlarmButton, progress: &Progress) -> String {
        let text = match self {
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
            Action::GatherFirst => "Vorräte anlegen  [B]\nAlle Erwachsenen sammeln Gras für den Vorrat, \
                 Graben und Wache haben Pause. Nochmal klicken zum Beenden."
                .into(),
            Action::Speed(None) => "Pause  [Leertaste]".into(),
            Action::Speed(Some(speed)) => format!("Tempo {speed}×  [{speed}]"),
            Action::View(layer) => format!("Ansicht: {}  [Tab]", layer.label()),
            Action::Menu => "Menü  [F10]
Pause, speichern, laden, neu beginnen oder beenden.".into(),
            Action::Overlay(overlay) => format!(
                "Übersichtskarte: {}  [Q]
{}",
                overlay.label(),
                match overlay {
                    MapOverlay::Mood => "Farbige Ringe zeigen, wie zufrieden jedes Kaninchen ist.",
                    _ => "Zeigt, wo die Wiese gesund ist und wo sie abgefressen und ausgelaugt ist.",
                }
            ),
            Action::Research => format!(
                "Forschungsbaum  [T]\nWeisheit aus der Erzählkammer in Neues stecken. Weisheit: {:.0}",
                progress.wisdom
            ),
        };
        match self.locked_by(progress) {
            Some(tech) => format!("{text}\nGesperrt: erst „{}“ erforschen.", tech.label()),
            None => text,
        }
    }
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

/// Eine zentrierte Zeile über die volle Breite, die selbst keine Mausklicks abfängt.
fn centered_row(commands: &mut Commands, bottom: f32) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(bottom),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id()
}

fn spawn_toolbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    let tools: Vec<Action> = TOOLS.iter().map(|(kind, _, _)| Action::Tool(*kind)).collect();
    let controls: [Vec<Action>; 6] = [
        [Depth::Shallow, Depth::Medium, Depth::Deep].map(Action::Depth).to_vec(),
        vec![Action::GatherFirst, Action::Alarm],
        vec![
            Action::Speed(None),
            Action::Speed(Some(1.0)),
            Action::Speed(Some(2.0)),
            Action::Speed(Some(3.0)),
        ],
        vec![Action::View(Layer::Surface), Action::View(Layer::Underground)],
        vec![Action::Overlay(MapOverlay::Pasture), Action::Overlay(MapOverlay::Mood)],
        vec![Action::Research, Action::Menu],
    ];

    let row = centered_row(&mut commands, 12.0);
    let bar = commands
        .spawn((
            Toolbar,
            Hovered::default(),
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            ChildOf(row),
        ))
        .id();
    let line = |commands: &mut Commands| {
        commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    ..default()
                },
                ChildOf(bar),
            ))
            .id()
    };

    let top = line(&mut commands);
    for action in tools {
        spawn_button(&mut commands, &asset_server, top, action);
    }
    let bottom = line(&mut commands);
    for (index, group) in controls.into_iter().enumerate() {
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
                ChildOf(bottom),
            ));
        }
        for action in group {
            spawn_button(&mut commands, &asset_server, bottom, action);
        }
    }

    // Tooltip über der Leiste.
    let tooltip_row = centered_row(&mut commands, TOOLBAR_HEIGHT + 20.0);
    let panel = commands
        .spawn((
            TooltipPanel,
            Node {
                display: Display::None,
                max_width: Val::Px(480.0),
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

fn spawn_button(commands: &mut Commands, asset_server: &AssetServer, parent: Entity, action: Action) {
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
            ChildOf(parent),
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
        None => commands.spawn((Text::new(action.caption()), text_style(17.0), Pickable::IGNORE, ChildOf(button))),
    };
    // Ein kleines Schloss unten rechts, sichtbar solange gesperrt.
    commands.spawn((
        LockIcon,
        ImageNode::new(asset_server.load("ui/schloss.png")),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(-2.0),
            bottom: Val::Px(-2.0),
            width: Val::Px(20.0),
            height: Val::Px(20.0),
            ..default()
        },
        Visibility::Hidden,
        Pickable::IGNORE,
        ChildOf(button),
    ));
}

/// Alles, was ein Knopf verändern kann.
#[derive(bevy::ecs::system::SystemParam)]
struct ActionTargets<'w> {
    tool: ResMut<'w, Tool>,
    view: ResMut<'w, ViewLayer>,
    time: ResMut<'w, Time<Virtual>>,
    alarm: ResMut<'w, AlarmButton>,
    priorities: ResMut<'w, Priorities>,
    research: ResMut<'w, ResearchOpen>,
    progress: Res<'w, Progress>,
    messages: ResMut<'w, Messages>,
    overlay: ResMut<'w, MapOverlay>,
    menu: ResMut<'w, OpenMenu>,
}

impl ActionTargets<'_> {
    fn perform(&mut self, action: Action) {
        if let Some(tech) = action.locked_by(&self.progress) {
            self.messages.push(locked_message(tech));
            return;
        }
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
            Action::Research => self.research.0 = !self.research.0,
            Action::Menu => self.menu.0 = true,
            Action::Overlay(which) => {
                self.overlay.toggle(which);
                // Die Weide sieht man nur von oben.
                if *self.overlay == MapOverlay::Pasture {
                    self.view.0 = Layer::Surface;
                }
            }
        }
    }
}

fn toolbar_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut priorities: ResMut<Priorities>,
    mut research: ResMut<ResearchOpen>,
) {
    if keys.just_pressed(KeyCode::KeyB) {
        priorities.gather_first = !priorities.gather_first;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        research.0 = !research.0;
    }
}

fn highlight_buttons(
    tool: Res<Tool>,
    view: Res<ViewLayer>,
    time: Res<Time<Virtual>>,
    alarm: Res<AlarmButton>,
    priorities: Res<Priorities>,
    research: Res<ResearchOpen>,
    progress: Res<Progress>,
    overlay: Res<MapOverlay>,
    mut buttons: Query<(&ActionButton, &Hovered, &Children, &mut BackgroundColor, &mut BorderColor)>,
    mut locks: Query<&mut Visibility, With<LockIcon>>,
) {
    for (button, hovered, children, mut background, mut border) in &mut buttons {
        let locked = button.0.locked_by(&progress).is_some();
        let (active, dimmed) = match button.0 {
            Action::Tool(kind) => (tool.kind == kind, locked),
            Action::Depth(depth) => (tool.depth == depth, locked || tool.kind != ToolKind::Dig),
            Action::Alarm => (false, alarm.cooldown > 0.0),
            Action::GatherFirst => (priorities.gather_first, false),
            Action::Speed(None) => (time.is_paused(), false),
            Action::Speed(Some(speed)) => (!time.is_paused() && time.relative_speed() == speed, false),
            Action::View(layer) => (view.0 == layer, false),
            Action::Research => (research.0, false),
            Action::Overlay(which) => (*overlay == which, false),
            Action::Menu => (false, false),
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
        let wanted = if locked { Visibility::Inherited } else { Visibility::Hidden };
        for child in children.iter() {
            if let Ok(mut visibility) = locks.get_mut(child)
                && *visibility != wanted
            {
                *visibility = wanted;
            }
        }
    }
}

fn update_tooltip(
    tooltip: Res<Tooltip>,
    alarm: Res<AlarmButton>,
    progress: Res<Progress>,
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
    // Jedes Bild neu, damit Sperrzeit und Weisheit mitlaufen.
    let content = action.tooltip(&alarm, &progress);
    if text.0 != content {
        text.0 = content;
    }
}

// --- Forschungsbaum ---

#[derive(Component)]
struct ResearchPanel;

#[derive(Component)]
struct ResearchHeader;

#[derive(Component)]
struct TechButton(Tech);

#[derive(Component)]
struct TechText(Tech);

fn spawn_research_panel(mut commands: Commands) {
    let row = centered_row(&mut commands, TOOLBAR_HEIGHT + 20.0);
    let panel = commands
        .spawn((
            ResearchPanel,
            Toolbar,
            Hovered::default(),
            Node {
                display: Display::None,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(14.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            ChildOf(row),
        ))
        .id();
    commands.spawn((ResearchHeader, Text::default(), text_style(16.0), Pickable::IGNORE, ChildOf(panel)));
    let columns = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(panel),
        ))
        .id();
    for branch in Branch::ALL {
        let column = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    width: Val::Px(210.0),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(columns),
            ))
            .id();
        commands.spawn((Text::new(branch.label()), text_style(16.0), Pickable::IGNORE, ChildOf(column)));
        for tech in Tech::ALL.into_iter().filter(|t| t.branch() == branch) {
            let button = commands
                .spawn((
                    TechButton(tech),
                    Hovered::default(),
                    Node {
                        padding: UiRect::all(Val::Px(8.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(BUTTON),
                    BorderColor::all(Color::NONE),
                    ChildOf(column),
                ))
                .observe(
                    move |click: On<PointerClick>, mut progress: ResMut<Progress>, mut messages: ResMut<Messages>| {
                        if click.button != PointerButton::Primary {
                            return;
                        }
                        if progress.research(tech) {
                            messages.push(format!("Erforscht: {}. {}", tech.label(), tech.description()));
                        }
                    },
                )
                .id();
            commands.spawn((TechText(tech), Text::default(), text_style(13.0), Pickable::IGNORE, ChildOf(button)));
        }
    }
}

fn show_research_panel(research: Res<ResearchOpen>, mut panel: Single<&mut Node, With<ResearchPanel>>) {
    if !research.is_changed() {
        return;
    }
    panel.display = if research.0 { Display::Flex } else { Display::None };
}

fn update_research_panel(
    research: Res<ResearchOpen>,
    progress: Res<Progress>,
    mut header: Single<&mut Text, (With<ResearchHeader>, Without<TechText>)>,
    mut texts: Query<(&TechText, &mut Text, &mut TextColor), Without<ResearchHeader>>,
    mut buttons: Query<(&TechButton, &Hovered, &mut BackgroundColor, &mut BorderColor)>,
) {
    if !research.0 {
        return;
    }
    let next = progress
        .stage
        .next_goal()
        .map_or(String::new(), |goal| format!("    Nächste Stufe {goal}"));
    let content = format!(
        "Forschungsbaum  [T]    Weisheit: {:.0}    Stufe: {}{next}\n\
         Weisheit sammeln Kaninchen in der Erzählkammer, alte Kaninchen dreimal so viel.",
        progress.wisdom,
        progress.stage.label()
    );
    if header.0 != content {
        header.0 = content;
    }

    for (tech_text, mut text, mut color) in &mut texts {
        let tech = tech_text.0;
        let state = match progress.check(tech) {
            Ok(()) => format!("Erforschen: {:.0} Weisheit", tech.cost()),
            Err(Blocked::Done) => "Erforscht".into(),
            Err(Blocked::NeedsPrerequisite(before)) => format!("Erst „{}“", before.label()),
            Err(Blocked::NeedsStage(stage)) => format!("Ab Stufe {}", stage.label()),
            Err(Blocked::NeedsWisdom(missing)) => {
                format!("{:.0} Weisheit, fehlen noch {missing:.0}", tech.cost())
            }
        };
        let content = format!("{}\n{}\n{state}", tech.label(), tech.description());
        if text.0 != content {
            text.0 = content;
        }
        let quiet = !progress.has(tech) && progress.check(tech).is_err();
        let wanted = if quiet { TEXT_QUIET } else { TEXT };
        if color.0 != wanted {
            color.0 = wanted;
        }
    }
    for (button, hovered, mut background, mut border) in &mut buttons {
        let done = progress.has(button.0);
        let ready = progress.check(button.0).is_ok();
        let fill = if done {
            Color::srgb(0.30, 0.42, 0.24)
        } else if hovered.0 && ready {
            BUTTON_HOVER
        } else if ready {
            BUTTON
        } else {
            BUTTON_DIMMED
        };
        if background.0 != fill {
            background.0 = fill;
        }
        let edge = if ready { BORDER_ACTIVE } else { Color::NONE };
        if border.top != edge {
            *border = BorderColor::all(edge);
        }
    }
}
