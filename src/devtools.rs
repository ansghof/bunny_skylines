//! Nur im Debug-Build: automatische Testläufe ohne Tastatur und Maus.
//!
//! `BUNNY_TEST` enthält durch Komma getrennte Anweisungen, z. B.
//! `BUNNY_TEST=unten,vorrat,nacht,tempo=3,bild=12 cargo run`
//!
//! - `unten` / `oben`: mit der Untergrund- bzw. Oberflächen-Ansicht starten
//! - `abstand=N`, `drehung=Grad`, `neigung=Grad`: Kamera einstellen
//! - `vorrat`: den Gang am Startbau zur Vorratskammer machen und 15 Futter einlagern
//! - `graben`: Grabaufträge neben dem Startgang setzen
//! - `zeiger`: Mauszeiger fest über ein ungegrabenes Feld neben dem Startbau setzen
//! - `nacht`: kurz vor Einbruch der Nacht starten
//! - `tempo=N`: Spielgeschwindigkeit
//! - `bild=S`: nach S Sekunden Echtzeit einen Screenshot nach `target/test-bild.png` speichern und beenden
//! - `serie=N`: statt eines Bildes N Bilder im Abstand von 0,12 s (`target/test-bild-0.png` …)

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::camera::{CursorWorld, Orbit, update_cursor};
use crate::colony::Colony;
use crate::daynight::DayClock;
use crate::map::{Chamber, Depth, Grid, Layer, TilePos, ViewLayer};

pub const ENV: &str = "BUNNY_TEST";
const SHOT_PATH: &str = "target/test-bild.png";
const SERIES_INTERVAL: f32 = 0.12;

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        let Ok(script) = std::env::var(ENV) else {
            return;
        };
        let commands: Vec<String> = script.split(',').map(|s| s.trim().to_string()).collect();
        app.insert_resource(TestScript(commands))
            .add_systems(Startup, prepare)
            .add_systems(PreUpdate, fake_cursor.after(update_cursor))
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
    mut orbit: ResMut<Orbit>,
) {
    if script.has("unten") {
        view.0 = Layer::Underground;
    }
    if script.has("oben") {
        view.0 = Layer::Surface;
    }
    if let Some(distance) = script.value("abstand") {
        orbit.distance = distance;
    }
    if let Some(degrees) = script.value("drehung") {
        orbit.yaw = degrees.to_radians();
    }
    if let Some(degrees) = script.value("neigung") {
        orbit.pitch = degrees.to_radians();
    }
    if script.has("vorrat") {
        for x in [21, 22] {
            grid.under_mut(TilePos::new(x, 15)).chamber = Some(Chamber::Storage);
        }
        colony.food = 15.0;
    }
    if script.has("graben") {
        for (x, y) in [(21, 16), (21, 17), (22, 17), (20, 17)] {
            grid.under_mut(TilePos::new(x, y)).planned = Some(Depth::Medium);
        }
    }
    if script.has("nacht") {
        clock.time = 0.74;
    }
    if let Some(speed) = script.value("tempo") {
        time.set_relative_speed(speed);
    }
}

fn fake_cursor(script: Res<TestScript>, mut cursor: ResMut<CursorWorld>) {
    if script.has("zeiger") {
        cursor.0 = Some(TilePos::new(21, 13).center());
    }
}

fn take_shot(
    mut commands: Commands,
    script: Res<TestScript>,
    time: Res<Time<Real>>,
    mut taken: Local<usize>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(at) = script.value("bild") else {
        return;
    };
    let series = script.value("serie").map(|n| n as usize);
    let count = series.unwrap_or(1);
    let elapsed = time.elapsed_secs();
    if *taken < count && elapsed >= at + *taken as f32 * SERIES_INTERVAL {
        let path = match series {
            Some(_) => format!("target/test-bild-{}.png", *taken),
            None => SHOT_PATH.to_string(),
        };
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *taken += 1;
    }
    // Eine Sekunde Zeit zum Speichern lassen.
    if *taken == count && elapsed >= at + count as f32 * SERIES_INTERVAL + 1.0 {
        exit.write(AppExit::Success);
    }
}
