mod behavior;
mod camera;
mod colony;
mod danger;
mod daynight;
#[cfg(debug_assertions)]
mod devtools;
mod effects;
mod ending;
mod furnishing;
mod ground;
mod inspect;
mod map;
mod pathfinding;
mod predator;
mod progress;
mod rabbit;
mod rig;
mod save;
mod scenario;
mod speed;
mod toolbar;
mod tools;
mod ui;

use bevy::prelude::*;

fn main() {
    // Automatische Testläufe sollen niemandem den Fokus wegnehmen.
    let test_script = std::env::var("BUNNY_TEST").ok().filter(|_| cfg!(debug_assertions));
    let test_run = test_script.is_some();
    // Lange Zeitraffer-Läufe brauchen nur das Protokoll, kein Fenster auf dem Bildschirm.
    let hidden = test_script.is_some_and(|s| s.split(',').any(|c| c.trim() == "unsichtbar"));

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Bunny Skylines – Bau ’nen Bau!".into(),
            resolution: (1280, 720).into(),
            focused: !test_run,
            visible: !hidden,
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
            ending::EndingPlugin,
            progress::ProgressPlugin,
            inspect::InspectPlugin,
            scenario::ScenarioPlugin,
            save::SavePlugin,
        ));
    #[cfg(debug_assertions)]
    app.add_plugins(devtools::DevToolsPlugin);
    app.run();
}
