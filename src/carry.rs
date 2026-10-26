//! Erwischte Kaninchen verschwinden nicht einfach: Fuchs, Marder und Bussard tragen sie sichtbar davon,
//! verkleinert und in der Fellfarbe des Kaninchens. Jeder Jäger meldet dafür, wo er seine Beute hält.

use bevy::prelude::*;

use crate::bird::Sky;
use crate::camera::{Orbit, figure_layer};
use crate::scenario::Losses;

const IMAGE: &str = "figuren/beute/getragen.png";
/// So groß hängt die Beute im Maul oder in den Fängen.
const SIZE: Vec2 = Vec2::new(44.0, 28.0);

pub struct CarryPlugin;

impl Plugin for CarryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, prepare_assets).add_systems(PostUpdate, follow_carrier);
    }
}

/// Wo ein Jäger seine Beute hält, in Weltkoordinaten; vom Jäger selbst jedes Bild gesetzt.
#[derive(Component, Default)]
pub struct CarryPoint {
    pub at: Vec3,
    /// Ob der Jäger gerade in der sichtbaren Ansicht ist.
    pub visible: bool,
    /// 1 = nach rechts auf dem Bildschirm, -1 = nach links.
    pub facing: f32,
}

#[derive(Component)]
struct Carried {
    carrier: Entity,
}

#[derive(Resource)]
struct CarryAssets {
    quad: Handle<Mesh>,
    image: Handle<Image>,
}

fn prepare_assets(mut commands: Commands, asset_server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(CarryAssets {
        // Aufgehängt an der Oberkante, damit die Beute unter dem Maul baumelt.
        quad: meshes.add(Mesh::from(Rectangle::new(1.0, 1.0)).translated_by(Vec3::new(0.0, -0.5, 0.0))),
        image: asset_server.load(IMAGE),
    });
}

/// Der Jäger `carrier` trägt ab jetzt ein Kaninchen mit diesem Fell davon.
/// Zählt zugleich als Verlust durch Feinde für die Ziele des Kapitels.
pub fn carry_off(commands: &mut Commands, carrier: Entity, fur: Color) {
    commands.queue(move |world: &mut World| {
        world.resource_mut::<Losses>().0 += 1;
        let (quad, image) = {
            let assets = world.resource::<CarryAssets>();
            (assets.quad.clone(), assets.image.clone())
        };
        let material = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
            base_color: fur,
            base_color_texture: Some(image),
            alpha_mode: AlphaMode::AlphaToCoverage,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        world.spawn((
            Sky,
            Carried { carrier },
            Mesh3d(quad),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            figure_layer(),
        ));
    });
}

fn follow_carrier(
    mut commands: Commands,
    time: Res<Time>,
    orbit: Res<Orbit>,
    carriers: Query<&CarryPoint>,
    mut carried: Query<(Entity, &Carried, &mut Transform, &mut Visibility)>,
) {
    for (entity, prey, mut transform, mut visibility) in &mut carried {
        let Ok(point) = carriers.get(prey.carrier) else {
            // Der Jäger ist fort, und die Beute mit ihm.
            commands.entity(entity).despawn();
            continue;
        };
        // Ein leichtes Schaukeln beim Laufen.
        let sway = (time.elapsed_secs() * 7.0).sin() * 0.08;
        transform.translation = point.at;
        transform.rotation = orbit.billboard() * Quat::from_rotation_z(sway);
        transform.scale = Vec3::new(SIZE.x * point.facing, SIZE.y, 1.0);
        *visibility = if point.visible { Visibility::Inherited } else { Visibility::Hidden };
    }
}
