//! Kampagne: Jedes Kapitel erzählt, wohin die Familie Buddelberg zieht, und setzt Ziele.
//! Kapitel haben ihre eigene Landschaft und Gefahren. Sind alle Ziele erreicht, ist das Kapitel
//! geschafft und das nächste wird frei; danach geht es auf Wunsch frei weiter.

use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::audio::{Sound, Sounds};
use crate::colony::Colony;
use crate::daynight::DayClock;
use crate::ending::{EndScreen, restart, spawn_button, spawn_overlay};
use crate::map::Grid;
use crate::progress::Tech;
use crate::rabbit::Rabbit;
use crate::save::{load_game, save_exists, unlock_chapter};

pub struct ScenarioPlugin;

impl Plugin for ScenarioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Scenario>()
            .init_resource::<Losses>()
            .add_systems(Update, check_goals);
    }
}

/// Ein Ziel des Kapitels.
#[derive(Clone, Copy)]
pub enum Target {
    /// So viele Kaninchen in der Familie.
    Rabbits(usize),
    /// So viel Futter im Vorrat.
    Food(f32),
    /// Durchhalten bis zum Frühling dieses Jahres.
    Year(u32),
}

/// Was die Ziele prüfen.
pub struct Snapshot {
    pub rabbits: usize,
    pub food: f32,
    pub year: u32,
}

impl Target {
    fn label(self) -> String {
        match self {
            Target::Rabbits(n) => format!("{n} Kaninchen"),
            Target::Food(n) => format!("{n:.0} Futter im Vorrat"),
            Target::Year(n) => format!("bis zum Frühling von Jahr {n}"),
        }
    }

    /// Wie weit die Familie ist, und wie weit sie kommen muss.
    fn progress(self, now: &Snapshot) -> (f32, f32) {
        match self {
            Target::Rabbits(n) => (now.rabbits as f32, n as f32),
            Target::Food(n) => (now.food.floor(), n),
            Target::Year(n) => (now.year as f32, n as f32),
        }
    }
}

/// Wie die Karte eines Kapitels aussieht.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terrain {
    Meadow,
    /// Wald am oberen Rand; von dort kommen die Füchse.
    ForestEdge,
    /// Ein alter Steinwall mit Fels darunter; darin wohnen Marder.
    StoneWall,
    /// Fast überall Sand: nur schmale Gänge, wenige feste Stellen.
    SandPit,
    /// Ein Bach mit Aue, der im Frühling und nach Gewittern über die Ufer tritt.
    Brook,
}

impl Terrain {
    pub fn grid(self) -> Grid {
        match self {
            Terrain::Meadow => Grid::starting_meadow(),
            Terrain::ForestEdge => Grid::forest_edge(),
            Terrain::StoneWall => Grid::stone_wall(),
            Terrain::SandPit => Grid::sand_pit(),
            Terrain::Brook => Grid::brook(),
        }
    }
}

/// Welche Feinde ein Kapitel wie früh schickt.
pub struct Dangers {
    /// Spielsekunden bis zum ersten Fuchs.
    pub first_fox: f32,
    /// So viel öfter als gewöhnlich kommen Füchse.
    pub fox_pace: f32,
    /// Der Bussard kommt nicht erst ab der Stufe „Bau“.
    pub buzzard_from_start: bool,
    /// Der Marder kommt nicht erst ab der Stufe „Siedlung“.
    pub marten_from_start: bool,
}

pub struct Chapter {
    pub number: u32,
    pub title: &'static str,
    pub story: &'static str,
    pub targets: &'static [Target],
    /// Verliert die Familie mehr Kaninchen an Feinde, ist das Kapitel verloren.
    pub max_losses: Option<u32>,
    pub terrain: Terrain,
    pub dangers: Dangers,
    /// Was die Familie aus früheren Kapiteln schon kann.
    pub known: &'static [Tech],
    /// So viele erwachsene Kinder ziehen mit den Gründern um.
    pub relatives: usize,
    /// Erde, die die Familie mitbringt.
    pub start_earth: f32,
    /// Nachtwache und Erdpfropf gibt es von Anfang an.
    pub night_tools: bool,
}

const THE_HEDGE: Chapter = Chapter {
    number: 1,
    title: "Die Hecke",
    story: "Der Bauer hat die alte Wiese umgepflügt. Schneeflocke und Lukas haben ein paar \
            Grashalme gerettet und ein neues Plätzchen gefunden: eine sonnige Wiese an einer Hecke.\n\n\
            Grabt Gänge und Kammern, sammelt Gras und macht die Wiese zum Zuhause \
            für eine richtige Familie.",
    targets: &[Target::Rabbits(10), Target::Food(30.0)],
    max_losses: None,
    terrain: Terrain::Meadow,
    dangers: Dangers { first_fox: 600.0, fox_pace: 0.4, buzzard_from_start: false, marten_from_start: false },
    known: &[],
    relatives: 0,
    start_earth: 0.0,
    night_tools: false,
};

