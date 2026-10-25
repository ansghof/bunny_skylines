//! Bauwerkzeuge: Mit gedrückter linker Maustaste einen Bereich aufziehen.
//! Die Kaninchen arbeiten die Aufträge danach selbst ab.

use bevy::prelude::*;

use crate::camera::CursorWorld;
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
    Remove,
}

impl ToolKind {
    pub fn label(self) -> &'static str {
        match self {
            ToolKind::Dig => "Graben",
            ToolKind::Entrance => "Eingang",
            ToolKind::Chamber(chamber) => chamber.label(),
            ToolKind::Post => "Wachposten",
            ToolKind::Remove => "Entfernen",
        }
    }

    fn is_single(self) -> bool {
        matches!(self, ToolKind::Entrance | ToolKind::Post)
    }

    /// Die Ansicht, in der das Werkzeug benutzt wird; `None` = beide.
    fn layer(self) -> Option<Layer> {
        match self {
            ToolKind::Dig | ToolKind::Chamber(_) => Some(Layer::Underground),
            ToolKind::Entrance | ToolKind::Post => Some(Layer::Surface),
            ToolKind::Remove => None,
        }
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
    let selected = [
        (KeyCode::KeyG, ToolKind::Dig),
        (KeyCode::KeyE, ToolKind::Entrance),
        (KeyCode::KeyN, ToolKind::Chamber(Chamber::Sleeping)),
        (KeyCode::KeyV, ToolKind::Chamber(Chamber::Storage)),
        (KeyCode::KeyP, ToolKind::Post),
        (KeyCode::KeyX, ToolKind::Remove),
    ]
    .into_iter()
    .find(|(key, _)| keys.just_pressed(*key));

    if let Some((_, kind)) = selected {
        tool.kind = kind;
        // Gleich in die passende Ansicht wechseln, damit man sieht, was man baut.
        if let Some(layer) = kind.layer() {
            view.0 = layer;
        }
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
) {
    if buttons.just_pressed(MouseButton::Left) {
        drag.start = cursor.0.and_then(TilePos::from_world);
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
        if !tile.entrance {
            match tool.kind {
                ToolKind::Entrance => tile.entrance_planned = true,
                _ => tile.post = true,
            }
        }
        return;
    }
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
                if tile.dug.is_some() || tile.planned.is_some() {
                    tile.chamber = Some(chamber);
                }
            }
            ToolKind::Remove => {
                let tile = grid.under_mut(p);
                tile.planned = None;
                tile.chamber = None;
                let surface = grid.surface_mut(p);
                surface.entrance_planned = false;
                surface.post = false;
            }
            ToolKind::Entrance | ToolKind::Post => unreachable!(),
        }
    }
}

fn draw_preview(
    mut gizmos: Gizmos,
    buttons: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorWorld>,
    tool: Res<Tool>,
    view: Res<ViewLayer>,
    drag: Res<Drag>,
) {
    let Some(cursor) = cursor.0 else {
        return;
    };
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
    gizmos.rect_2d(Isometry2d::from_translation((min + max) / 2.0), max - min, color);
}
