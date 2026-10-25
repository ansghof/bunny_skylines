//! Bauwerkzeuge: Mit gedrückter linker Maustaste einen Bereich aufziehen.
//! Die Kaninchen arbeiten die Aufträge danach selbst ab.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;

use crate::camera::CursorWorld;
use crate::toolbar::Toolbar;
use crate::colony::{Colony, Messages, cost};
use crate::ground::gizmo_rect;
use crate::map::{Chamber, Depth, Grid, Layer, TILE_SIZE, TilePos, ViewLayer, tiles_between};

pub struct ToolsPlugin;

impl Plugin for ToolsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Tool { kind: ToolKind::Dig, depth: Depth::Medium })
            .init_resource::<Drag>()
            .add_systems(Update, (select_tool, apply_drag, draw_preview).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolKind {
    Dig,
    Entrance,
    Chamber(Chamber),
    Post,
    Clover,
    Remove,
}

/// Alle Werkzeuge in der Reihenfolge der Werkzeugleiste, mit Taste und Tastenname.
pub const TOOLS: [(ToolKind, KeyCode, &str); 7] = [
    (ToolKind::Dig, KeyCode::KeyG, "G"),
    (ToolKind::Entrance, KeyCode::KeyE, "E"),
    (ToolKind::Chamber(Chamber::Sleeping), KeyCode::KeyN, "N"),
    (ToolKind::Chamber(Chamber::Storage), KeyCode::KeyV, "V"),
    (ToolKind::Post, KeyCode::KeyP, "P"),
    (ToolKind::Clover, KeyCode::KeyK, "K"),
    (ToolKind::Remove, KeyCode::KeyX, "X"),
];

impl ToolKind {
    pub fn label(self) -> &'static str {
        match self {
            ToolKind::Dig => "Graben",
            ToolKind::Entrance => "Eingang",
            ToolKind::Chamber(chamber) => chamber.label(),
            ToolKind::Post => "Wachposten",
            ToolKind::Clover => "Kleefeld säen",
            ToolKind::Remove => "Entfernen",
        }
    }

    /// Ein Satz dazu, was das Werkzeug tut, für den Tooltip.
    pub fn description(self) -> &'static str {
        match self {
            ToolKind::Dig => "Gänge und Kammern ausheben lassen. Bringt Erde.",
            ToolKind::Entrance => "Einen Eingang von der Wiese in den Bau graben lassen.",
            ToolKind::Chamber(Chamber::Sleeping) => "Gegrabene Felder zur Schlafkammer machen. Mehr Nester, mehr Nachwuchs.",
            ToolKind::Chamber(Chamber::Storage) => "Gegrabene Felder zur Vorratskammer machen. Hier lagert das Futter.",
            ToolKind::Post => "Ein Erdhügel, von dem aus eine Wache Füchse entdeckt.",
            ToolKind::Clover => "Klee wächst doppelt so schnell nach und ist nahrhafter als Gras.",
            ToolKind::Remove => "Aufträge, Kammern, Posten und Klee entfernen.",
        }
    }

    /// Was das Werkzeug kostet, für die Anzeige.
    pub fn cost_label(self) -> Option<String> {
        match self {
            ToolKind::Post => Some(format!("{:.0} Erde", cost::POST_EARTH)),
            ToolKind::Chamber(Chamber::Sleeping) => Some(format!("{:.0} Futter pro Nest", cost::NEST_FOOD)),
            ToolKind::Clover => Some(format!("{:.0} Futter pro Feld", cost::CLOVER_FOOD)),
            _ => None,
        }
    }

    fn is_single(self) -> bool {
        matches!(self, ToolKind::Entrance | ToolKind::Post)
    }

    /// Die Ansicht, in der das Werkzeug benutzt wird; `None` = beide.
    pub fn layer(self) -> Option<Layer> {
        match self {
            ToolKind::Dig | ToolKind::Chamber(_) => Some(Layer::Underground),
            ToolKind::Entrance | ToolKind::Post | ToolKind::Clover => Some(Layer::Surface),
            ToolKind::Remove => None,
        }
    }
}

/// Wählt ein Werkzeug und wechselt in die passende Ansicht, damit man sieht, was man baut.
pub fn select(kind: ToolKind, tool: &mut Tool, view: &mut ViewLayer) {
    tool.kind = kind;
    if let Some(layer) = kind.layer() {
        view.0 = layer;
    }
}

#[derive(Resource)]
pub struct Tool {
    pub kind: ToolKind,
    pub depth: Depth,
}

#[derive(Resource, Default)]
struct Drag {
    start: Option<TilePos>,
}

fn select_tool(keys: Res<ButtonInput<KeyCode>>, mut tool: ResMut<Tool>, mut view: ResMut<ViewLayer>) {
    if let Some((kind, _, _)) = TOOLS.into_iter().find(|(_, key, _)| keys.just_pressed(*key)) {
        select(kind, &mut tool, &mut view);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        tool.depth = tool.depth.next();
    }
}

