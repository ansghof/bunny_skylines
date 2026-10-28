//! Minka, die Hofkatze. Sie kommt ab und zu vom Bauernhof herüber. Gibt es einen Kräutergarten,
//! wälzt sie sich dort in der Katzenminze; nach ein paar Besuchen ist sie Freundin der Familie.
//! Bis dahin jagt sie neugierig die Jungen herum (sie erwischt keins, aber sie erschrecken).
//!
//! Als Freundin bleibt sie: Tagsüber döst sie am Sonnenplatz oder an einem Eingang, und Kaninchen
//! in ihrer Nähe sind zufriedener und beruhigen sich schneller. Nachts schläft sie an einem Eingang;
//! dort traut sich kein Marder hinein. Und sie hält die Wühlmäuse vom Vorrat fern, die sonst nachts
//! an einem großen Vorrat knabbern.

use std::f32::consts::TAU;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::behavior::{BehaviorSet, Fear, Path, Reserved, start_flight};
use crate::behavior::Activity;
use crate::bird::Sky;
use crate::camera::Orbit;
use crate::colony::{Colony, Messages, Rng};
use crate::daynight::{DayClock, figure_light};
use crate::map::{Feature, Grid, Layer, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, TilePos, ViewLayer};
use crate::rabbit::{Mood, OnLayer, Rabbit};
use crate::rig::{Avatar, AvatarLink, Part, RigRoot, RigSource, pose_part};

const RIG_PATH: &str = "figuren/katze/katze.rig.ron";
const RIG_FOLDER: &str = "figuren/katze";
pub const CAT_NAME: &str = "Minka";
const WALK_SPEED: f32 = 80.0;
/// Spielsekunden zwischen zwei Besuchen, solange sie noch keine Freundin ist.
const VISIT_EVERY: (f32, f32) = (150.0, 260.0);
/// So lange streift sie bei einem Besuch über die Wiese.
const VISIT_SECONDS: f32 = 40.0;
/// So lange wälzt sie sich in der Katzenminze.
const ROLL_SECONDS: f32 = 8.0;
/// Nach drei Besuchen im Kräutergarten ist sie Freundin.
const FRIENDSHIP_PER_ROLL: f32 = 0.34;
/// So nah muss sie einem Jungen kommen, damit es erschrickt.
const CHASE_REACH: f32 = TILE_SIZE * 1.5;
/// Wie sehr ein gejagtes Junges sich erschreckt.
const CHASE_MOOD: f32 = 0.1;
/// So weit wirkt ihre Ruhe, wenn sie döst.
pub const CALM_REACH: f32 = TILE_SIZE * 3.0;
/// In ihrer Nähe legt sich die Angst so viel schneller.
const CALM_FEAR_FACTOR: f32 = 2.0;
/// Ab so viel Futter im Vorrat kommen nachts Wühlmäuse.
const VOLE_FOOD: f32 = 15.0;
/// Wie wahrscheinlich sie kommen: Futter durch diese Zahl, höchstens `VOLE_CHANCE_MAX`.
const VOLE_FOOD_PER_CHANCE: f32 = 80.0;
const VOLE_CHANCE_MAX: f32 = 0.6;
/// So viel vom Vorrat fressen sie, mindestens `VOLE_MIN_LOSS`.
const VOLE_SHARE: f32 = 0.08;
const VOLE_MIN_LOSS: f32 = 2.0;

pub struct CatPlugin;

impl Plugin for CatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CatBond>()
            .init_resource::<CatRest>()
            .add_systems(Update, (cat_visits, cat_moves, cat_calms, voles).chain().after(BehaviorSet))
            .add_systems(Update, animate_cat.after(cat_moves));
    }
}

/// Wie gut Minka die Familie kennt; wird gespeichert.
#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct CatBond {
    pub friendship: f32,
    pub friend: bool,
    #[serde(skip, default = "first_visit")]
    next_visit: f32,
}

fn first_visit() -> f32 {
    VISIT_EVERY.0
}

impl CatBond {
    /// Nur für Tests: gleich vorbeikommen.
    pub fn summon(&mut self) {
        self.next_visit = 1.0;
    }
}

impl Default for CatBond {
    fn default() -> Self {
        Self { friendship: 0.0, friend: false, next_visit: first_visit() }
    }
}

/// Wo Minka gerade döst und an welchem Eingang sie nachts liegt.
#[derive(Resource, Default)]
pub struct CatRest {
    pub dozing: Option<Vec2>,
    pub guarded_entrance: Option<TilePos>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    /// Zu Besuch: auf dem Weg zum Kräutergarten oder hinter den Jungen her.
    Visiting { left: f32 },
    /// Wälzt sich in der Katzenminze.
    Rolling { left: f32 },
    Leaving,
    /// Als Freundin: auf dem Weg zu ihrem Ruheplatz oder dort.
    Resting,
}

