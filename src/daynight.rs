//! Tageszeit: Morgen, Tag, Abend, Nacht. Nachts schlafen die Kaninchen, in der Dämmerung kommt der Fuchs.
//! Sonne und Himmelslicht folgen der Tageszeit. Dazu die Jahreszeiten: Im Winter wächst nichts,
//! dafür lohnt sich der Vorrat.

use bevy::prelude::*;

use crate::colony::Messages;
use crate::map::{Layer, ViewLayer};

/// Länge eines ganzen Tages in Spielsekunden.
pub const DAY_LENGTH: f32 = 240.0;
/// Ein Spieljahr: 6 Tage, bei 1× also 24 Minuten.
pub const DAYS_PER_YEAR: u32 = 6;
/// Ein Spieljahr in Spielsekunden, z. B. für das Alter der Kaninchen.
pub const YEAR_LENGTH: f32 = DAY_LENGTH * DAYS_PER_YEAR as f32;
const NIGHT_DARKNESS: f32 = 0.45;
/// Die milden Stunden eines Wintertags, als Anteil des Tages.
const MILD_FROM: f32 = 0.28;
const MILD_UNTIL: f32 = 0.55;
const SUN_DAY: f32 = 2400.0;
const SUN_NIGHT: f32 = 700.0;
const AMBIENT_DAY: f32 = 330.0;
const AMBIENT_NIGHT: f32 = 220.0;
const SKY_DAY: [f32; 3] = [0.98, 0.95, 0.86];
const SKY_NIGHT: [f32; 3] = [0.10, 0.12, 0.22];

pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DayClock>()
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: AMBIENT_DAY,
                affects_lightmapped_meshes: true,
            })
            .add_systems(Update, (advance_clock, update_lights).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Morning,
    Day,
    Evening,
    Night,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Morning => "Morgen",
            Phase::Day => "Tag",
            Phase::Evening => "Abend",
            Phase::Night => "Nacht",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn label(self) -> &'static str {
        match self {
            Season::Spring => "Frühling",
            Season::Summer => "Sommer",
            Season::Autumn => "Herbst",
            Season::Winter => "Winter",
        }
    }

    /// Meldung, wenn diese Jahreszeit beginnt.
    fn announcement(self) -> &'static str {
        match self {
            Season::Spring => "Frühling! Das Gras wächst wieder, und es gibt Nachwuchs.",
            Season::Summer => "Sommer: Die Wiese steht in vollem Saft.",
            Season::Autumn => "Herbst: Das Gras wächst langsamer. Zeit, Vorräte anzulegen!",
            Season::Winter => "Der Winter kommt: Das Gras erfriert. Jetzt hilft nur noch der Vorrat.",
        }
    }
}

#[derive(Resource)]
pub struct DayClock {
    pub day: u32,
    /// Anteil des Tages, 0 = Sonnenaufgang.
    pub time: f32,
}

impl Default for DayClock {
    /// Ein Spiel beginnt am Vormittag des ersten Frühlingstags.
    fn default() -> Self {
        Self { day: 1, time: 0.2 }
    }
}

impl DayClock {
    pub fn year(&self) -> u32 {
        (self.day - 1) / DAYS_PER_YEAR + 1
    }

    /// Tag innerhalb des Jahres, ab 1.
    pub fn day_of_year(&self) -> u32 {
        (self.day - 1) % DAYS_PER_YEAR + 1
    }

    /// Anteil des Jahres, 0 = Frühlingsanfang.
    pub fn year_progress(&self) -> f32 {
        ((self.day_of_year() - 1) as f32 + self.time) / DAYS_PER_YEAR as f32
    }

    pub fn season(&self) -> Season {
        match self.year_progress() {
            p if p < 0.25 => Season::Spring,
            p if p < 0.5 => Season::Summer,
            p if p < 0.75 => Season::Autumn,
            _ => Season::Winter,
        }
    }

    /// Wie weit die aktuelle Jahreszeit fortgeschritten ist, 0 bis 1.
    pub fn season_progress(&self) -> f32 {
        (self.year_progress() * 4.0).fract()
    }

    /// Schneedecke auf der Wiese, 0 bis 1: fällt zu Winterbeginn, taut zum Frühling.
    pub fn snow(&self) -> f32 {
        if self.season() != Season::Winter {
            return 0.0;
        }
        let p = self.season_progress();
        (p / 0.2).min(1.0) * ((1.0 - p) / 0.15).min(1.0)
    }

