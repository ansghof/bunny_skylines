//! Nur im Debug-Build: automatische Testläufe ohne Tastatur und Maus.
//!
//! `BUNNY_TEST` enthält durch Komma getrennte Anweisungen, z. B.
//! `BUNNY_TEST=unten,vorrat,nacht,tempo=3,bild=12 cargo run`
//!
//! - `unten` / `oben`: mit der Untergrund- bzw. Oberflächen-Ansicht starten
//! - `abstand=N`, `drehung=Grad`, `neigung=Grad`: Kamera einstellen
//! - `vorrat`: den Gang am Startbau zur Vorratskammer machen und 15 Futter einlagern
//! - `lager=N`: N zusätzliche Felder Vorratskammer neben dem Startbau
//! - `nester=N`: N zusätzliche Felder Schlafkammer, je zwei Plätze
//! - `graben`: Grabaufträge neben dem Startgang setzen
//! - `zeiger`: Mauszeiger fest über ein ungegrabenes Feld neben dem Startbau setzen
//! - `auswahl`: mit dem Kleefeld-Werkzeug von dort aus einen Bereich aufziehen (Maustaste bleibt gedrückt)
//! - `alt`: Die Gründer haben nur noch 5 Spielsekunden zu leben
//! - `hungersnot`: kein Gras, kein Vorrat, alle Kaninchen völlig ausgehungert
//! - `klee`: ein paar Kleefelder neben dem Eingang säen
//! - `tipp`: den Tooltip des Kleefeld-Knopfs zeigen
//! - `speichern=S`: nach S Sekunden speichern; `laden`: nach einer Sekunde den Spielstand laden
//! - `familie=N`: so viele erwachsene Kaninchen zusätzlich in der Schlafkammer
//! - `fangbild` oder `fangbild=S`: 0,8 s (oder S s) nach dem ersten Fang ein Bild machen
//! - `fuchs`: gleich einen Fuchs schicken
//! - `marder`: in der nächsten Nacht sofort einen Marder schicken (mit `nacht` gleich)
//! - `wachen`: Nachtwache im Startgang einrichten; `pfropf`: Erdpfropf im Startgang
//! - `bussard`: gleich einen Bussard schicken
//! - `karte_weide` / `karte_stimmung`: mit dieser Übersichtskarte starten
//! - `menue`: nach zwei Sekunden das Pausenmenü öffnen
//! - `kapitel=N`: gleich mit Kapitel N beginnen
//! - `kapitelwahl`: auf dem Titelbild die Kapitelwahl öffnen
//! - `titel`: auf dem Titelbild bleiben
//! - `intro`: die Einleitung des Kapitels offen lassen (sonst wird sie gleich geschlossen)
//! - `ziel`: Ziele des Kapitels sofort erfüllen, damit der Siegbildschirm erscheint
//! - `infos`: nach zwei Sekunden echt auf Schneeflocke klicken, damit das Infofeld aufgeht
//! - `winter`: mitten im verschneiten Winter starten
//! - `neustart`: sobald die Familie ausgestorben ist, „Neu beginnen“ drücken
//! - `forschung`: alles freischalten; `weisheit=N`: so viel Weisheit zum Start; `baum`: Forschungsbaum offen
//! - `nacht`: kurz vor Einbruch der Nacht starten
//!
//! Bei jedem Wechsel der Jahreszeit steht im Protokoll, wie viele Kaninchen leben und wie viel Futter da ist.
//! - `tempo=N`: Spielgeschwindigkeit
//! - `unsichtbar`: ohne sichtbares Fenster, für lange Läufe, die nur das Protokoll brauchen
//! - `ende=S`: nach S Sekunden Echtzeit ohne Screenshot beenden
//! - `bild=S`: nach S Sekunden Echtzeit einen Screenshot nach `target/test-bild.png` speichern und beenden
//! - `serie=N`: statt eines Bildes N Bilder im Abstand von 0,12 s (`target/test-bild-0.png` …)

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::camera::{CursorWorld, Orbit, update_cursor};
use crate::colony::{Colony, Messages, Rng};
use crate::daynight::{DayClock, Season};
use crate::ending::{EndScreen, restart};
use crate::map::{Chamber, Depth, Grid, Layer, TilePos, ViewLayer, tiles_between};
use crate::progress::Progress;
use crate::toolbar::{Action, ResearchOpen, Tooltip};
use crate::tools::{Tool, ToolKind, apply_drag};
use crate::behavior::{Activity, Reserved};
use crate::rabbit::{Hunger, Mood, OnLayer, Rabbit, spawn_extra};
use crate::inspect::{RABBIT_HEIGHT, pick_rabbit};
use crate::overlay::MapOverlay;
use crate::bird::BuzzardSpawner;
use crate::marten::MartenSpawner;
use crate::predator::FoxSpawner;
use crate::menu::{OpenMenu, TitleScreen, close_title, new_game};
use crate::save::{load_game, save_game};
use crate::scenario::{Chapter, Dangers, Scenario, Stage, Target, Terrain, play_on, start_chapter, start_playing};
use crate::ground::{floor_y, to_world3};
use crate::camera::MainCamera;
use bevy::window::PrimaryWindow;

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
            .add_systems(Update, (age_founders, famine, auto_restart, log_seasons, log_crowding, quit_after))
            .add_systems(PreUpdate, fake_cursor.after(update_cursor))
            .add_systems(Update, fake_press.before(apply_drag))
            .add_systems(Update, click_rabbit.before(pick_rabbit))
            .add_systems(Update, skip_intro)
            .add_systems(Update, log_messages)
            .add_systems(Startup, extra_family.after(prepare))
            .add_systems(Update, save_and_load)
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
    mut tooltip: ResMut<Tooltip>,
    mut progress: ResMut<Progress>,
    mut research: ResMut<ResearchOpen>,
    mut overlay: ResMut<MapOverlay>,
    mut buzzards: ResMut<BuzzardSpawner>,
    mut martens: ResMut<MartenSpawner>,
    mut foxes: ResMut<FoxSpawner>,
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
    if script.has("wachen") {
        grid.under_mut(TilePos::new(21, 15)).watch = true;
    }
    if script.has("pfropf") {
        grid.under_mut(TilePos::new(21, 15)).plug = true;
    }
    if script.has("vorrat") {
        for x in [21, 22] {
            grid.under_mut(TilePos::new(x, 15)).chamber = Some(Chamber::Storage);
        }
        colony.food = 15.0;
    }
    if let Some(tiles) = script.value("lager") {
        // Eine Reihe gegrabener Vorratsfelder über dem Startgang.
        for x in 20..20 + tiles as i32 {
            let tile = grid.under_mut(TilePos::new(x, 14));
            tile.dug = Some(Depth::Medium);
            tile.chamber = Some(Chamber::Storage);
        }
    }
    if let Some(tiles) = script.value("nester") {
        // Eine zweite Reihe Schlafkammer unter dem Startgang, je Feld zwei Plätze.
        for x in 20..20 + tiles as i32 {
            let tile = grid.under_mut(TilePos::new(x, 16));
            tile.dug = Some(Depth::Medium);
            tile.chamber = Some(Chamber::Sleeping);
        }
    }
    if script.has("graben") {
        for (x, y) in [(21, 16), (21, 17), (22, 17), (20, 17)] {
            grid.under_mut(TilePos::new(x, y)).planned = Some(Depth::Medium);
        }
    }
    if script.has("klee") {
        for p in tiles_between(TilePos::new(17, 13), TilePos::new(19, 17)) {
            let field = grid.surface_mut(p);
            field.clover = true;
            field.grass = 1.0;
        }
    }
    if script.has("tipp") {
        tooltip.0 = Some(Action::Tool(ToolKind::Clover));
    }
    if script.has("winter") {
        // Tag 6 am Vormittag: tiefer Winter, die Wiese liegt unter Schnee.
        clock.day = 6;
        clock.time = 0.3;
    }
    if script.has("forschung") {
        progress.unlock_all();
    }
    if let Some(wisdom) = script.value("weisheit") {
        progress.wisdom = wisdom;
    }
    if script.has("baum") {
        research.0 = true;
    }
    if script.has("bussard") {
        buzzards.summon();
    }
    if script.has("fuchs") {
        foxes.summon();
    }
    if script.has("marder") {
        martens.summon();
    }
    if script.has("karte_weide") {
        *overlay = MapOverlay::Pasture;
    }
    if script.has("karte_stimmung") {
        *overlay = MapOverlay::Mood;
    }
    if script.has("nacht") {
        clock.time = 0.74;
    }
    if let Some(speed) = script.value("tempo") {
        time.set_relative_speed(speed);
    }
}