#[derive(Component)]
pub struct Cat {
    mode: Mode,
    velocity: Vec2,
    anim_phase: f32,
    /// Liegt sie gerade an ihrem Platz?
    settled: bool,
}

fn half_extent() -> Vec2 {
    Vec2::new(MAP_WIDTH as f32, MAP_HEIGHT as f32) * TILE_SIZE / 2.0
}

fn random_edge(rng: &mut Rng) -> Vec2 {
    let half = half_extent() - TILE_SIZE * 0.5;
    let along = rng.range(-1.0, 1.0);
    match rng.below(4) {
        0 => Vec2::new(-half.x, along * half.y),
        1 => Vec2::new(half.x, along * half.y),
        2 => Vec2::new(along * half.x, -half.y),
        _ => Vec2::new(along * half.x, half.y),
    }
}

fn nearest_edge(position: Vec2) -> Vec2 {
    let half = half_extent() + TILE_SIZE;
    [
        Vec2::new(-half.x, position.y),
        Vec2::new(half.x, position.y),
        Vec2::new(position.x, -half.y),
        Vec2::new(position.x, half.y),
    ]
    .into_iter()
    .min_by(|a, b| a.distance(position).total_cmp(&b.distance(position)))
    .unwrap()
}

fn nearest_tile(from: Vec2, wanted: impl Fn(TilePos) -> bool) -> Option<TilePos> {
    Grid::positions()
        .filter(|p| wanted(*p))
        .min_by(|a, b| a.center().distance(from).total_cmp(&b.center().distance(from)))
}

/// Besuche, solange sie noch keine Freundin ist; als Freundin kommt sie einmal und bleibt.
fn cat_visits(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<DayClock>,
    grid: Res<Grid>,
    asset_server: Res<AssetServer>,
    mut bond: ResMut<CatBond>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    cats: Query<(), With<Cat>>,
    rabbits: Query<(), With<Rabbit>>,
) {
    if !cats.is_empty() || rabbits.is_empty() {
        return;
    }
    let mode = if bond.friend {
        Mode::Resting
    } else {
        if clock.is_night() {
            return;
        }
        bond.next_visit -= time.delta_secs();
        if bond.next_visit > 0.0 {
            return;
        }
        bond.next_visit = rng.range(VISIT_EVERY.0, VISIT_EVERY.1);
        messages.push(if bond.friendship > 0.0 {
            format!("{CAT_NAME}, die Hofkatze, schaut wieder vorbei.")
        } else {
            format!("{CAT_NAME}, die Katze vom Bauernhof, streift über die Wiese. Sie ist neugierig auf die Jungen. Katzenminze aus dem Kräutergarten mag sie gern.")
        });
        Mode::Visiting { left: VISIT_SECONDS }
    };
    // Als Freundin gehört sie zum Bau: Sie taucht gleich an einem Eingang auf, nicht erst am Rand.
    let home = nearest_tile(Vec2::ZERO, |p| grid.surface(p).entrance).filter(|_| bond.friend);
    let position = home.map_or_else(|| random_edge(&mut rng), |p| p.center());
    commands.spawn((
        Sky,
        Cat { mode, velocity: Vec2::ZERO, anim_phase: 0.0, settled: false },
        RigSource { rig: asset_server.load(RIG_PATH), folder: RIG_FOLDER },
        Transform::from_translation(position.extend(0.0)),
        Visibility::default(),
        OnLayer(Layer::Surface),
    ));
}