const THE_FOREST_EDGE: Chapter = Chapter {
    number: 2,
    title: "Der Waldrand",
    story: "Die Familie zieht weiter, an den Rand des großen Waldes. Hier wächst saftiges Gras, \
            und unter den Bäumen halten Wurzeln den Boden zusammen. Doch aus dem Wald schleichen Füchse, \
            und über der Lichtung kreist ein Bussard.\n\n\
            Stellt Wachposten auf, pflanzt Hecken und grabt Fluchtlöcher. Bringt die Familie durch \
            das erste Jahr, ohne mehr als sechs Kaninchen an die Räuber zu verlieren.",
    targets: &[Target::Rabbits(15), Target::Year(2)],
    max_losses: Some(6),
    terrain: Terrain::ForestEdge,
    dangers: Dangers { first_fox: 150.0, fox_pace: 1.1, buzzard_from_start: true, marten_from_start: false },
    known: &[Tech::Posts, Tech::Hedges, Tech::Clover],
    relatives: 4,
    start_earth: 15.0,
    night_tools: false,
};

const THE_STONE_WALL: Chapter = Chapter {
    number: 3,
    title: "Der alte Steinwall",
    story: "Hinter dem Wald liegt eine geschützte Wiese an einem alten Steinwall. Füchse kommen \
            selten hierher – doch in den Ritzen des Walls wohnen Marder, und nachts schleichen sie \
            in den Bau.\n\n\
            Setzt Nachtwachen ein, verschließt Gänge mit Erdpfropfen und grabt Rundgänge mit \
            mehreren Ausgängen, damit niemand in der Falle sitzt. Unter dem Wall ist Fels, \
            dort kann man nicht graben.",
    targets: &[Target::Rabbits(20), Target::Year(2)],
    max_losses: Some(4),
    terrain: Terrain::StoneWall,
    dangers: Dangers { first_fox: 400.0, fox_pace: 0.5, buzzard_from_start: false, marten_from_start: true },
    known: &[Tech::Posts, Tech::Hedges, Tech::Clover, Tech::Claws, Tech::DeepTunnels],
    relatives: 6,
    start_earth: 20.0,
    night_tools: true,
};

const THE_SAND_PIT: Chapter = Chapter {
    number: 4,
    title: "Die Sandgrube",
    story: "Die Familie findet eine weite Wiese an einer alten Sandgrube. Gras gibt es reichlich, \
            und Feinde kommen selten. Doch der Boden ist locker: Im Sand halten nur schmale Gänge, \
            breitere Kammern gehen nur dort, wo Erde oder Wurzeln den Boden zusammenhalten.\n\n\
            Sucht die festen Stellen, plant den Bau sorgfältig und baut genug Nester und Vorräte \
            für eine große Familie.",
    targets: &[Target::Rabbits(30), Target::Food(60.0)],
    max_losses: None,
    terrain: Terrain::SandPit,
    dangers: Dangers { first_fox: 300.0, fox_pace: 0.5, buzzard_from_start: false, marten_from_start: false },
    known: &[
        Tech::Posts,
        Tech::Hedges,
        Tech::Clover,
        Tech::Claws,
        Tech::DeepTunnels,
        Tech::DriedHay,
        Tech::SunSpots,
        Tech::Nursery,
    ],
    relatives: 4,
    start_earth: 10.0,
    night_tools: true,
};

const THE_BROOK: Chapter = Chapter {
    number: 5,
    title: "Am Bach",
    story: "Die Familie zieht an einen munteren Bach. In der Aue am Ufer wächst das saftigste Gras \
            weit und breit. Doch zur Schneeschmelze und nach Gewittern tritt der Bach über die Ufer, \
            und das Wasser läuft durch die Eingänge in den Bau – zuerst in die tiefsten Gänge.\n\n\
            Baut Eingänge lieber nicht zu nah am Wasser, schützt den Bau mit Dämmen und Erdpfropfen \
            und legt Vorräte nicht ganz unten an. Über den Bach kommt ihr nur an den Furten. \
            Bringt die Familie durch zwei Frühlingshochwasser.",
    targets: &[Target::Rabbits(25), Target::Year(3)],
    max_losses: None,
    terrain: Terrain::Brook,
    dangers: Dangers { first_fox: 400.0, fox_pace: 0.5, buzzard_from_start: false, marten_from_start: false },
    known: &[
        Tech::Posts,
        Tech::Hedges,
        Tech::Clover,
        Tech::Claws,
        Tech::DeepTunnels,
        Tech::DriedHay,
        Tech::BoltHoles,
        Tech::SunSpots,
        Tech::Nursery,
    ],
    relatives: 4,
    start_earth: 25.0,
    night_tools: true,
};

