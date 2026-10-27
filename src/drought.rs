//! Dürre auf der Trockenwiese: Jeden Sommer kommen heiße Tage. Erst warnt das Spiel, dann
//! verdorrt das Gras auf der offenen Wiese und wächst kaum nach. Nur im Schatten von Hecken und
//! Bäumen und am Tümpel bleibt es grün. Wer vorher Vorräte angelegt hat, kommt gut durch.

use bevy::prelude::*;

use crate::audio::{Sound, Sounds};
use crate::colony::Messages;
use crate::daynight::{DAY_LENGTH, DayClock, Season};
use crate::map::Grid;
use crate::scenario::Scenario;

/// So lange vorher warnt das Spiel, in Sekunden.
const WARNING_SECONDS: f32 = 40.0;
/// So lange dauert die Dürre: fast den ganzen restlichen Sommer.
const DRY_SECONDS: f32 = DAY_LENGTH * 0.85;
/// Die heißen Tage beginnen, wenn so viel vom Sommer vorbei ist …
const STARTS_AT: f32 = 0.15;
/// … aber nicht mehr, wenn der Sommer schon fast um ist (z. B. nach dem Laden eines Spielstands).
const TOO_LATE: f32 = 0.5;
/// So schnell kommt und geht die Dürre: in so vielen Sekunden von nichts bis ganz.
const FADE_SECONDS: f32 = 30.0;

pub struct DroughtPlugin;

impl Plugin for DroughtPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Drought>().add_systems(Update, (schedule_drought, parch).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DroughtPhase {
    #[default]
    Calm,
    /// Heiße Tage: Gleich beginnt die Dürre.
    Warning,
    Dry,
}

#[derive(Resource, Default)]
pub struct Drought {
    pub phase: DroughtPhase,
    timer: f32,
    /// In welchem Jahr die Dürre schon kam.
    year: u32,
    /// Wie stark die Dürre gerade ist; kommt und geht langsam.
    level: f32,
}

impl Drought {
    /// Sofort warnen, z. B. für Tests.
    pub fn summon(&mut self) {
        self.phase = DroughtPhase::Warning;
        self.timer = WARNING_SECONDS;
    }
}

fn schedule_drought(
    clock: Res<DayClock>,
    scenario: Res<Scenario>,
    mut drought: ResMut<Drought>,
    mut messages: ResMut<Messages>,
) {
    if !scenario.chapter.terrain.has_droughts() || drought.phase != DroughtPhase::Calm {
        return;
    }
    let progress = clock.season_progress();
    let hot = clock.season() == Season::Summer && (STARTS_AT..TOO_LATE).contains(&progress);
    if !hot || drought.year == clock.year() {
        return;
    }
    drought.year = clock.year();
    drought.summon();
    messages.push(
        "Heiße Tage: Die Sonne brennt, bald kommt die Dürre! Jetzt Vorräte anlegen. \
         Im Schatten von Hecken und Bäumen und am Tümpel bleibt das Gras grün.",
    );
}

fn parch(
    time: Res<Time>,
    mut drought: ResMut<Drought>,
    mut grid: ResMut<Grid>,
    mut messages: ResMut<Messages>,
    mut sounds: ResMut<Sounds>,
) {
    let elapsed = time.delta_secs();
    if drought.phase != DroughtPhase::Calm {
        drought.timer -= elapsed;
        if drought.timer <= 0.0 {
            if drought.phase == DroughtPhase::Warning {
                drought.phase = DroughtPhase::Dry;
                drought.timer = DRY_SECONDS;
                messages.push("Dürre! Auf der offenen Wiese verdorrt das Gras und wächst kaum noch nach.");
            } else {
                drought.phase = DroughtPhase::Calm;
                sounds.play(Sound::Flood);
                messages.push("Endlich Regen! Die Dürre ist vorbei, das Gras erholt sich.");
            }
        }
    }
    let target = if drought.phase == DroughtPhase::Dry { 1.0 } else { 0.0 };
    let step = elapsed / FADE_SECONDS;
    let level = drought.level;
    drought.level = if level < target { (level + step).min(target) } else { (level - step).max(target) };
    // Nur in kleinen Stufen weitergeben, damit der Boden nicht jedes Bild neu gefärbt wird.
    if (drought.level - grid.drought).abs() >= 0.05 || (drought.level == target && grid.drought != target) {
        grid.drought = drought.level;
    }
}