/// Wohin sie läuft und was sie dort tut.
#[allow(clippy::too_many_arguments)]
fn cat_moves(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<DayClock>,
    grid: Res<Grid>,
    mut bond: ResMut<CatBond>,
    mut rest: ResMut<CatRest>,
    mut messages: ResMut<Messages>,
    mut reserved: ResMut<Reserved>,
    mut cats: Query<(Entity, &mut Cat, &mut Transform), Without<Rabbit>>,
    mut rabbits: Query<(&Rabbit, &Transform, &OnLayer, &mut Activity, &mut Path, &mut Fear, &mut Mood), Without<Cat>>,
) {
    let dt = time.delta_secs();
    let mut resting_at = None;
    let mut guarding = None;
    for (entity, mut cat, mut transform) in &mut cats {
        let position = transform.translation.truncate();
        let herbs = |p: TilePos| grid.owns(p) && grid.surface(p).feature == Some(Feature::Herbs);
        let target = match cat.mode {
            Mode::Visiting { left } => {
                let left = left - dt;
                cat.mode = if left <= 0.0 { Mode::Leaving } else { Mode::Visiting { left } };
                if let Some(garden) = nearest_tile(position, herbs) {
                    if garden.center().distance(position) < 6.0 {
                        cat.mode = Mode::Rolling { left: ROLL_SECONDS };
                        messages.push(format!("{CAT_NAME} wälzt sich schnurrend in der Katzenminze."));
                    }
                    Some(garden.center())
                } else {
                    // Kein Kräutergarten: hinter dem nächsten Jungen draußen her.
                    let young = rabbits
                        .iter()
                        .filter(|(r, _, layer, ..)| !r.is_adult() && layer.0 == Layer::Surface)
                        .map(|(_, t, ..)| t.translation.truncate())
                        .min_by(|a, b| a.distance(position).total_cmp(&b.distance(position)));
                    for (rabbit, at, layer, mut activity, mut path, mut fear, mut mood) in &mut rabbits {
                        let close = at.translation.truncate().distance(position) < CHASE_REACH;
                        if !rabbit.is_adult() && layer.0 == Layer::Surface && close && fear.0 <= 0.0 {
                            start_flight(&grid, &mut reserved, at, layer, &mut activity, &mut path, &mut fear);
                            mood.value = (mood.value - CHASE_MOOD).max(0.0);
                        }
                    }
                    young.or_else(|| nearest_tile(position, |p| grid.surface(p).entrance).map(|p| p.center()))
                }
            }
            Mode::Rolling { left } => {
                let left = left - dt;
                if left > 0.0 {
                    cat.mode = Mode::Rolling { left };
                } else {
                    bond.friendship = (bond.friendship + FRIENDSHIP_PER_ROLL).min(1.0);
                    if bond.friendship >= 1.0 && !bond.friend {
                        bond.friend = true;
                        messages.push(format!(
                            "{CAT_NAME} mag euch jetzt! Sie bleibt: Tagsüber döst sie am Sonnenplatz, nachts schläft sie an einem Eingang und hält Marder und Wühlmäuse fern."
                        ));
                        cat.mode = Mode::Resting;
                    } else {
                        cat.mode = Mode::Leaving;
                    }
                }
                None
            }
            Mode::Leaving => {
                let exit = nearest_edge(position);
                if exit.distance(position) < 6.0 {
                    commands.entity(entity).despawn();
                    continue;
                }
                Some(exit)
            }
            Mode::Resting => {
                // Nachts an einem Eingang, am liebsten dem beim Sonnenplatz; tagsüber am Sonnenplatz.
                let sun = nearest_tile(position, |p| grid.owns(p) && grid.surface(p).feature == Some(Feature::SunSpot));
                let home = sun.map_or(position, |p| p.center());
                let entrance = nearest_tile(home, |p| grid.surface(p).entrance);
                let spot = if clock.is_night() {
                    entrance.map(|p| p.center())
                } else {
                    // Neben dem Eingang, nicht davor, damit sie keinem im Weg liegt.
                    sun.map(|p| p.center()).or(entrance.map(|p| p.center() + Vec2::new(TILE_SIZE * 0.6, 0.0)))
                };
                if let Some(spot) = spot.filter(|spot| spot.distance(position) < 6.0) {
                    resting_at = Some(spot);
                    if clock.is_night() {
                        guarding = entrance;
                    }
                }
                spot
            }
        };
        let step = target.map_or(Vec2::ZERO, |goal| (goal - position).clamp_length_max(WALK_SPEED * dt));
        cat.velocity = step / dt.max(1e-4);
        cat.settled = matches!(cat.mode, Mode::Rolling { .. }) || resting_at.is_some();
        transform.translation += step.extend(0.0);
    }
    if rest.dozing != resting_at {
        rest.dozing = resting_at;
    }
    if rest.guarded_entrance != guarding {
        rest.guarded_entrance = guarding;
    }
}

/// In ihrer Nähe legt sich die Angst schneller.
fn cat_calms(
    time: Res<Time>,
    rest: Res<CatRest>,
    mut rabbits: Query<(&Transform, &OnLayer, &mut Fear), With<Rabbit>>,
) {
    let Some(spot) = rest.dozing else {
        return;
    };
    let dt = time.delta_secs();
    for (transform, layer, mut fear) in &mut rabbits {
        if layer.0 == Layer::Surface && fear.0 > 0.0 && transform.translation.truncate().distance(spot) < CALM_REACH {
            fear.0 = (fear.0 - dt * (CALM_FEAR_FACTOR - 1.0)).max(0.0);
        }
    }
}

/// Wie wahrscheinlich in einer Nacht Wühlmäuse kommen: je größer der Vorrat, desto eher.
fn vole_chance(food: f32) -> f32 {
    if food < VOLE_FOOD { 0.0 } else { (food / VOLE_FOOD_PER_CHANCE).min(VOLE_CHANCE_MAX) }
}

/// Wie viel sie dann fressen.
fn vole_loss(food: f32) -> f32 {
    (food * VOLE_SHARE).max(VOLE_MIN_LOSS).min(food)
}

