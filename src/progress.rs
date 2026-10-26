//! Fortschritt der Familie: Wachstumsstufen, Weisheit und der Forschungsbaum.
//!
//! In der Erzählkammer sammeln Kaninchen Weisheit, alte Kaninchen mehr als junge.
//! Mit Weisheit schaltet der Spieler Bauwerke und Fähigkeiten frei; höhere Stufen im Baum
//! brauchen die passende Wachstumsstufe (Erdloch → Bau → Siedlung → Kaninchenstadt).

use std::collections::HashSet;

use bevy::prelude::*;

use crate::colony::{Colony, FOOD_PER_STORAGE_TILE, Messages};
use crate::daynight::{DayClock, Season};
use crate::rabbit::Rabbit;

pub struct ProgressPlugin;

impl Plugin for ProgressPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Progress>()
            .add_systems(Update, (count_winters, grow_stage, apply_research).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum Stage {
    Burrow,
    Warren,
    Settlement,
    Town,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Burrow => "Erdloch",
            Stage::Warren => "Bau",
            Stage::Settlement => "Siedlung",
            Stage::Town => "Kaninchenstadt",
        }
    }

    /// Was es braucht, um diese Stufe zu erreichen: Kaninchen und überstandene Winter.
    fn requirement(self) -> (usize, u32) {
        match self {
            Stage::Burrow => (0, 0),
            Stage::Warren => (8, 0),
            Stage::Settlement => (15, 1),
            Stage::Town => (30, 2),
        }
    }

    fn next(self) -> Option<Stage> {
        match self {
            Stage::Burrow => Some(Stage::Warren),
            Stage::Warren => Some(Stage::Settlement),
            Stage::Settlement => Some(Stage::Town),
            Stage::Town => None,
        }
    }

    /// Was zur nächsten Stufe fehlt, für die Anzeige.
    pub fn next_goal(self) -> Option<String> {
        let next = self.next()?;
        let (rabbits, winters) = next.requirement();
        let winters = match winters {
            0 => String::new(),
            1 => " und 1 überstandener Winter".into(),
            n => format!(" und {n} überstandene Winter"),
        };
        Some(format!("{}: {rabbits} Kaninchen{winters}", next.label()))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Branch {
    Digging,
    Food,
    Safety,
    Community,
}

impl Branch {
    pub const ALL: [Branch; 4] = [Branch::Digging, Branch::Food, Branch::Safety, Branch::Community];

    pub fn label(self) -> &'static str {
        match self {
            Branch::Digging => "Graben",
            Branch::Food => "Nahrung",
            Branch::Safety => "Sicherheit",
            Branch::Community => "Gemeinschaft",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum Tech {
    Claws,
    DeepTunnels,
    WideTunnels,
    Clover,
    DriedHay,
    Herbs,
    Posts,
    Hedges,
    BoltHoles,
    SunSpots,
    Nursery,
    Playground,
}

impl Tech {
    pub const ALL: [Tech; 12] = [
        Tech::Claws,
        Tech::DeepTunnels,
        Tech::WideTunnels,
        Tech::Clover,
        Tech::DriedHay,
        Tech::Herbs,
        Tech::Posts,
        Tech::Hedges,
        Tech::BoltHoles,
        Tech::SunSpots,
        Tech::Nursery,
        Tech::Playground,
    ];

    pub fn branch(self) -> Branch {
        match self {
            Tech::Claws | Tech::DeepTunnels | Tech::WideTunnels => Branch::Digging,
            Tech::Clover | Tech::DriedHay | Tech::Herbs => Branch::Food,
            Tech::Posts | Tech::Hedges | Tech::BoltHoles => Branch::Safety,
            Tech::SunSpots | Tech::Nursery | Tech::Playground => Branch::Community,
        }
    }

    /// Stufe im Ast, 1 bis 3.
    pub fn tier(self) -> u8 {
        match self {
            Tech::Claws | Tech::Clover | Tech::Posts | Tech::SunSpots => 1,
            Tech::DeepTunnels | Tech::DriedHay | Tech::Hedges | Tech::Nursery => 2,
            Tech::WideTunnels | Tech::Herbs | Tech::BoltHoles | Tech::Playground => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tech::Claws => "Grabkrallen",
            Tech::DeepTunnels => "Tiefe Gänge",
            Tech::WideTunnels => "Breite Gänge",
            Tech::Clover => "Kleefeld",
            Tech::DriedHay => "Heu trocknen",
            Tech::Herbs => "Kräutergarten",
            Tech::Posts => "Wachposten",
            Tech::Hedges => "Dornenhecke",
            Tech::BoltHoles => "Fluchtlöcher",
            Tech::SunSpots => "Sonnenplatz",
            Tech::Nursery => "Kinderstube",
            Tech::Playground => "Spielwiese",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Tech::Claws => "Alle graben 30 % schneller.",
            Tech::DeepTunnels => "Tief graben: sicher vor Füchsen, gut für Vorräte.",
            Tech::WideTunnels => "Unter der Erde hoppeln alle 25 % schneller.",
            Tech::Clover => "Klee säen: wächst schnell, nahrhaft, heilt ausgelaugten Boden.",
            Tech::DriedHay => "Getrocknetes Heu lagert dichter: Vorratskammern fassen doppelt so viel.",
            Tech::Herbs => "Kräuterfeld säen: Wer dort frisst, wird deutlich zufriedener.",
            Tech::Posts => "Wachposten bauen: Wachen entdecken Füchse früh.",
            Tech::Hedges => "Dornenhecken pflanzen: Füchse kommen nur langsam hindurch.",
            Tech::BoltHoles => "Fluchtlöcher graben: billige Verstecke draußen auf der Weide.",
            Tech::SunSpots => "Sonnenplatz anlegen: Kaninchen in der Nähe werden zufriedener.",
            Tech::Nursery => "Kinderstube einrichten: Junge wachsen doppelt so schnell, Würfe werden größer.",
            Tech::Playground => "Spielwiese anlegen: Die ganze Familie ist zufriedener.",
        }
    }

    pub fn cost(self) -> f32 {
        match self.tier() {
            1 => 10.0,
            2 => 25.0,
            _ => 50.0,
        }
    }

    /// Ab welcher Wachstumsstufe sich die Forschung freischalten lässt.
    pub fn stage(self) -> Stage {
        match self.tier() {
            1 => Stage::Burrow,
            2 => Stage::Warren,
            _ => Stage::Settlement,
        }
    }

    /// Die Forschung davor im selben Ast.
    pub fn prerequisite(self) -> Option<Tech> {
        Tech::ALL
            .into_iter()
            .find(|t| t.branch() == self.branch() && t.tier() + 1 == self.tier())
    }
}

#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct Progress {
    pub stage: Stage,
    pub wisdom: f32,
    pub winters_survived: u32,
    unlocked: HashSet<Tech>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            stage: Stage::Burrow,
            wisdom: 0.0,
            winters_survived: 0,
            unlocked: HashSet::new(),
        }
    }
}

