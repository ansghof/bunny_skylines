//! 3D-Boden aus dem Gitter: Die Karte liegt in der x-z-Ebene, Karten-y zeigt nach -z.
//! Oberfläche = Wiese mit Löchern an den Eingängen, Untergrund = Erdblock mit eingegrabenen Gängen.
//! Beide Meshes werden neu gebaut, sobald sich das Gitter ändert.

use std::f32::consts::FRAC_PI_2;

use bevy::asset::RenderAssetUsages;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::map::{Depth, Grid, Layer, TILE_SIZE, TilePos, ViewLayer, surface_color, underground_color};

/// Wie tief ein Eingangsloch an der Oberfläche wirkt.
const ENTRANCE_DEPTH: f32 = 36.0;
const SOIL_WALL: [f32; 3] = [0.42, 0.29, 0.17];

pub struct GroundPlugin;

impl Plugin for GroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_ground, spawn_sun))
            .add_systems(Update, (rebuild_ground, show_view_layer));
    }
}

/// Kartenpunkt (2D) auf Höhe `y` in die 3D-Welt.
pub fn to_world3(point: Vec2, y: f32) -> Vec3 {
    Vec3::new(point.x, y, -point.y)
}

/// Bodenhöhe eines gegrabenen Feldes.
pub fn depth_y(depth: Depth) -> f32 {
    match depth {
        Depth::Shallow => -28.0,
        Depth::Medium => -44.0,
        Depth::Deep => -60.0,
    }
}

/// Auf welcher Höhe eine Figur auf diesem Feld steht.
pub fn floor_y(grid: &Grid, layer: Layer, p: TilePos) -> f32 {
    match layer {
        Layer::Surface => 0.0,
        Layer::Underground => grid.under(p).dug.map_or(0.0, depth_y),
    }
}

/// Höhe, auf der die Maus in der jeweiligen Ansicht den Boden trifft.
pub fn floor_y_for_view(layer: Layer) -> f32 {
    match layer {
        Layer::Surface => 0.0,
        Layer::Underground => -20.0,
    }
}

/// Ein Rechteck auf dem Boden, z. B. für die Bauvorschau.
pub fn gizmo_rect(gizmos: &mut Gizmos, min: Vec2, max: Vec2, y: f32, color: Color) {
    let center = to_world3((min + max) / 2.0, y + 2.0);
    gizmos.rect(Isometry3d::new(center, Quat::from_rotation_x(-FRAC_PI_2)), max - min, color);
}

/// Ein Kreis auf dem Boden, z. B. für die Stampf-Welle.
pub fn gizmo_circle(gizmos: &mut Gizmos, center: Vec2, radius: f32, y: f32, color: Color) {
    let center = to_world3(center, y + 2.0);
    gizmos.circle(Isometry3d::new(center, Quat::from_rotation_x(-FRAC_PI_2)), radius, color);
}

#[derive(Component)]
struct GroundMesh(Layer);

fn spawn_ground(
    mut commands: Commands,
    grid: Res<Grid>,
    view: Res<ViewLayer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Die Farben kommen aus den Ecken des Meshes.
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        ..default()
    });
    for layer in [Layer::Surface, Layer::Underground] {
        commands.spawn((
            GroundMesh(layer),
            Mesh3d(meshes.add(build_mesh(&grid, layer))),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            visibility_for(layer, view.0),
        ));
    }
}

