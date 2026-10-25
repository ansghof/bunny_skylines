//! Nur im Debug-Build: automatische Testläufe ohne Tastatur und Maus.
//!
//! `BUNNY_TEST` enthält durch Komma getrennte Anweisungen, z. B.
//! `BUNNY_TEST=unten,vorrat,nacht,tempo=3,bild=12 cargo run`
//!
//! - `unten`: mit der Untergrund-Ansicht starten
//! - `vorrat`: den Gang am Startbau zur Vorratskammer machen und 15 Futter einlagern
//! - `nacht`: kurz vor Einbruch der Nacht starten
//! - `tempo=N`: Spielgeschwindigkeit
//! - `bild=S`: nach S Sekunden Echtzeit einen Screenshot nach `target/test-bild.png` speichern und beenden

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::colony::Colony;
use crate::daynight::DayClock;
use crate::map::{Chamber, Grid, Layer, TilePos, ViewLayer};

pub const ENV: &str = "BUNNY_TEST";
const SHOT_PATH: &str = "target/test-bild.png";

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        let Ok(script) = std::env::var(ENV) else {
            return;
        };
        let commands: Vec<String> = script.split(',').map(|s| s.trim().to_string()).collect();
        app.insert_resource(TestScript(commands))
            .add_systems(Startup, prepare)
            .add_systems(Update, take_shot);
    }
}

#[derive(Resource)]
struct TestScript(Vec<String>);

impl TestScript {
    fn has(&self, word: &str) -> bool {
        self.0.iter().any(|c| c == word)
    }

    fn value(&self, key: &str) -> Option<f32> {
        self.0
            .iter()
            .find_map(|c| c.strip_prefix(key)?.strip_prefix('=')?.parse().ok())
    }
}

fn prepare(
    script: Res<TestScript>,
    mut view: ResMut<ViewLayer>,
    mut grid: ResMut<Grid>,
    mut colony: ResMut<Colony>,
    mut clock: ResMut<DayClock>,
    mut time: ResMut<Time<Virtual>>,
) {
    if script.has("unten") {
        view.0 = Layer::Underground;
    }
    if script.has("vorrat") {
        for x in [21, 22] {
            grid.under_mut(TilePos::new(x, 15)).chamber = Some(Chamber::Storage);
        }
        colony.food = 15.0;
    }
    if script.has("nacht") {
        clock.time = 0.74;
    }
    if let Some(speed) = script.value("tempo") {
        time.set_relative_speed(speed);
    }
}

fn take_shot(
    mut commands: Commands,
    script: Res<TestScript>,
    time: Res<Time<Real>>,
    mut taken: Local<bool>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(at) = script.value("bild") else {
        return;
    };
    let elapsed = time.elapsed_secs();
    if !*taken && elapsed >= at {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(SHOT_PATH));
        *taken = true;
    }
    // Eine Sekunde Zeit zum Speichern lassen.
    if *taken && elapsed >= at + 1.0 {
        exit.write(AppExit::Success);
    }
}
