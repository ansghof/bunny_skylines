//! Spielstände: F5 speichert, F9 lädt; zu jeder neuen Jahreszeit wird automatisch gespeichert.
//! Es gibt einen Speicherplatz, eine RON-Datei im Benutzerordner.

use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::behavior::{Activity, Carrying, Priorities};
use crate::colony::{Colony, Messages, Rng};
use crate::daynight::{DayClock, Season};
use crate::ending::clear_world;
use crate::map::{Grid, Layer, ViewLayer};
use crate::progress::Progress;
use crate::rabbit::{BirthCooldown, Fur, Hunger, Mood, OnLayer, Pregnant, Rabbit, SavedRabbit};
use crate::scenario::{Scenario, Stage};

/// Ändert sich der Aufbau des Spielstands, wird die Zahl erhöht; alte Stände lassen sich dann nicht laden.
const VERSION: u32 = 1;

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (save_keys, autosave));
    }
}

/// Zum Schreiben: alles nur geliehen.
#[derive(Serialize)]
struct SaveRef<'a> {
    version: u32,
    grid: &'a Grid,
    colony: &'a Colony,
    clock: &'a DayClock,
    progress: &'a Progress,
    priorities: &'a Priorities,
    stage: Stage,
    view: Layer,
    rabbits: Vec<SavedRabbit>,
}

/// Zum Lesen: dieselben Felder, diesmal mit Besitz.
#[derive(Deserialize)]
struct SaveGame {
    version: u32,
    grid: Grid,
    colony: Colony,
    clock: DayClock,
    progress: Progress,
    priorities: Priorities,
    stage: Stage,
    view: Layer,
    rabbits: Vec<SavedRabbit>,
}

/// Testläufe schreiben in den Build-Ordner, damit sie keinen echten Spielstand überschreiben.
fn save_path() -> PathBuf {
    if cfg!(debug_assertions) && std::env::var_os("BUNNY_TEST").is_some() {
        return PathBuf::from("target/test-spielstand.ron");
    }
    match std::env::var_os("APPDATA") {
        Some(dir) => PathBuf::from(dir).join("BunnySkylines").join("spielstand.ron"),
        None => PathBuf::from("spielstand.ron"),
    }
}

pub fn save_exists() -> bool {
    save_path().exists()
}

fn save_keys(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    if keys.just_pressed(KeyCode::F5) {
        commands.run_system_cached(save_game);
    }
    if keys.just_pressed(KeyCode::F9) {
        commands.run_system_cached(load_game);
    }
}

/// Zu Beginn jeder Jahreszeit, solange die Familie lebt und gespielt wird.
fn autosave(
    clock: Res<DayClock>,
    scenario: Res<Scenario>,
    rabbits: Query<(), With<Rabbit>>,
    mut commands: Commands,
    mut last: Local<Option<Season>>,
) {
    let season = clock.season();
    let changed = last.is_some_and(|last| last != season);
    *last = Some(season);
    if changed && matches!(scenario.stage, Stage::Playing | Stage::Free) && !rabbits.is_empty() {
        commands.run_system_cached(save_game);
    }
}

pub fn save_game(world: &mut World) {
    let mut query = world.query::<(
        &Rabbit,
        &Fur,
        &Hunger,
        &Mood,
        &OnLayer,
        &Transform,
        &Carrying,
        Option<&Pregnant>,
        Option<&BirthCooldown>,
        &Activity,
    )>();
    let rabbits = query
        .iter(world)
        .filter(|(.., activity)| !matches!(activity, Activity::Gone))
        .map(|(rabbit, fur, hunger, mood, layer, transform, carrying, pregnant, cooldown, _)| {
            SavedRabbit::capture(rabbit, fur, hunger, mood, layer, transform, carrying, pregnant, cooldown)
        })
        .collect();
    // Einleitung und Siegbildschirm sind beim Laden schon vorbei.
    let stage = match world.resource::<Scenario>().stage {
        Stage::Intro | Stage::Playing => Stage::Playing,
        Stage::Won | Stage::Free => Stage::Free,
    };
    let save = SaveRef {
        version: VERSION,
        grid: world.resource::<Grid>(),
        colony: world.resource::<Colony>(),
        clock: world.resource::<DayClock>(),
        progress: world.resource::<Progress>(),
        priorities: world.resource::<Priorities>(),
        stage,
        view: world.resource::<ViewLayer>().0,
        rabbits,
    };
    let path = save_path();
    let written = ron::to_string(&save)
        .map_err(|e| e.to_string())
        .and_then(|text| {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, text).map_err(|e| e.to_string())
        });
    let message = match written {
        Ok(()) => "Spielstand gespeichert.".to_string(),
        Err(error) => {
            warn!("Speichern fehlgeschlagen: {error}");
            "Speichern hat nicht geklappt.".to_string()
        }
    };
    world.resource_mut::<Messages>().push(message);
}

pub fn load_game(world: &mut World) {
    let loaded = std::fs::read_to_string(save_path())
        .map_err(|e| e.to_string())
        .and_then(|text| ron::from_str::<SaveGame>(&text).map_err(|e| e.to_string()));
    let save = match loaded {
        Ok(save) if save.version == VERSION => save,
        Ok(_) => {
            world.resource_mut::<Messages>().push("Der Spielstand stammt von einer älteren Version.");
            return;
        }
        Err(error) => {
            warn!("Laden fehlgeschlagen: {error}");
            world.resource_mut::<Messages>().push("Es gibt noch keinen Spielstand zum Laden.");
            return;
        }
    };

    clear_world(world);
    world.insert_resource(save.grid);
    world.insert_resource(save.colony);
    world.insert_resource(save.clock);
    world.insert_resource(save.progress);
    world.insert_resource(save.priorities);
    world.insert_resource(ViewLayer(save.view));
    world.resource_mut::<Scenario>().stage = save.stage;
    world.run_system_cached_with(spawn_saved, save.rabbits).ok();
    world.resource_mut::<Messages>().push("Spielstand geladen.");
}

fn spawn_saved(In(rabbits): In<Vec<SavedRabbit>>, mut commands: Commands, asset_server: Res<AssetServer>, mut rng: ResMut<Rng>) {
    for rabbit in rabbits {
        rabbit.spawn(&mut commands, &asset_server, &mut rng);
    }
}
