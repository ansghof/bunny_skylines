//! Spielgeschwindigkeit: Leertaste pausiert, 1–3 wählt das Tempo.
//! Alle Spielsysteme lesen `Time` (virtuelle Zeit) und laufen dadurch automatisch mit.

use bevy::prelude::*;

use crate::ending::EndScreen;

pub struct SpeedPlugin;

impl Plugin for SpeedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, speed_keys);
    }
}

fn speed_keys(keys: Res<ButtonInput<KeyCode>>, mut time: ResMut<Time<Virtual>>, screens: Query<(), With<EndScreen>>) {
    // Solange Einleitung, Sieg oder Ende offen sind, steht das Spiel still.
    if !screens.is_empty() {
        return;
    }
    if keys.just_pressed(KeyCode::Space) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
    for (key, speed) in [(KeyCode::Digit1, 1.0), (KeyCode::Digit2, 2.0), (KeyCode::Digit3, 3.0)] {
        if keys.just_pressed(key) {
            time.set_relative_speed(speed);
            time.unpause();
        }
    }
}

/// Text für die Anzeige, z. B. „2×“ oder „Pause“.
pub fn label(time: &Time<Virtual>) -> String {
    if time.is_paused() {
        "Pause".into()
    } else {
        format!("{}×", time.relative_speed())
    }
}
