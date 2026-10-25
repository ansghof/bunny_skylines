//! Tageszeit: Morgen, Tag, Abend, Nacht. Nachts schlafen die Kaninchen, in der Dämmerung kommt der Fuchs.

use bevy::prelude::*;

use crate::colony::Messages;
use crate::map::{Layer, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, ViewLayer};

/// Länge eines ganzen Tages in Spielsekunden.
pub const DAY_LENGTH: f32 = 240.0;
const NIGHT_DARKNESS: f32 = 0.45;

pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DayClock { day: 1, time: 0.2 })
            .add_systems(Startup, spawn_night_overlay)
            .add_systems(Update, (advance_clock, update_night_overlay).chain());
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
    fn darkness(&self) -> f32 {
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

#[derive(Component)]
struct NightOverlay;

fn spawn_night_overlay(mut commands: Commands) {
    // Etwas größer als die Karte, damit beim Verschieben keine hellen Ränder entstehen.
    let size = Vec2::new(MAP_WIDTH as f32, MAP_HEIGHT as f32) * TILE_SIZE * 3.0;
    commands.spawn((
        NightOverlay,
        Sprite::from_color(Color::srgba(0.05, 0.07, 0.2, 0.0), size),
        Transform::from_xyz(0.0, 0.0, 30.0),
    ));
}

fn update_night_overlay(
    clock: Res<DayClock>,
    view: Res<ViewLayer>,
    mut overlay: Single<&mut Sprite, With<NightOverlay>>,
) {
    // Unter der Erde ist es ohnehin dunkel; dort blendet die Nacht nicht zusätzlich ab.
    let alpha = if view.0 == Layer::Surface { clock.darkness() } else { 0.0 };
    overlay.color.set_alpha(alpha);
}