/// Einmal, sobald die Gründer da sind.
fn age_founders(script: Res<TestScript>, mut done: Local<bool>, mut rabbits: Query<&mut Rabbit>) {
    if *done || !script.has("alt") || rabbits.is_empty() {
        return;
    }
    for mut rabbit in &mut rabbits {
        rabbit.set_remaining_life(5.0);
    }
    *done = true;
}

/// Hält Wiese und Vorrat leer und alle hungrig, solange der Testlauf dauert.
fn famine(
    script: Res<TestScript>,
    mut grid: ResMut<Grid>,
    mut colony: ResMut<Colony>,
    mut done: Local<bool>,
    mut rabbits: Query<&mut Hunger>,
) {
    if !script.has("hungersnot") {
        return;
    }
    colony.food = 0.0;
    for p in Grid::positions() {
        if grid.surface(p).grass > 0.0 {
            grid.surface_mut(p).grass = 0.0;
        }
    }
    if !*done && !rabbits.is_empty() {
        for mut hunger in &mut rabbits {
            hunger.0 = 1.0;
        }
        *done = true;
    }
}

fn auto_restart(script: Res<TestScript>, mut commands: Commands, screens: Query<(), With<EndScreen>>) {
    if script.has("neustart") && !screens.is_empty() {
        commands.run_system_cached(restart);
    }
}