pub const CHAPTERS: [&Chapter; 5] = [&THE_HEDGE, &THE_FOREST_EDGE, &THE_STONE_WALL, &THE_SAND_PIT, &THE_BROOK];

/// Das Kapitel mit dieser Nummer; unbekannte Nummern ergeben das erste.
pub fn chapter(number: u32) -> &'static Chapter {
    CHAPTERS.iter().find(|c| c.number == number).copied().unwrap_or(&THE_HEDGE)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Stage {
    /// Die Einleitung ist offen, das Spiel wartet.
    Intro,
    Playing,
    /// Alle Ziele erreicht, der Siegbildschirm ist offen.
    Won,
    /// Zu viele Verluste, das Kapitel ist verloren.
    Lost,
    /// Nach dem Kapitel geht es ohne Ziele weiter.
    Free,
}

#[derive(Resource)]
pub struct Scenario {
    pub chapter: &'static Chapter,
    pub stage: Stage,
}

impl Default for Scenario {
    fn default() -> Self {
        Self { chapter: &THE_HEDGE, stage: Stage::Intro }
    }
}

/// Wie viele Kaninchen in diesem Kapitel Feinden zum Opfer gefallen sind.
#[derive(Resource, Default)]
pub struct Losses(pub u32);

impl Scenario {
    /// Neubeginn nach dem Aussterben: dasselbe Kapitel, aber ohne erneute Einleitung.
    pub fn restart(&mut self) {
        self.stage = Stage::Playing;
    }

    /// Zeile für die Statusanzeige, z. B. „Kapitel 1 „Die Hecke“: 10 Kaninchen (4/10) · …“.
    pub fn goal_line(&self, now: &Snapshot, losses: u32) -> Option<String> {
        if self.stage == Stage::Free {
            return None;
        }
        let mut goals: Vec<String> = self
            .chapter
            .targets
            .iter()
            .map(|target| {
                let (have, need) = target.progress(now);
                match target {
                    _ if have >= need => format!("{} (geschafft)", target.label()),
                    Target::Year(_) => format!("{} (jetzt Jahr {have:.0})", target.label()),
                    _ => format!("{} ({have:.0}/{need:.0})", target.label()),
                }
            })
            .collect();
        if let Some(max) = self.chapter.max_losses {
            goals.push(format!("Verluste durch Feinde {losses}/{max}"));
        }
        Some(format!("Kapitel {} „{}“: {}", self.chapter.number, self.chapter.title, goals.join(" · ")))
    }

    fn reached(&self, now: &Snapshot) -> bool {
        self.chapter.targets.iter().all(|target| {
            let (have, need) = target.progress(now);
            have >= need
        })
    }
}

/// Nach „Neues Spiel“: Wohin es geht und was zu tun ist. Das Spiel steht still, bis es losgeht.
pub fn show_intro(mut commands: Commands, scenario: Res<Scenario>, mut time: ResMut<Time<Virtual>>) {
    if scenario.stage != Stage::Intro {
        return;
    }
    time.pause();
    let chapter = scenario.chapter;
    let mut goals: Vec<String> = chapter.targets.iter().map(|t| format!("• {}", t.label())).collect();
    if let Some(max) = chapter.max_losses {
        goals.push(format!("• höchstens {max} Kaninchen an Feinde verlieren"));
    }
    let body = format!("{}\n\nZiele:\n{}", chapter.story, goals.join("\n"));
    let panel = spawn_overlay(&mut commands, &format!("Kapitel {}: {}", chapter.number, chapter.title), &body);
    spawn_button(&mut commands, panel, "Los geht’s!").observe(|click: On<PointerClick>, mut commands: Commands| {
        if click.button == PointerButton::Primary {
            commands.run_system_cached(start_playing);
        }
    });
    if save_exists() {
        spawn_button(&mut commands, panel, "Spielstand laden").observe(|click: On<PointerClick>, mut commands: Commands| {
            if click.button == PointerButton::Primary {
                commands.run_system_cached(load_game);
            }
        });
    }
}

