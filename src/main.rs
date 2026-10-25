mod behavior;
mod camera;
mod colony;
mod danger;
mod daynight;
#[cfg(debug_assertions)]
mod devtools;
mod effects;
mod furnishing;
mod ground;
mod map;
mod pathfinding;
mod predator;
mod rabbit;
mod rig;
mod speed;
mod toolbar;
mod tools;
mod ui;

use bevy::prelude::*;

fn main() {
    // Automatische Testläufe sollen niemandem den Fokus wegnehmen.
    let test_run = cfg!(debug_assertions) && std::env::var_os("BUNNY_TEST").is_some();

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Bunny Skylines – Bau ’nen Bau!".into(),
            resolution: (1280, 720).into(),
            focused: !test_run,
            ..default()
        }),
        ..default()
    }))
        .insert_resource(ClearColor(Color::srgb(0.98, 0.95, 0.86)))
        // Welt und Spiellogik
        .add_plugins((
            camera::CameraPlugin,
            speed::SpeedPlugin,
            map::MapPlugin,
            ground::GroundPlugin,
            tools::ToolsPlugin,
            colony::ColonyPlugin,
            rig::RigPlugin,
            rabbit::RabbitPlugin,
            behavior::BehaviorPlugin,
            daynight::DayNightPlugin,
            predator::PredatorPlugin,
            danger::DangerPlugin,
        ))
        // Darstellung und Bedienung
        .add_plugins((
            furnishing::FurnishingPlugin,
            effects::EffectsPlugin,
            ui::UiPlugin,
            toolbar::ToolbarPlugin,
        ));
    #[cfg(debug_assertions)]
    app.add_plugins(devtools::DevToolsPlugin);
    app.run();
}