/// Bei Einbruch der Nacht kommen an einen großen Vorrat manchmal Wühlmäuse, außer Minka ist da.
fn voles(
    clock: Res<DayClock>,
    bond: Res<CatBond>,
    mut colony: ResMut<Colony>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
    mut was_night: Local<bool>,
) {
    let night = clock.is_night();
    let nightfall = night && !*was_night;
    *was_night = night;
    if !nightfall || colony.food < VOLE_FOOD {
        return;
    }
    if !rng.chance(vole_chance(colony.food)) {
        return;
    }
    if bond.friend {
        messages.push(format!("{CAT_NAME} hat in der Nacht Wühlmäuse vom Vorrat verjagt."));
        return;
    }
    let loss = vole_loss(colony.food);
    colony.food -= loss;
    messages.push(format!(
        "Wühlmäuse haben nachts am Vorrat geknabbert: {loss:.0} Futter weg. Eine Katze würde sie verjagen."
    ));
}

/// Läuft im Trab, wälzt sich und liegt zusammengerollt mit untergeschlagenen Pfoten.
#[allow(clippy::too_many_arguments)]
fn animate_cat(
    time: Res<Time>,
    view: Res<ViewLayer>,
    orbit: Res<Orbit>,
    clock: Res<DayClock>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cats: Query<(&mut Cat, &AvatarLink)>,
    avatars: Query<&Children, With<Avatar>>,
    mut roots: Query<(&RigRoot, &mut Transform, &Children), (Without<Cat>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Transform), (Without<RigRoot>, Without<Cat>)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let light = figure_light(&clock, Layer::Surface);
    let alpha = if view.0 == Layer::Underground { 0.35 } else { 1.0 };
    for (mut cat, link) in &mut cats {
        let Ok(children) = avatars.get(link.0) else {
            continue;
        };
        let speed = cat.velocity.length();
        let moving = speed > 1.0;
        cat.anim_phase = (cat.anim_phase + dt * speed / 50.0).fract();
        let swing = (cat.anim_phase * TAU).sin();
        let rolling = matches!(cat.mode, Mode::Rolling { .. });
        let lying = cat.settled && !moving;
        let sideways = cat.velocity.dot(orbit.screen_right());
        for child in children.iter() {
            let Ok((root, mut root_transform, root_children)) = roots.get_mut(child) else {
                continue;
            };
            if sideways.abs() > 0.1 {
                root_transform.scale.x = sideways.signum() * root.pixel_scale;
            } else if root_transform.scale.x == 0.0 {
                root_transform.scale.x = root.pixel_scale;
            }
            root_transform.scale.y = root.pixel_scale;
            root_transform.scale.z = root.pixel_scale;
            // Liegend sinkt der Körper auf die untergeschlagenen Pfoten.
            let low = if lying { -9.0 } else if moving { swing.abs() * 1.5 } else { 0.0 };
            root_transform.translation = Vec3::new(0.0, low, 0.0);
            root_transform.rotation = orbit.billboard() * Quat::from_rotation_z(if rolling { (t * 3.0).sin() * 0.25 } else { 0.0 });
            for part_entity in root_children.iter() {
                let Ok((part, mut part_transform)) = parts.get_mut(part_entity) else {
                    continue;
                };
                let stride = if moving { 26.0 } else { 0.0 };
                let tuck = if lying { 80.0 } else { 0.0 };
                let angle = match part.name.as_str() {
                    "bein_hinten_nah" | "bein_vorne_fern" => swing * stride + tuck,
                    "bein_vorne_nah" | "bein_hinten_fern" => -swing * stride + tuck,
                    // Der Schwanz zuckt gemächlich; liegend legt er sich um sie.
                    "schwanz" if lying => 30.0 + (t * 1.5).sin() * 5.0,
                    "schwanz" => (t * 2.0).sin() * 8.0,
                    _ => 0.0,
                };
                let variant = (part.name == "auge" && lying).then_some("zu");
                pose_part(part, &mut part_transform, &mut materials, Vec2::ZERO, angle, variant, Color::WHITE.with_alpha(alpha), light);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voles_come_more_often_to_a_bigger_store_and_take_a_share() {
        assert_eq!(vole_chance(10.0), 0.0, "ein kleiner Vorrat lockt keine");
        assert!(vole_chance(20.0) < vole_chance(40.0));
        assert_eq!(vole_chance(500.0), VOLE_CHANCE_MAX);
        assert_eq!(vole_loss(20.0), VOLE_MIN_LOSS);
        assert_eq!(vole_loss(100.0), 100.0 * VOLE_SHARE);
    }

    #[test]
    fn three_catnip_visits_make_a_friend() {
        let visits = (1.0 / FRIENDSHIP_PER_ROLL).ceil() as u32;
        assert_eq!(visits, 3);
    }
}
