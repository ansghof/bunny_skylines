//! Kampagne: Jedes Kapitel erzählt, wohin die Familie Buddelberg zieht, und setzt Ziele.
//! Sind alle erreicht, ist das Kapitel geschafft; danach geht es frei weiter.
//! Bisher gibt es nur das erste Kapitel, „Die Hecke“.

use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::audio::{Sound, Sounds};
use crate::colony::Colony;
use crate::daynight::DayClock;
use crate::ending::{restart, spawn_button, spawn_overlay};
use crate::rabbit::Rabbit;
use crate::save::{load_game, save_exists};

pub struct ScenarioPlugin;

impl Plugin for ScenarioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Scenario>()
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
}

impl Target {
    fn label(self) -> String {
        match self {
            Target::Rabbits(n) => format!("{n} Kaninchen"),
            Target::Food(n) => format!("{n:.0} Futter im Vorrat"),
        }
    }

    /// Wie weit die Familie ist, und ob es reicht.
    fn progress(self, rabbits: usize, food: f32) -> (f32, f32) {
        match self {
            Target::Rabbits(n) => (rabbits as f32, n as f32),
            Target::Food(n) => (food.floor(), n),
        }
    }
}

pub struct Chapter {
    pub number: u32,
    pub title: &'static str,
    pub story: &'static str,
    pub targets: &'static [Target],
}

const THE_HEDGE: Chapter = Chapter {
    number: 1,
    title: "Die Hecke",
    story: "Der Bauer hat die alte Wiese umgepflügt. Schneeflocke und Lukas haben ein paar \
            Grashalme gerettet und ein neues Plätzchen gefunden: eine sonnige Wiese an einer Hecke.\n\n\
            Grabt Gänge und Kammern, sammelt Gras und macht die Wiese zum Zuhause \
            für eine richtige Familie.",
    targets: &[Target::Rabbits(10), Target::Food(30.0)],
};

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Stage {
    /// Die Einleitung ist offen, das Spiel wartet.
    Intro,
    Playing,
    /// Alle Ziele erreicht, der Siegbildschirm ist offen.
    Won,
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

impl Scenario {
    /// Neubeginn nach dem Aussterben: dasselbe Kapitel, aber ohne erneute Einleitung.
    pub fn restart(&mut self) {
        self.stage = Stage::Playing;
    }

    /// Zeile für die Statusanzeige, z. B. „Ziele: 10 Kaninchen (4/10) · 30 Futter im Vorrat (8/30)“.
    pub fn goal_line(&self, rabbits: usize, food: f32) -> Option<String> {
        if self.stage == Stage::Free {
            return None;
        }
        let goals: Vec<String> = self
            .chapter
            .targets
            .iter()
            .map(|target| {
                let (have, need) = target.progress(rabbits, food);
                if have >= need {
                    format!("{} (geschafft)", target.label())
                } else {
                    format!("{} ({have:.0}/{need:.0})", target.label())
                }
            })
            .collect();
        Some(format!("Kapitel {} „{}“: {}", self.chapter.number, self.chapter.title, goals.join(" · ")))
    }

    fn reached(&self, rabbits: usize, food: f32) -> bool {
        self.chapter.targets.iter().all(|target| {
            let (have, need) = target.progress(rabbits, food);
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
    let goals: Vec<String> = chapter.targets.iter().map(|t| format!("• {}", t.label())).collect();
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


/// Schließt die Einleitung und lässt die Zeit laufen.
pub fn start_playing(
    mut commands: Commands,
    mut scenario: ResMut<Scenario>,
    mut time: ResMut<Time<Virtual>>,
    screens: Query<Entity, With<crate::ending::EndScreen>>,
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
    mut sounds: ResMut<Sounds>,
    rabbits: Query<(), With<Rabbit>>,
) {
    let count = rabbits.iter().count();
    if scenario.stage != Stage::Playing || !scenario.reached(count, colony.food) {
        return;
    }
    scenario.stage = Stage::Won;
    time.pause();
    sounds.play(Sound::Victory);
    let chapter = scenario.chapter;
    let body = format!(
        "Die Familie Buddelberg hat sich an der {} eingelebt: {count} Kaninchen und {:.0} Futter im Vorrat, \
         im {} von Jahr {}.\n\nDie nächsten Kapitel folgen bald. Bis dahin kannst du hier frei weiterbauen.",
        chapter.title.trim_start_matches("Die "),
        colony.food,
        clock.season().label(),
        clock.year(),
    );
    let panel = spawn_overlay(&mut commands, &format!("Geschafft! Kapitel {}: {}", chapter.number, chapter.title), &body);
    spawn_button(&mut commands, panel, "Frei weiterspielen").observe(|click: On<PointerClick>, mut commands: Commands| {
        if click.button == PointerButton::Primary {
            commands.run_system_cached(play_on);
        }
    });
    spawn_button(&mut commands, panel, "Neu beginnen").observe(|click: On<PointerClick>, mut commands: Commands| {
        if click.button == PointerButton::Primary {
            commands.run_system_cached(restart);
        }
    });
}

pub fn play_on(world: &mut World) {
    world.resource_mut::<Scenario>().stage = Stage::Free;
    world.run_system_cached(start_playing).ok();
}