/// Warum sich eine Forschung gerade nicht freischalten lässt.
pub enum Blocked {
    Done,
    NeedsPrerequisite(Tech),
    NeedsStage(Stage),
    NeedsWisdom(f32),
}

impl Progress {
    /// Eine Forschung ohne Kosten freischalten, z. B. was die Familie aus früheren Kapiteln kann.
    pub fn grant(&mut self, tech: Tech) {
        self.unlocked.insert(tech);
    }

    pub fn has(&self, tech: Tech) -> bool {
        self.unlocked.contains(&tech)
    }

    pub fn check(&self, tech: Tech) -> Result<(), Blocked> {
        if self.has(tech) {
            return Err(Blocked::Done);
        }
        if let Some(before) = tech.prerequisite()
            && !self.has(before)
        {
            return Err(Blocked::NeedsPrerequisite(before));
        }
        if self.stage < tech.stage() {
            return Err(Blocked::NeedsStage(tech.stage()));
        }
        if self.wisdom < tech.cost() {
            return Err(Blocked::NeedsWisdom(tech.cost() - self.wisdom));
        }
        Ok(())
    }

    /// Schaltet frei, wenn alles passt; gibt zurück, ob es geklappt hat.
    pub fn research(&mut self, tech: Tech) -> bool {
        if self.check(tech).is_err() {
            return false;
        }
        self.wisdom -= tech.cost();
        self.unlocked.insert(tech);
        true
    }

    /// Nur für Testläufe: alles freischalten.
    #[cfg(debug_assertions)]
    pub fn unlock_all(&mut self) {
        self.unlocked.extend(Tech::ALL);
    }
}

/// Forschungen, die dauerhaft eine Zahl verändern, statt ein Werkzeug freizuschalten.
fn apply_research(progress: Res<Progress>, mut colony: ResMut<Colony>) {
    // Getrocknetes Heu lagert doppelt so dicht.
    let per_tile = if progress.has(Tech::DriedHay) {
        FOOD_PER_STORAGE_TILE * 2.0
    } else {
        FOOD_PER_STORAGE_TILE
    };
    if colony.storage_per_tile != per_tile {
        colony.storage_per_tile = per_tile;
    }
}

fn count_winters(clock: Res<DayClock>, mut progress: ResMut<Progress>, mut last: Local<Option<Season>>) {
    let season = clock.season();
    if *last == Some(Season::Winter) && season == Season::Spring {
        progress.winters_survived += 1;
    }
    *last = Some(season);
}

fn grow_stage(mut progress: ResMut<Progress>, mut messages: ResMut<Messages>, rabbits: Query<(), With<Rabbit>>) {
    let Some(next) = progress.stage.next() else {
        return;
    };
    let (needed, winters) = next.requirement();
    if rabbits.iter().count() >= needed && progress.winters_survived >= winters {
        progress.stage = next;
        messages.push(match next {
            Stage::Burrow | Stage::Warren => "Aus dem Erdloch ist ein richtiger Bau geworden! Neue Forschungen sind möglich.",
            Stage::Settlement => "Der Bau ist zur Siedlung gewachsen! Neue Forschungen sind möglich.",
            Stage::Town => "Die Siedlung ist jetzt eine Kaninchenstadt!",
        });
    }
}
