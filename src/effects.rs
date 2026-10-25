//! Kleine Effekte in der Welt: Erdkrümel, die beim Graben nach hinten fliegen.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::camera::Orbit;
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
}

#[derive(Component)]
struct Dirt {
    velocity: Vec3,
    age: f32,
    lifetime: f32,
    size: f32,
}

fn prepare_dirt(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Frisch gelockerte Erde ist heller als der festgetretene Gangboden.
    let materials = [[0.74, 0.55, 0.33], [0.64, 0.46, 0.27], [0.83, 0.65, 0.42]]
        .into_iter()
        .map(|[r, g, b]| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                unlit: true,
                ..default()
            })
        })
        .collect();
    commands.insert_resource(DirtAssets {
        mesh: meshes.add(Circle::new(1.0)),
        materials,
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
        },
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(material),
        NotShadowCaster,
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
        crumb.velocity.y -= GRAVITY * dt;
        transform.translation += crumb.velocity * dt;
        transform.rotation = facing;
        // Zum Ende hin kleiner werden statt plötzlich zu verschwinden.
        let left = 1.0 - crumb.age / crumb.lifetime;
        transform.scale = Vec3::splat(crumb.size * left.min(0.3) / 0.3);
    }
}
