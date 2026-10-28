//! Warnen und Flüchten: Wachen entdecken den Fuchs und stampfen, alle in Hörweite rennen zum
//! nächsten Eingang und geben die Warnung im kleinen Umkreis weiter. Dazu der Alarmknopf.

use bevy::prelude::*;

use crate::behavior::{Activity, BehaviorSet, Carrying, Fear, Path, Reserved, release, start_flight};
use crate::audio::{Sound, Sounds};
use crate::colony::Messages;
use crate::ground::gizmo_circle;
use crate::map::{Grid, Layer, TILE_SIZE};
use crate::bird::Buzzard;
use crate::behavior::is_hidden;
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
/// So lange traut sich eine Wache nach einem Fuchs nicht wieder auf den Posten.
const GUARD_FRIGHT_SECONDS: f32 = 12.0;
const RING_SECONDS: f32 = 0.8;

pub struct DangerPlugin;

impl Plugin for DangerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Alarms>()
            .init_resource::<AlarmButton>()
            .add_systems(
                Update,
                (guards_watch, notice_foxes, alarm_button, spread_alarms, keep_inside, draw_rings)
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
pub(crate) struct Alarms(Vec<Alarm>);

/// Eine Wache stampft hier: Alle in diesem Umkreis flüchten und geben die Warnung weiter.
pub(crate) fn raise_alarm(alarms: &mut Alarms, position: Vec2, radius: f32) {
    alarms.0.push(Alarm { position, radius, relay: true });
}

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
    mut foxes: Query<(&mut Fox, &Transform)>,
    mut guards: Query<(Entity, &Rabbit, &Transform, &OnLayer, &mut Activity, &mut Path, &mut Fear)>,
) {
    for (entity, rabbit, transform, layer, mut activity, mut path, mut fear) in &mut guards {
        // Nur Wachen oben halten nach Füchsen Ausschau; die Nachtwache im Bau ist für Marder da.
        if !matches!(*activity, Activity::Guarding { .. }) || layer.0 != Layer::Surface {
            continue;
        }
        let position = transform.translation.truncate();
        // Gute Wachen sehen den Fuchs schon, wenn er noch im Gebüsch lauert.
        let mut sees_fox = false;
        for (mut fox, at) in &mut foxes {
            if at.translation.truncate().distance(position) < GUARD_SIGHT {
                sees_fox = true;
                fox.known = true;
            }
        }
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
        // Kein Weg in den Bau: Die Schicht endet trotzdem, sonst stampft sie jedes Bild aufs Neue.
        if !start_flight(&grid, &mut reserved, transform, layer, &mut activity, &mut path, &mut fear) {
            release(&activity, &mut reserved);
            *activity = Activity::Idle(1.0);
            fear.0 = GUARD_FRIGHT_SECONDS;
        }
    }
}

/// Kommt ein Fuchs zu nahe, flüchtet ein Kaninchen auch ohne Warnung, aber oft zu spät.
fn notice_foxes(
    grid: Res<Grid>,
    mut reserved: ResMut<Reserved>,
    mut foxes: Query<(&mut Fox, &Transform)>,
    mut rabbits: FlightQuery,
) {
    for (mut fox, at) in &mut foxes {
        if !fox.is_out() {
            continue;
        }
        let fox_at = at.translation.truncate();
        for (transform, layer, mut activity, mut path, _, mut fear) in &mut rabbits {
            if layer.0 == Layer::Surface && fox_at.distance(transform.translation.truncate()) < NOTICE_RADIUS {
                start_flight(&grid, &mut reserved, transform, layer, &mut activity, &mut path, &mut fear);
                fox.known = true;
            }
        }
    }
}

/// So lange hält das Wissen um einen Feind noch nach, wenn er schon fort ist.
const KEEP_INSIDE_SECONDS: f32 = 1.0;

/// Solange ein bemerkter Fuchs oder Bussard noch da ist, bleibt im Bau, wer drinnen ist, und im
/// Fluchtloch, wer dort sitzt: Ihre Angst läuft nicht ab. Zieht der letzte Feind ab, gibt es Entwarnung.
fn keep_inside(
    grid: Res<Grid>,
    mut messages: ResMut<Messages>,
    foxes: Query<&Fox>,
    buzzards: Query<&Buzzard>,
    mut rabbits: Query<(&Transform, &OnLayer, &mut Fear), With<Rabbit>>,
    mut before: Local<(bool, bool)>,
) {
    let fox = foxes.iter().any(Fox::is_known_threat);
    let bird = buzzards.iter().any(Buzzard::is_known_threat);
    if before.0 && !fox {
        messages.push("Der Fuchs ist fort. Entwarnung!");
    }
    if before.1 && !bird {
        messages.push("Der Bussard ist fort. Entwarnung!");
    }
    *before = (fox, bird);
    if !fox && !bird {
        return;
    }
    for (transform, layer, mut fear) in &mut rabbits {
        let inside = layer.0 == Layer::Underground || is_hidden(&grid, crate::behavior::current_node(transform, layer));
        if inside && fear.0 < KEEP_INSIDE_SECONDS {
            fear.0 = KEEP_INSIDE_SECONDS;
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
    mut sounds: ResMut<Sounds>,
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
    sounds.play(Sound::Alarm);
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