fn log_seasons(
    clock: Res<DayClock>,
    colony: Res<Colony>,
    grid: Res<Grid>,
    rabbits: Query<(&Rabbit, &Mood)>,
    mut last: Local<Option<Season>>,
) {
    let season = clock.season();
    if *last == Some(season) {
        return;
    }
    *last = Some(season);
    let count = rabbits.iter().count();
    let young = rabbits.iter().filter(|(r, _)| !r.is_adult()).count();
    let mood = rabbits.iter().map(|(_, m)| m.value).sum::<f32>() / count.max(1) as f32;
    let tiles: Vec<f32> = Grid::positions().map(|p| grid.surface(p).fertility).collect();
    let soil = tiles.iter().sum::<f32>() / tiles.len() as f32;
    let worn = tiles.iter().filter(|f| **f < 0.5).count();
    info!(
        "Jahr {} {}: {count} Kaninchen ({young} Junge), Plätze {}, Futter {:.0}/{:.0}, Stimmung {:.0} %, Boden {:.0} %, ausgelaugt {worn}",
        clock.year(),
        season.label(),
        Colony::sleeping_places(&grid),
        colony.food,
        colony.food_capacity(&grid),
        mood * 100.0,
        soil * 100.0,
    );
}

/// Alle 5 Sekunden: die meisten Kaninchen, die sich gerade ein Feld teilen, unterwegs mitgezählt.
fn log_crowding(time: Res<Time<Real>>, mut timer: Local<f32>, reserved: Res<Reserved>, rabbits: Query<&Activity>) {
    *timer += time.delta_secs();
    if *timer < 5.0 {
        return;
    }
    *timer = 0.0;
    let grazing = rabbits
        .iter()
        .filter(|a| matches!(a, Activity::Eating { .. } | Activity::Gathering { .. } | Activity::Walking(_)))
        .count();
    info!("Gedraenge: hoechstens {} auf einem Feld, {grazing} unterwegs oder grasend", reserved.busiest_spot());
}

