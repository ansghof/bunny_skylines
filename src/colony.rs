//! Gemeinsame Daten der Kolonie: Vorrat, Kapazitäten, Meldungen und ein einfacher Zufallsgenerator.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::map::{Chamber, Grid};

/// Plätze pro Feld Schlafkammer.
pub const SLEEP_PER_TILE: usize = 2;
/// Futtereinheiten pro Feld Vorratskammer.
pub const FOOD_PER_STORAGE_TILE: f32 = 10.0;

const MAX_MESSAGES: usize = 5;

pub struct ColonyPlugin;

impl Plugin for ColonyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Colony { food: 0.0 })
            .insert_resource(Rng(0x5EED_CAFE_F00D_1234))
            .init_resource::<Messages>();
    }
}

#[derive(Resource)]
pub struct Colony {
    pub food: f32,
}

impl Colony {
    pub fn food_capacity(grid: &Grid) -> f32 {
        grid.chamber_tiles(Chamber::Storage) as f32 * FOOD_PER_STORAGE_TILE
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
