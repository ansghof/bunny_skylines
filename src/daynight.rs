//! Tageszeit: Morgen, Tag, Abend, Nacht. Nachts schlafen die Kaninchen, in der Dämmerung kommt der Fuchs.
//! Sonne und Himmelslicht folgen der Tageszeit.

use bevy::prelude::*;

use crate::colony::Messages;
use crate::map::{Layer, ViewLayer};

/// Länge eines ganzen Tages in Spielsekunden.
pub const DAY_LENGTH: f32 = 240.0;
const NIGHT_DARKNESS: f32 = 0.45;
const SUN_DAY: f32 = 2400.0;
const SUN_NIGHT: f32 = 700.0;
const AMBIENT_DAY: f32 = 330.0;
const AMBIENT_NIGHT: f32 = 220.0;
const SKY_DAY: [f32; 3] = [0.98, 0.95, 0.86];
const SKY_NIGHT: [f32; 3] = [0.10, 0.12, 0.22];

pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DayClock { day: 1, time: 0.2 })
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

#[derive(Resource)]
pub struct DayClock {
    pub day: u32,
    /// Anteil des Tages, 0 = Sonnenaufgang.
    pub time: f32,
}

impl DayClock {
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
    clock.time += time.delta_secs() / DAY_LENGTH;
    if clock.time >= 1.0 {
        clock.time -= 1.0;
        clock.day += 1;
    }
    let after = clock.phase();
    if before != after {
        match after {
            Phase::Morning => messages.push(format!("Tag {} beginnt.", clock.day)),
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