    pub fn phase(&self) -> Phase {
        match self.time {
            t if t < 0.15 => Phase::Morning,
            t if t < 0.6 => Phase::Day,
            t if t < 0.75 => Phase::Evening,
            _ => Phase::Night,
        }
    }

    pub fn is_night(&self) -> bool {
        self.phase() == Phase::Night
    }

    /// Zeit für das Nest: nachts, und im Winter außer zur milden Mittagszeit.
    pub fn is_resting_time(&self) -> bool {
        self.is_night() || (self.season() == Season::Winter && !self.is_mild_winter_hours())
    }

    /// Mittags im Winter sind die Kaninchen ein paar Stunden wach.
    pub fn is_mild_winter_hours(&self) -> bool {
        self.season() == Season::Winter && (MILD_FROM..MILD_UNTIL).contains(&self.time)
    }

    /// Im Winter ist flache Erde gefroren.
    pub fn is_ground_frozen(&self) -> bool {
        self.season() == Season::Winter
    }

    pub fn is_twilight(&self) -> bool {
        matches!(self.phase(), Phase::Morning | Phase::Evening)
    }

    /// 0 = hell, `NIGHT_DARKNESS` = Nacht; in der Dämmerung fließend.
    pub fn darkness(&self) -> f32 {
        let fade = match self.phase() {
            Phase::Morning => 1.0 - self.time / 0.15,
            Phase::Day => 0.0,
            Phase::Evening => (self.time - 0.6) / 0.15,
            Phase::Night => 1.0,
        };
        fade * NIGHT_DARKNESS
    }
}

fn advance_clock(time: Res<Time>, mut clock: ResMut<DayClock>, mut messages: ResMut<Messages>) {
    let before = clock.phase();
    let season = clock.season();
    clock.time += time.delta_secs() / DAY_LENGTH;
    if clock.time >= 1.0 {
        clock.time -= 1.0;
        clock.day += 1;
    }
    if clock.season() != season {
        messages.push(clock.season().announcement());
    }
    let after = clock.phase();
    if before != after {
        match after {
            Phase::Morning if clock.day_of_year() == 1 => {
                messages.push(format!("Ein neues Jahr beginnt: Jahr {}.", clock.year()))
            }
            Phase::Morning => messages.push(format!("Tag {} beginnt.", clock.day_of_year())),
            Phase::Night => messages.push("Es wird Nacht. Die Kaninchen gehen schlafen."),
            _ => {}
        }
    }
}

/// Helligkeit für die unbeleuchtet gemalten Figuren: 1 am Tag, dunkler in der Nacht.
/// Unter der Erde ist es immer etwas gedämpft, aber die Nacht ändert dort nichts.
pub fn figure_light(clock: &DayClock, layer: Layer) -> f32 {
    match layer {
        Layer::Surface => 1.0 - clock.darkness() * 0.9,
        Layer::Underground => 0.85,
    }
}

/// Sonne und Himmelslicht folgen der Tageszeit; im Untergrund bleibt es gleichmäßig.
fn update_lights(
    clock: Res<DayClock>,
    view: Res<ViewLayer>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sky: ResMut<ClearColor>,
    mut suns: Query<&mut DirectionalLight>,
) {
    if !clock.is_changed() && !view.is_changed() {
        return;
    }
    let night = if view.0 == Layer::Surface {
        clock.darkness() / NIGHT_DARKNESS
    } else {
        0.0
    };
    // Ohne Tonemapping so bemessen, dass der Boden am Tag etwa seine gemalte Farbe zeigt.
    for mut sun in &mut suns {
        sun.illuminance = SUN_DAY - (SUN_DAY - SUN_NIGHT) * night;
        sun.color = Color::srgb(1.0, 0.97 - 0.2 * night, 0.92 - 0.1 * night);
    }
    ambient.brightness = AMBIENT_DAY - (AMBIENT_DAY - AMBIENT_NIGHT) * night;
    ambient.color = Color::srgb(1.0 - 0.45 * night, 1.0 - 0.35 * night, 1.0);
    let [r, g, b] = [0, 1, 2].map(|i| SKY_DAY[i] + (SKY_NIGHT[i] - SKY_DAY[i]) * night);
    sky.0 = Color::srgb(r, g, b);
}