/// Ein Kapitel von vorn beginnen: frische Karte des Kapitels, Gründer, Einleitung.
pub fn start_chapter(In(number): In<u32>, world: &mut World) {
    world.resource_mut::<Scenario>().chapter = chapter(number);
    restart(world);
    world.resource_mut::<Scenario>().stage = Stage::Intro;
    world.run_system_cached(show_intro).ok();
}

/// Schließt die Einleitung und lässt die Zeit laufen.
pub fn start_playing(
    mut commands: Commands,
    mut scenario: ResMut<Scenario>,
    mut time: ResMut<Time<Virtual>>,
    screens: Query<Entity, With<EndScreen>>,
) {
    for screen in &screens {
        commands.entity(screen).despawn();
    }
    if scenario.stage == Stage::Intro {
        scenario.stage = Stage::Playing;
    }
    time.unpause();
}

fn check_goals(
    mut commands: Commands,
    mut scenario: ResMut<Scenario>,
    mut time: ResMut<Time<Virtual>>,
    clock: Res<DayClock>,
    colony: Res<Colony>,
    losses: Res<Losses>,
    mut sounds: ResMut<Sounds>,
    rabbits: Query<(), With<Rabbit>>,
) {
    if scenario.stage != Stage::Playing {
        return;
    }
    let chapter = scenario.chapter;
    let now = Snapshot { rabbits: rabbits.iter().count(), food: colony.food, year: clock.year() };

    // Zu viele Verluste: verloren, aber gleich noch einmal versuchbar.
    if let Some(max) = chapter.max_losses.filter(|max| losses.0 > *max) {
        scenario.stage = Stage::Lost;
        time.pause();
        sounds.play(Sound::Extinct);
        let body = format!(
            "Die Räuber haben {} Kaninchen geholt, das sind mehr als {max}.\n\n\
             Wachposten entdecken Füchse früh, in Hecken und Fluchtlöchern sind die Kaninchen sicher.",
            losses.0,
        );
        let title = format!("Kapitel {}: {} – nicht geschafft", chapter.number, chapter.title);
        let panel = spawn_overlay(&mut commands, &title, &body);
        let number = chapter.number;
        spawn_button(&mut commands, panel, "Noch einmal versuchen").observe(
            move |click: On<PointerClick>, mut commands: Commands| {
                if click.button == PointerButton::Primary {
                    commands.run_system_cached_with(start_chapter, number);
                }
            },
        );
        return;
    }

    if !scenario.reached(&now) {
        return;
    }
    scenario.stage = Stage::Won;
    time.pause();
    sounds.play(Sound::Victory);
    unlock_chapter(chapter.number + 1);
    let next = CHAPTERS.iter().find(|c| c.number == chapter.number + 1);
    let outlook = match next {
        Some(next) => format!("Kapitel {} „{}“ ist jetzt frei.", next.number, next.title),
        None => "Die nächsten Kapitel folgen bald. Bis dahin kannst du hier frei weiterbauen.".into(),
    };
    let feat = match chapter.terrain {
        Terrain::Meadow => "sich an der Hecke eingelebt",
        Terrain::ForestEdge => "das Jahr am Waldrand überstanden",
        Terrain::StoneWall => "den Winter am Steinwall ohne große Verluste überstanden",
        Terrain::SandPit => "an der Sandgrube eine große Familie gegründet",
        Terrain::Brook => "zwei Hochwasser am Bach überstanden",
    };
    let body = format!(
        "Die Familie Buddelberg hat {feat}: {} Kaninchen und {:.0} Futter im Vorrat, im {} von Jahr {}.\n\n{outlook}",
        now.rabbits,
        colony.food,
        clock.season().label(),
        clock.year(),
    );
    let panel = spawn_overlay(&mut commands, &format!("Geschafft! Kapitel {}: {}", chapter.number, chapter.title), &body);
    if let Some(next) = next {
        let number = next.number;
        spawn_button(&mut commands, panel, &format!("Weiter zu Kapitel {number}")).observe(
            move |click: On<PointerClick>, mut commands: Commands| {
                if click.button == PointerButton::Primary {
                    commands.run_system_cached_with(start_chapter, number);
                }
            },
        );
    }
    spawn_button(&mut commands, panel, "Frei weiterspielen").observe(|click: On<PointerClick>, mut commands: Commands| {
        if click.button == PointerButton::Primary {
            commands.run_system_cached(play_on);
        }
    });
}

pub fn play_on(world: &mut World) {
    world.resource_mut::<Scenario>().stage = Stage::Free;
    world.run_system_cached(start_playing).ok();
}