fn fake_cursor(script: Res<TestScript>, time: Res<Time<Real>>, mut cursor: ResMut<CursorWorld>) {
    if !script.has("zeiger") {
        return;
    }
    // Mit `auswahl` wandert der Zeiger nach dem Drücken fünf Felder weiter.
    let dragged = script.has("auswahl") && time.elapsed_secs() > 1.5;
    let tile = if dragged { TilePos::new(25, 12) } else { TilePos::new(21, 13) };
    cursor.0 = Some(tile.center());
}

/// Drückt nach einer Sekunde die linke Maustaste und hält sie; läuft direkt vor dem Werkzeug,
/// damit die Eingabeverarbeitung den Druck nicht vorher schon wieder löscht.
fn fake_press(
    script: Res<TestScript>,
    time: Res<Time<Real>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut tool: ResMut<Tool>,
) {
    if script.has("auswahl") && time.elapsed_secs() > 1.0 && !mouse.pressed(MouseButton::Left) {
        tool.kind = ToolKind::Clover;
        mouse.press(MouseButton::Left);
    }
}

/// Klickt mit der simulierten Maus auf Schneeflocke: Zeiger auf die Figur, drücken, im nächsten
/// Bild loslassen. So wird auch die Trefferprüfung mitgetestet.
fn click_rabbit(
    script: Res<TestScript>,
    time: Res<Time<Real>>,
    grid: Res<Grid>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    rabbits: Query<(&Rabbit, &Transform, &OnLayer)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut step: Local<u8>,
) {
    if !script.has("infos") || time.elapsed_secs() < 2.0 {
        return;
    }
    match *step {
        0 => {
            let Some((_, transform, layer)) = rabbits.iter().find(|(r, _, _)| r.name == "Schneeflocke") else {
                return;
            };
            let position = transform.translation.truncate();
            let floor = floor_y(&grid, layer.0, TilePos::from_world_clamped(position));
            let middle = to_world3(position, floor + RABBIT_HEIGHT / 2.0);
            let (camera, camera_transform) = *camera;
            if let Ok(screen) = camera.world_to_viewport(camera_transform, middle) {
                window.set_cursor_position(Some(screen));
                mouse.press(MouseButton::Left);
                *step = 1;
            }
        }
        1 => {
            mouse.release(MouseButton::Left);
            *step = 2;
        }
        _ => {}
    }
}

/// Ein Kapitel, dessen Ziele schon zu Beginn erfüllt sind.
static EASY_CHAPTER: Chapter = Chapter {
    number: 1,
    title: "Die Hecke",
    story: "Testkapitel",
    targets: &[Target::Rabbits(2), Target::Food(5.0)],
    max_losses: None,
    terrain: Terrain::Meadow,
    dangers: Dangers { first_fox: 300.0, fox_pace: 1.0, buzzard_from_start: false, marten_from_start: false },
    known: &[],
    relatives: 0,
    start_earth: 0.0,
};

fn save_and_load(
    script: Res<TestScript>,
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut menu: ResMut<OpenMenu>,
    mut done: Local<(bool, bool, bool)>,
) {
    let now = time.elapsed_secs();
    if !done.2 && script.has("menue") && now >= 2.0 {
        done.2 = true;
        menu.0 = true;
    }
    if !done.0 && script.value("speichern").is_some_and(|at| now >= at) {
        done.0 = true;
        commands.run_system_cached(save_game);
    }
    if !done.1 && script.has("laden") && now >= 1.0 {
        done.1 = true;
        commands.run_system_cached(load_game);
    }
}