fn apply_drag(
    buttons: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorWorld>,
    tool: Res<Tool>,
    view: Res<ViewLayer>,
    mut drag: ResMut<Drag>,
    mut grid: ResMut<Grid>,
    mut colony: ResMut<Colony>,
    mut messages: ResMut<Messages>,
    toolbar: Query<&Hovered, With<Toolbar>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        // Ein Klick auf die Werkzeugleiste ist keiner in die Welt.
        let on_toolbar = toolbar.iter().any(|hovered| hovered.0);
        drag.start = if on_toolbar {
            None
        } else {
            cursor.0.and_then(TilePos::from_world)
        };
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    let (Some(start), Some(cursor)) = (drag.start.take(), cursor.0) else {
        return;
    };
    if tool.kind.layer().is_some_and(|layer| layer != view.0) {
        return;
    }

    let end = TilePos::from_world_clamped(cursor);
    // Eingänge und Posten immer einzeln, ein ganzer Bereich davon ergibt keinen Sinn.
    if tool.kind.is_single() {
        let tile = grid.surface_mut(end);
        if tile.entrance {
            return;
        }
        match tool.kind {
            ToolKind::Entrance => tile.entrance_planned = true,
            _ if tile.post => {}
            _ if colony.pay(0.0, cost::POST_EARTH) => tile.post = true,
            _ => messages.push(format!(
                "Für einen Wachposten fehlt Erde ({:.0} von {:.0}). Erde gibt es beim Graben.",
                colony.earth,
                cost::POST_EARTH
            )),
        }
        return;
    }

    let mut missing_food = false;
    let mut missing_seed = false;
    for p in tiles_between(start, end) {
        match tool.kind {
            ToolKind::Dig => {
                let tile = grid.under_mut(p);
                if tile.dug.is_none() {
                    tile.planned = Some(tool.depth);
                }
            }
            ToolKind::Chamber(chamber) => {
                let tile = grid.under_mut(p);
                if (tile.dug.is_none() && tile.planned.is_none()) || tile.chamber == Some(chamber) {
                    continue;
                }
                // Wird ein Nest zu etwas anderem, kommt das Gras zurück in den Vorrat.
                if tile.chamber == Some(Chamber::Sleeping) {
                    colony.food += cost::NEST_FOOD;
                }
                if chamber == Chamber::Sleeping && !colony.pay(cost::NEST_FOOD, 0.0) {
                    missing_food = true;
                    continue;
                }
                tile.chamber = Some(chamber);
            }
            ToolKind::Clover => {
                let field = grid.surface_mut(p);
                if field.entrance || field.post || field.clover {
                    continue;
                }
                if !colony.pay(cost::CLOVER_FOOD, 0.0) {
                    missing_seed = true;
                    continue;
                }
                // Frisch gesät: erst einmal kurz.
                field.clover = true;
                field.grass = field.grass.min(0.15);
            }
            ToolKind::Remove => {
                let tile = grid.under_mut(p);
                tile.planned = None;
                if tile.chamber.take() == Some(Chamber::Sleeping) {
                    colony.food += cost::NEST_FOOD;
                }
                let surface = grid.surface_mut(p);
                surface.entrance_planned = false;
                surface.clover = false;
                if std::mem::take(&mut surface.post) {
                    colony.earth += cost::POST_EARTH;
                }
            }
            ToolKind::Entrance | ToolKind::Post => unreachable!(),
        }
    }
    if missing_seed {
        messages.push(format!(
            "Für mehr Klee fehlt Saatgut ({:.0} Futter pro Feld).",
            cost::CLOVER_FOOD
        ));
    }
    if missing_food {
        messages.push(format!(
            "Für mehr Nester fehlt Futter ({:.0} Futter pro Nest). Sammler tragen Gras in den Vorrat.",
            cost::NEST_FOOD
        ));
    }
}

fn draw_preview(
    mut gizmos: Gizmos,
    buttons: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorWorld>,
    tool: Res<Tool>,
    view: Res<ViewLayer>,
    drag: Res<Drag>,
    toolbar: Query<&Hovered, With<Toolbar>>,
) {
    let Some(cursor) = cursor.0 else {
        return;
    };
    // Über der Werkzeugleiste keine Vorschau in der Welt darunter.
    if toolbar.iter().any(|hovered| hovered.0) {
        return;
    }
    let wrong_view = tool.kind.layer().is_some_and(|layer| layer != view.0);
    let color = if wrong_view {
        Color::srgba(0.9, 0.3, 0.2, 0.8)
    } else {
        Color::srgba(1.0, 0.95, 0.7, 0.9)
    };

    let end = TilePos::from_world_clamped(cursor);
    let start = match drag.start {
        Some(start) if buttons.pressed(MouseButton::Left) && !tool.kind.is_single() => start,
        _ => end,
    };
    let min = start.center().min(end.center()) - TILE_SIZE / 2.0;
    let max = start.center().max(end.center()) + TILE_SIZE / 2.0;
    // Auf der Oberkante, sonst steckt der Rahmen dort, wo noch nicht gegraben ist, in der Erde.
    gizmo_rect(&mut gizmos, min, max, 0.0, color);
}
