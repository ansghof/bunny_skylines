mod camera;
mod map;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bunny Skylines – Bau ’nen Bau!".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.98, 0.95, 0.86)))
        .add_plugins((camera::CameraPlugin, map::MapPlugin))
        .run();
}
