//! Kleine Effekte in der Welt: Erdkrümel, die beim Graben nach hinten fliegen, und Konfetti,
//! wenn ein Denkmal eingeweiht wird.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::camera::{Orbit, figure_layer};
use crate::colony::Rng;

const GRAVITY: f32 = 320.0;

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, prepare_dirt).add_systems(Update, fly_dirt);
    }
}

#[derive(Resource)]
pub struct DirtAssets {
    mesh: Handle<Mesh>,
    /// Ein paar Erdtöne, damit es nicht wie aus einer Form wirkt.
    materials: Vec<Handle<StandardMaterial>>,
    /// Bunte Schnipsel zum Feiern.
    confetti: Vec<Handle<StandardMaterial>>,
}

#[derive(Component)]
struct Dirt {
    velocity: Vec3,
    age: f32,
    lifetime: f32,
    size: f32,
    /// Konfetti sinkt langsam und flattert, Erde fällt einfach.
    flutter: Option<f32>,
}

fn prepare_dirt(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Frisch gelockerte Erde ist heller als der festgetretene Gangboden.
    let dirt = [[0.74, 0.55, 0.33], [0.64, 0.46, 0.27], [0.83, 0.65, 0.42]]
        .into_iter()
        .map(|[r, g, b]| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                unlit: true,
                ..default()
            })
        })
        .collect();
    let confetti = [
        [0.85, 0.31, 0.23],
        [0.94, 0.75, 0.25],
        [0.35, 0.60, 0.28],
        [0.35, 0.54, 0.82],
        [0.85, 0.42, 0.56],
        [0.98, 0.96, 0.90],
    ]
    .into_iter()
    .map(|[r, g, b]| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    })
    .collect();
    commands.insert_resource(DirtAssets {
        mesh: meshes.add(Circle::new(1.0)),
        materials: dirt,
        confetti,
    });
}

/// Wirft einen Erdkrümel von `origin` aus nach hinten (`backward`) und nach oben.
pub fn throw_dirt(commands: &mut Commands, assets: &DirtAssets, rng: &mut Rng, origin: Vec3, backward: Vec3) {
    let sideways = backward.cross(Vec3::Y);
    // Hoch genug, dass die Krümel über den Rand flacher Gänge fliegen.
    let velocity =
        backward * rng.range(40.0, 90.0) + Vec3::Y * rng.range(120.0, 170.0) + sideways * rng.range(-25.0, 25.0);
    let size = rng.range(2.0, 3.6);
    let material = assets.materials[rng.below(assets.materials.len())].clone();
    commands.spawn((
        Dirt {
            velocity,
            age: 0.0,
            lifetime: rng.range(0.7, 1.0),
            size,
            flutter: None,
        },
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(material),
        NotShadowCaster,
        figure_layer(),
        Transform::from_translation(origin).with_scale(Vec3::splat(size)),
    ));
}

/// Ein Konfettischnipsel, der von `origin` aus nach oben und zur Seite fliegt und langsam herabflattert.
pub fn throw_confetti(commands: &mut Commands, assets: &DirtAssets, rng: &mut Rng, origin: Vec3) {
    let angle = rng.range(0.0, std::f32::consts::TAU);
    let out = Vec3::new(angle.cos(), 0.0, angle.sin()) * rng.range(30.0, 110.0);
    let velocity = out + Vec3::Y * rng.range(110.0, 220.0);
    let size = rng.range(2.5, 4.0);
    let material = assets.confetti[rng.below(assets.confetti.len())].clone();
    commands.spawn((
        Dirt {
            velocity,
            age: 0.0,
            lifetime: rng.range(2.2, 3.2),
            size,
            flutter: Some(rng.range(6.0, 11.0)),
        },
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(material),
        NotShadowCaster,
        figure_layer(),
        Transform::from_translation(origin).with_scale(Vec3::splat(size)),
    ));
}

fn fly_dirt(
    mut commands: Commands,
    time: Res<Time>,
    orbit: Res<Orbit>,
    mut dirt: Query<(Entity, &mut Dirt, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let facing = orbit.billboard();
    for (entity, mut crumb, mut transform) in &mut dirt {
        crumb.age += dt;
        if crumb.age >= crumb.lifetime {
            commands.entity(entity).despawn();
            continue;
        }
        let left = 1.0 - crumb.age / crumb.lifetime;
        let fade = left.min(0.3) / 0.3;
        transform.rotation = facing;
        if let Some(spin) = crumb.flutter {
            // Die Luft bremst: Nach dem Hochwerfen sinkt Konfetti gemächlich und dreht sich dabei.
            crumb.velocity.y = (crumb.velocity.y - GRAVITY * 0.5 * dt).max(-45.0);
            crumb.velocity.x *= 1.0 - 1.5 * dt;
            crumb.velocity.z *= 1.0 - 1.5 * dt;
            transform.translation += crumb.velocity * dt;
            let turn = (crumb.age * spin).cos().abs().max(0.2);
            transform.scale = Vec3::new(crumb.size * turn, crumb.size * 1.4, 1.0) * fade;
            continue;
        }
        crumb.velocity.y -= GRAVITY * dt;
        transform.translation += crumb.velocity * dt;
        // Zum Ende hin kleiner werden statt plötzlich zu verschwinden.
        transform.scale = Vec3::splat(crumb.size * fade);
    }
}
