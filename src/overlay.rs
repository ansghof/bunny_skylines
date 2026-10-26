//! Übersichtskarten wie in Cities: Skylines: Eine Farbschicht zeigt auf einen Blick,
//! wo die Weide leidet und welche Kaninchen unglücklich sind. Q schaltet durch.

use bevy::prelude::*;

use crate::ground::{floor_y, gizmo_circle};
use crate::map::{Grid, Layer, SurfaceTile, TilePos, ViewLayer};
use crate::rabbit::{Mood, OnLayer, Rabbit};

pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapOverlay>()
            .add_systems(Startup, spawn_legend)
            .add_systems(Update, (overlay_key, show_legend, mood_rings));
    }
}

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MapOverlay {
    #[default]
    Off,
    /// Wie gesund der Boden ist und wie viel Gras darauf steht.
    Pasture,
    /// Ringe unter den Kaninchen in der Farbe ihrer Stimmung.
    Mood,
}

impl MapOverlay {
    pub fn label(self) -> &'static str {
        match self {
            MapOverlay::Off => "aus",
            MapOverlay::Pasture => "Weidedruck",
            MapOverlay::Mood => "Stimmung",
        }
    }

    fn legend(self) -> &'static str {
        match self {
            MapOverlay::Off => "",
            MapOverlay::Pasture => {
                "Übersicht: Weidedruck  [Q]\n\
                 Grün: gesunder Boden · Gelb: abgegrast · Rot: ausgelaugt\n\
                 Je heller, desto mehr Gras steht noch.\n\
                 Rote Flecken ruhen lassen oder Klee säen."
            }
            MapOverlay::Mood => {
                "Übersicht: Stimmung  [Q]\n\
                 Ringe unter den Kaninchen:\n\
                 Grün zufrieden · Gelb geht so · Orange unzufrieden · Rot will fort\n\
                 Ein Klick auf ein Kaninchen zeigt den Grund."
            }
        }
    }

    /// Knopf drücken: dieselbe Karte schaltet aus, eine andere an.
    pub fn toggle(&mut self, which: MapOverlay) {
        *self = if *self == which { MapOverlay::Off } else { which };
    }
}

/// Farbe eines Wiesenfeldes in der Weidedruck-Karte.
pub fn pasture_color(tile: &SurfaceTile) -> Color {
    if tile.entrance {
        return Color::srgb(0.29, 0.20, 0.13);
    }
    // Wald ist keine Weide.
    if tile.forest {
        return Color::srgb(0.24, 0.30, 0.22);
    }
    // Fruchtbarkeit von rot über gelb nach grün, Gras macht heller.
    let health = tile.fertility.clamp(0.0, 1.0);
    let rgb = if health < 0.5 {
        mix([0.85, 0.22, 0.18], [0.95, 0.80, 0.25], health / 0.5)
    } else {
        mix([0.95, 0.80, 0.25], [0.30, 0.72, 0.30], (health - 0.5) / 0.5)
    };
    let light = 0.55 + 0.45 * tile.grass.clamp(0.0, 1.0);
    let rgb = rgb.map(|c| c * light);
    Color::srgb(rgb[0], rgb[1], rgb[2])
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn overlay_key(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<MapOverlay>, mut view: ResMut<ViewLayer>) {
    if !keys.just_pressed(KeyCode::KeyQ) {
        return;
    }
    *overlay = match *overlay {
        MapOverlay::Off => MapOverlay::Pasture,
        MapOverlay::Pasture => MapOverlay::Mood,
        MapOverlay::Mood => MapOverlay::Off,
    };
    if *overlay == MapOverlay::Pasture {
        view.0 = Layer::Surface;
    }
}

#[derive(Component)]
struct Legend;

fn spawn_legend(mut commands: Commands) {
    commands.spawn((
        Legend,
        Text::default(),
        Node {
            display: Display::None,
            position_type: PositionType::Absolute,
            right: Val::Px(12.0),
            top: Val::Px(140.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.22, 0.15, 0.10, 0.85)),
        BorderColor::all(Color::srgb(0.95, 0.80, 0.45)),
        TextColor(Color::srgb(0.98, 0.95, 0.88)),
        TextFont {
            font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
            ..TextFont::from_font_size(15.0)
        },
    ));
}

fn show_legend(overlay: Res<MapOverlay>, mut legend: Single<(&mut Text, &mut Node), With<Legend>>) {
    if !overlay.is_changed() {
        return;
    }
    let (text, node) = &mut *legend;
    text.0 = overlay.legend().to_string();
    node.display = if *overlay == MapOverlay::Off { Display::None } else { Display::Flex };
}

fn mood_color(value: f32) -> Color {
    match value {
        v if v >= 0.75 => Color::srgb(0.35, 0.85, 0.35),
        v if v >= 0.5 => Color::srgb(0.98, 0.85, 0.30),
        v if v >= 0.35 => Color::srgb(0.98, 0.55, 0.20),
        _ => Color::srgb(0.92, 0.20, 0.18),
    }
}

fn mood_rings(
    mut gizmos: Gizmos,
    overlay: Res<MapOverlay>,
    grid: Res<Grid>,
    view: Res<ViewLayer>,
    rabbits: Query<(&Transform, &OnLayer, &Mood), With<Rabbit>>,
) {
    if *overlay != MapOverlay::Mood {
        return;
    }
    for (transform, layer, mood) in &rabbits {
        if layer.0 != view.0 {
            continue;
        }
        let position = transform.translation.truncate();
        let floor = floor_y(&grid, layer.0, TilePos::from_world_clamped(position));
        let color = mood_color(mood.value);
        gizmo_circle(&mut gizmos, position, 16.0, floor, color);
        gizmo_circle(&mut gizmos, position, 13.0, floor, color);
    }
}
