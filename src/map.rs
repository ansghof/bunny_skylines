//! Spielkarte. Vorerst nur ein Platzhalter-Wiesengitter, damit Kamera und Zoom sichtbar sind.

use bevy::prelude::*;

pub const TILE_SIZE: f32 = 64.0;
pub const MAP_WIDTH: u32 = 40;
pub const MAP_HEIGHT: u32 = 30;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_placeholder_meadow);
    }
}

/// Mittelpunkt eines Feldes in Weltkoordinaten; die Karte ist um den Ursprung zentriert.
pub fn tile_center(x: u32, y: u32) -> Vec2 {
    let origin = -Vec2::new(MAP_WIDTH as f32, MAP_HEIGHT as f32) * TILE_SIZE / 2.0;
    origin + (Vec2::new(x as f32, y as f32) + 0.5) * TILE_SIZE
}

fn spawn_placeholder_meadow(mut commands: Commands) {
    let light = Color::srgb(0.61, 0.80, 0.48);
    let dark = Color::srgb(0.56, 0.76, 0.43);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let color = if (x + y) % 2 == 0 { light } else { dark };
            commands.spawn((
                Sprite::from_color(color, Vec2::splat(TILE_SIZE)),
                Transform::from_translation(tile_center(x, y).extend(0.0)),
            ));
        }
    }
}
