//! Warnen und Flüchten: Wachen entdecken den Fuchs und stampfen, alle in Hörweite rennen zum
//! nächsten Eingang und geben die Warnung im kleinen Umkreis weiter. Dazu der Alarmknopf.

use bevy::prelude::*;

use crate::behavior::{Activity, BehaviorSet, Carrying, Fear, Path, Reserved, start_flight};
use crate::colony::Messages;
use crate::ground::gizmo_circle;
use crate::map::{Grid, Layer, TILE_SIZE};
use crate::predator::Fox;
use crate::rabbit::{OnLayer, Rabbit, Stomping};

/// So weit sieht eine Wache.
const GUARD_SIGHT: f32 = TILE_SIZE * 6.0;
/// So weit hört man das Stampfen einer Wache.
const STOMP_RADIUS: f32 = TILE_SIZE * 7.0;
/// So weit geben flüchtende Kaninchen die Warnung weiter.
const RELAY_RADIUS: f32 = TILE_SIZE * 2.5;
/// Ab dieser Entfernung bemerkt ein Kaninchen den Fuchs selbst.
const NOTICE_RADIUS: f32 = TILE_SIZE * 2.2;
pub const ALARM_COOLDOWN_SECONDS: f32 = 30.0;
const RING_SECONDS: f32 = 0.8;

pub struct DangerPlugin;

impl Plugin for DangerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Alarms>()
            .init_resource::<AlarmButton>()
            .add_systems(
                Update,
                (guards_watch, notice_foxes, alarm_button, spread_alarms, draw_rings)
                    .chain()
                    .after(BehaviorSet),
            );
    }
}

struct Alarm {
    position: Vec2,
    radius: f32,
    /// Ob Gewarnte die Warnung weitergeben.
    relay: bool,
}

#[derive(Resource, Default)]
struct Alarms(Vec<Alarm>);

#[derive(Resource, Default)]
pub struct AlarmButton {
    pub cooldown: f32,
    /// Über die Werkzeugleiste ausgelöst; wirkt wie die Taste F.
    pub requested: bool,
}

/// Sichtbare Bodenwelle eines Stampfers.
#[derive(Component)]
struct StompRing {
    radius: f32,
    age: f32,
}

type FlightQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static OnLayer,
        &'static mut Activity,
        &'static mut Path,
        &'static mut Carrying,
        &'static mut Fear,
    ),
    With<Rabbit>,
>;

fn guards_watch(
    mut commands: Commands,
    grid: Res<Grid>,
    mut reserved: ResMut<Reserved>,
    mut alarms: ResMut<Alarms>,
    mut messages: ResMut<Messages>,
    foxes: Query<&Transform, With<Fox>>,
    mut guards: Query<(Entity, &Rabbit, &Transform, &OnLayer, &mut Activity, &mut Path, &mut Fear)>,
) {
    for (entity, rabbit, transform, layer, mut activity, mut path, mut fear) in &mut guards {
        if !matches!(*activity, Activity::Guarding { .. }) {
            continue;
        }
        let position = transform.translation.truncate();
        // Gute Wachen sehen den Fuchs schon, wenn er noch im Gebüsch lauert.
        let sees_fox = foxes.iter().any(|fox| fox.translation.truncate().distance(position) < GUARD_SIGHT);
        if !sees_fox {
            continue;
        }
        alarms.0.push(Alarm {
            position,
            radius: STOMP_RADIUS,
            relay: true,
        });
        commands.entity(entity).insert(Stomping(0.6));
        messages.push(format!("{} stampft: Fuchs in Sicht!", rabbit.name));
        start_flight(&grid, &mut reserved, transform, layer, &mut activity, &mut path, &mut fear);
    }
}

/// Kommt ein Fuchs zu nahe, flüchtet ein Kaninchen auch ohne Warnung, aber oft zu spät.
fn notice_foxes(
    grid: Res<Grid>,
    mut reserved: ResMut<Reserved>,
    foxes: Query<(&Fox, &Transform)>,
    mut rabbits: FlightQuery,
) {
    let visible: Vec<Vec2> = foxes
        .iter()
        .filter(|(fox, _)| fox.is_out())
        .map(|(_, t)| t.translation.truncate())
        .collect();
    if visible.is_empty() {
        return;
    }
    for (transform, layer, mut activity, mut path, _, mut fear) in &mut rabbits {
        let position = transform.translation.truncate();
        if visible.iter().any(|fox| fox.distance(position) < NOTICE_RADIUS) {
            start_flight(&grid, &mut reserved, transform, layer, &mut activity, &mut path, &mut fear);
        }
    }
}

fn alarm_button(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    grid: Res<Grid>,
    mut button: ResMut<AlarmButton>,
    mut reserved: ResMut<Reserved>,
    mut messages: ResMut<Messages>,
    mut rabbits: FlightQuery,
) {
    button.cooldown = (button.cooldown - time.delta_secs()).max(0.0);
    let pressed = keys.just_pressed(KeyCode::KeyF) || std::mem::take(&mut button.requested);
    if !pressed {
        return;
    }
    if button.cooldown > 0.0 {
        messages.push(format!("Der Alarm ist erst in {:.0} s wieder bereit.", button.cooldown));
        return;
    }
    button.cooldown = ALARM_COOLDOWN_SECONDS;
    messages.push("Alarm! Alle in den Bau!");
    for (transform, layer, mut activity, mut path, mut carrying, mut fear) in &mut rabbits {
        if start_flight(&grid, &mut reserved, transform, layer, &mut activity, &mut path, &mut fear) {
            // In der Eile bleibt getragenes Futter liegen.
            carrying.0 = 0.0;
        }
    }
}

fn spread_alarms(
    mut commands: Commands,
    grid: Res<Grid>,
    mut alarms: ResMut<Alarms>,
    mut reserved: ResMut<Reserved>,
    mut rabbits: FlightQuery,
) {
    // Weitergegebene Warnungen werden im selben Bild weiterverarbeitet.
    while let Some(alarm) = alarms.0.pop() {
        commands.spawn((
            StompRing {
                radius: alarm.radius,
                age: 0.0,
            },
            Transform::from_translation(alarm.position.extend(0.0)),
        ));
        for (transform, layer, mut activity, mut path, _, mut fear) in &mut rabbits {
            let position = transform.translation.truncate();
            if layer.0 != Layer::Surface || position.distance(alarm.position) > alarm.radius {
                continue;
            }
            let fled = start_flight(&grid, &mut reserved, transform, layer, &mut activity, &mut path, &mut fear);
            if fled && alarm.relay {
                alarms.0.push(Alarm {
                    position,
                    radius: RELAY_RADIUS,
                    relay: false,
                });
            }
        }
    }
}

fn draw_rings(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut gizmos: Gizmos,
    mut rings: Query<(Entity, &mut StompRing, &Transform)>,
) {
    for (entity, mut ring, transform) in &mut rings {
        ring.age += time.delta_secs();
        let progress = ring.age / RING_SECONDS;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let color = Color::srgba(0.55, 0.36, 0.2, 0.9 * (1.0 - progress));
        gizmo_circle(&mut gizmos, transform.translation.truncate(), ring.radius * progress, 0.0, color);
    }
}
