//! Gemeinsame Daten der Kolonie: Vorrat, Kapazitäten, Meldungen und ein einfacher Zufallsgenerator.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::map::{Chamber, Grid};

/// Plätze pro Feld Schlafkammer.
pub const SLEEP_PER_TILE: usize = 2;
/// Futtereinheiten pro Feld Vorratskammer.
pub const FOOD_PER_STORAGE_TILE: f32 = 10.0;

/// Was das Bauen kostet. Rohstoffe sind die Währung: Futter (Gras, auch fürs Nest) und Erde.
pub mod cost {
    /// Saatmöhren aus dem Vorrat.
    pub const CARROTS_FOOD: f32 = 3.0;
    /// Ein Festplatz wird aus Erde aufgeschüttet.
    pub const FESTIVAL_EARTH: f32 = 4.0;
    /// Ein Erdpfropf braucht Erde zum Verschließen.
    pub const PLUG_EARTH: f32 = 2.0;
    /// Ein Platz für die Nachtwache.
    pub const WATCH_EARTH: f32 = 1.0;
    /// Ein Wachposten ist ein Erdhügel.
    pub const POST_EARTH: f32 = 5.0;
    /// Ein Nest pro Feld Schlafkammer, aus eingetragenem Gras.
    pub const NEST_FOOD: f32 = 2.0;
    /// Saatgut für ein Feld Klee.
    pub const CLOVER_FOOD: f32 = 1.0;
    /// Saatgut für ein Feld Kräuter.
    pub const HERBS_FOOD: f32 = 2.0;
    /// Bauten aus Erde an der Oberfläche.
    pub const HEDGE_EARTH: f32 = 3.0;
    pub const BOLT_HOLE_EARTH: f32 = 1.0;
    pub const SUN_SPOT_EARTH: f32 = 2.0;
    pub const PLAYGROUND_EARTH: f32 = 3.0;
    /// Erde, die beim Graben eines Feldes anfällt.
    pub const EARTH_PER_TILE: f32 = 1.0;
}

const START_FOOD: f32 = 8.0;

const MAX_MESSAGES: usize = 5;

pub struct ColonyPlugin;

impl Plugin for ColonyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Colony>()
            .insert_resource(Rng(0x5EED_CAFE_F00D_1234))
            .init_resource::<Messages>();
    }
}

/// Die Rohstoffe der Kolonie.
#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct Colony {
    /// Futter im Vorrat; wird gegessen und für Nester verbraucht.
    pub food: f32,
    /// Erde aus dem Graben; Baumaterial für Hügel und Wälle.
    pub earth: f32,
    /// Futtereinheiten pro Feld Vorratskammer; mit getrocknetem Heu mehr.
    pub storage_per_tile: f32,
}

impl Default for Colony {
    fn default() -> Self {
        Self {
            food: START_FOOD,
            earth: 0.0,
            storage_per_tile: FOOD_PER_STORAGE_TILE,
        }
    }
}

impl Colony {
    /// Zieht den Preis ab, wenn genug da ist. Sonst bleibt alles, wie es ist.
    pub fn pay(&mut self, food: f32, earth: f32) -> bool {
        if self.food < food || self.earth < earth {
            return false;
        }
        self.food -= food;
        self.earth -= earth;
        true
    }

    pub fn food_capacity(&self, grid: &Grid) -> f32 {
        grid.chamber_tiles(Chamber::Storage) as f32 * self.storage_per_tile
    }

    pub fn sleeping_places(grid: &Grid) -> usize {
        grid.chamber_tiles(Chamber::Sleeping) * SLEEP_PER_TILE
    }
}

/// Die letzten Meldungen für die Anzeige, neueste zuerst.
#[derive(Resource, Default)]
pub struct Messages(pub VecDeque<String>);

impl Messages {
    pub fn push(&mut self, message: impl Into<String>) {
        self.0.push_front(message.into());
        self.0.truncate(MAX_MESSAGES);
    }
}

/// Xorshift: reicht für Spielentscheidungen und kommt ohne zusätzliche Abhängigkeit aus.
#[derive(Resource)]
pub struct Rng(u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Gleichverteilt in 0..1.
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.f32()
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    pub fn chance(&mut self, probability: f32) -> bool {
        self.f32() < probability
    }
}