/// `familie=N`: so viele erwachsene Kaninchen zusätzlich in der Schlafkammer.
fn extra_family(script: Res<TestScript>, mut commands: Commands, asset_server: Res<AssetServer>, mut rng: ResMut<Rng>) {
    let Some(count) = script.value("familie") else {
        return;
    };
    for i in 0..count as usize {
        let tile = TilePos::new(23 + (i % 2) as i32, 14 + (i / 2 % 3) as i32);
        spawn_extra(&mut commands, &asset_server, &mut rng, format!("Test {}", i + 1), Layer::Underground, tile.center());
    }
}

/// Jede neue Meldung auch ins Protokoll, damit Testläufe nachlesbar sind.
fn log_messages(
    mut commands: Commands,
    script: Res<TestScript>,
    time: Res<Time<Real>>,
    messages: Res<Messages>,
    mut seen: Local<Option<String>>,
    mut caught_at: Local<Option<f32>>,
) {
    // `fangbild`: kurz nach dem ersten Fang ein Bild, um die weggetragene Beute zu sehen.
    if let Some(at) = *caught_at
        && time.elapsed_secs() > at + script.value("fangbild").unwrap_or(0.8)
    {
        *caught_at = Some(f32::INFINITY);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(SHOT_PATH));
    }
    if !messages.is_changed() {
        return;
    }
    // Neue Meldungen stehen vorn; bis zur zuletzt gesehenen alles protokollieren.
    let fresh: Vec<&String> = messages.0.iter().take_while(|m| Some(*m) != seen.as_ref()).collect();
    for message in fresh.iter().rev() {
        info!("Meldung: {message}");
        let wanted = script.has("fangbild") || script.value("fangbild").is_some();
        if wanted && caught_at.is_none() && message.contains(" wurde vom ") {
            *caught_at = Some(time.elapsed_secs());
        }
    }
    if let Some(first) = messages.0.front() {
        *seen = Some(first.clone());
    }
}

/// Testläufe sollen nicht an der Einleitung hängen bleiben.
fn skip_intro(
    script: Res<TestScript>,
    mut scenario: ResMut<Scenario>,
    mut commands: Commands,
    titles: Query<(), With<TitleScreen>>,
    screens: Query<(), With<EndScreen>>,
    time: Res<Time<Real>>,
    mut done: Local<(bool, bool)>,
) {
    if script.has("ziel") {
        scenario.chapter = &EASY_CHAPTER;
    }
    // Lange Testläufe sollen nicht am Siegbildschirm stehen bleiben (außer mit `ziel`).
    if scenario.stage == Stage::Won && !script.has("ziel") {
        commands.run_system_cached(play_on);
    }
    // Erst das Titelbild überspringen (außer mit `titel`), dann die Einleitung (außer mit `intro`).
    // Mit `kapitel=N` geht es gleich in dieses Kapitel.
    if script.has("kapitelwahl") && !done.0 && time.elapsed_secs() > 2.0 {
        done.0 = true;
        commands.run_system_cached(new_game);
        return;
    }
    if !done.0 && !script.has("titel") && !script.has("kapitelwahl") {
        done.0 = true;
        commands.run_system_cached(close_title);
        let number = script.value("kapitel").map_or(1, |n| n as u32);
        commands.run_system_cached_with(start_chapter, number);
        return;
    }
    let intro_open = titles.is_empty() && !screens.is_empty();
    if done.1 || script.has("intro") || scenario.stage != Stage::Intro || !intro_open {
        return;
    }
    let done = &mut done.1;
    *done = true;
    commands.run_system_cached(start_playing);
}

fn quit_after(script: Res<TestScript>, time: Res<Time<Real>>, mut exit: MessageWriter<AppExit>) {
    if script.value("ende").is_some_and(|at| time.elapsed_secs() >= at) {
        exit.write(AppExit::Success);
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