fn spawn_sun(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 2400.0,
            shadow_maps_enabled: true,
            ..default()
        },
        // Die Karte ist groß, die Schatten müssen weit reichen.
        CascadeShadowConfigBuilder {
            maximum_distance: 5000.0,
            first_cascade_far_bound: 900.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(-600.0, 1500.0, 900.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn visibility_for(layer: Layer, view: Layer) -> Visibility {
    if layer == view {
        Visibility::Visible
    } else {
        Visibility::Hidden
    }
}

fn show_view_layer(view: Res<ViewLayer>, mut grounds: Query<(&GroundMesh, &mut Visibility)>) {
    if !view.is_changed() {
        return;
    }
    for (ground, mut visibility) in &mut grounds {
        *visibility = visibility_for(ground.0, view.0);
    }
}

fn rebuild_ground(grid: Res<Grid>, mut meshes: ResMut<Assets<Mesh>>, grounds: Query<(&GroundMesh, &Mesh3d)>) {
    if !grid.is_changed() || grid.is_added() {
        return;
    }
    for (ground, mesh) in &grounds {
        // Schlägt nur fehl, wenn das Mesh gerade entfernt wurde.
        let _ = meshes.insert(&mesh.0, build_mesh(&grid, ground.0));
    }
}

/// Sammelt Vierecke mit flacher Beleuchtung und Eckfarben.
#[derive(Default)]
struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    /// Ein Viereck aus vier Ecken; die Reihenfolge wird so gedreht, dass es zur Normalen zeigt.
    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, color: [f32; 3]) {
        let facing = (corners[1] - corners[0]).cross(corners[2] - corners[0]).dot(normal);
        let order = if facing >= 0.0 { [0, 1, 2, 3] } else { [0, 3, 2, 1] };
        let start = self.positions.len() as u32;
        let linear = Color::srgb(color[0], color[1], color[2]).to_linear().to_f32_array();
        for i in order {
            self.positions.push(corners[i].to_array());
            self.normals.push(normal.to_array());
            self.colors.push(linear);
        }
        self.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
    }

    fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

fn rgb(color: Color) -> [f32; 3] {
    let c = color.to_srgba();
    [c.red, c.green, c.blue]
}

/// Höhe der Oberkante eines Feldes in der jeweiligen Ansicht.
fn top_y(grid: &Grid, layer: Layer, p: TilePos) -> f32 {
    match layer {
        Layer::Surface if grid.surface(p).entrance => -ENTRANCE_DEPTH,
        Layer::Surface => 0.0,
        Layer::Underground => grid.under(p).dug.map_or(0.0, depth_y),
    }
}

fn build_mesh(grid: &Grid, layer: Layer) -> Mesh {
    let mut builder = MeshBuilder::default();
    let half = TILE_SIZE / 2.0;

    for p in Grid::positions() {
        let center = p.center();
        let y = top_y(grid, layer, p);
        let color = match layer {
            Layer::Surface => rgb(surface_color(grid, p)),
            Layer::Underground => rgb(underground_color(grid, p)),
        };
        let corner = |dx: f32, dy: f32, h: f32| to_world3(center + Vec2::new(dx, dy), h);
        builder.quad(
            [corner(-half, -half, y), corner(half, -half, y), corner(half, half, y), corner(-half, half, y)],
            Vec3::Y,
            color,
        );

        // Wände dort, wo der Nachbar höher liegt, damit Gänge eingegraben wirken.
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let neighbor = TilePos::new(p.x + dx, p.y + dy);
            let neighbor_y = if neighbor.in_bounds() { top_y(grid, layer, neighbor) } else { 0.0 };
            if neighbor_y <= y {
                continue;
            }
            let edge = Vec2::new(dx as f32, dy as f32) * half;
            let along = Vec2::new(-(dy as f32), dx as f32) * half;
            let normal = -to_world3(Vec2::new(dx as f32, dy as f32), 0.0);
            // Tiefere Wände etwas dunkler.
            let shade = 1.0 + y / 160.0;
            let wall = SOIL_WALL.map(|c| c * shade);
            builder.quad(
                [
                    to_world3(center + edge - along, y),
                    to_world3(center + edge + along, y),
                    to_world3(center + edge + along, neighbor_y),
                    to_world3(center + edge - along, neighbor_y),
                ],
                normal,
                wall,
            );
        }
    }
    builder.build()
}
