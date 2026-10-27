//! 3D-Boden aus dem Gitter: Die Karte liegt in der x-z-Ebene, Karten-y zeigt nach -z.
//! Oberfläche = Wiese mit Löchern an den Eingängen, Untergrund = Erdblock mit eingegrabenen Gängen.
//! Beide Meshes werden neu gebaut, sobald sich das Gitter ändert.
//! Jede Bodenart hat eine kachelbare Textur aus `assets/boden/`, die die Feldfarbe nur noch einfärbt.

use std::f32::consts::FRAC_PI_2;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::overlay::{MapOverlay, pasture_color};
use crate::map::{Depth, Grid, Layer, MAP_WIDTH, TILE_SIZE, TilePos, ViewLayer, surface_color, underground_color};

const SOIL_WALL: [f32; 3] = [0.42, 0.29, 0.17];
/// So viele Felder deckt eine Bodentextur ab, bevor sie sich wiederholt.
const TEXTURE_SPAN: f32 = 4.0 * TILE_SIZE;

/// Die Bodenarten, jede mit eigener Textur.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Texture {
    Meadow,
    Dirt,
    Sand,
    Stones,
    Water,
    ForestFloor,
    Soil,
    Tunnel,
}

impl Texture {
    const ALL: [Texture; 8] = [
        Texture::Meadow,
        Texture::Dirt,
        Texture::Sand,
        Texture::Stones,
        Texture::Water,
        Texture::ForestFloor,
        Texture::Soil,
        Texture::Tunnel,
    ];

    fn file(self) -> &'static str {
        match self {
            Texture::Meadow => "boden/wiese.png",
            Texture::Dirt => "boden/erde.png",
            Texture::Sand => "boden/sand.png",
            Texture::Stones => "boden/steine.png",
            Texture::Water => "boden/wasser.png",
            Texture::ForestFloor => "boden/waldboden.png",
            Texture::Soil => "boden/untergrund.png",
            Texture::Tunnel => "boden/gang.png",
        }
    }

    /// Welche Textur ein Feld in dieser Ansicht bekommt.
    fn of(grid: &Grid, layer: Layer, p: TilePos) -> Texture {
        if layer == Layer::Underground {
            return if grid.under(p).dug.is_some() { Texture::Tunnel } else { Texture::Soil };
        }
        let tile = grid.surface(p);
        match () {
            _ if tile.water && tile.ford => Texture::Stones,
            _ if tile.water => Texture::Water,
            _ if tile.stones => Texture::Stones,
            _ if tile.sand => Texture::Sand,
            _ if tile.forest => Texture::ForestFloor,
            // Kahlgefressen oder ausgelaugt: nackte Erde.
            _ if tile.grass < 0.08 || tile.fertility < 0.45 => Texture::Dirt,
            _ => Texture::Meadow,
        }
    }
}

pub struct GroundPlugin;

impl Plugin for GroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_ground, spawn_sun))
            .add_systems(Update, (rebuild_ground, show_view_layer, overlay_lighting, prepare_textures));
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

/// Der Kartenpunkt, den ein Mausstrahl trifft. Zuerst auf Höhe der Oberkante;
/// liegt dort im Untergrund ein Gang, folgt der Strahl bis auf dessen Boden.
pub fn pick(grid: &Grid, view: Layer, ray: Ray3d) -> Option<Vec2> {
    let hit_at = |y: f32| {
        let distance = (y - ray.origin.y) / ray.direction.y;
        (distance > 0.0).then(|| {
            let hit = ray.origin + *ray.direction * distance;
            Vec2::new(hit.x, -hit.z)
        })
    };
    let top = hit_at(0.0)?;
    if view == Layer::Surface {
        return Some(top);
    }
    let Some(tile) = TilePos::from_world(top).filter(|p| grid.under(*p).dug.is_some()) else {
        return Some(top);
    };
    // Trifft der Strahl auf dem Gangboden wieder einen Gang, ist das der sichtbare Punkt;
    // sonst endet er an einer Gangwand, und die Oberkante passt besser.
    let floor = hit_at(floor_y(grid, Layer::Underground, tile))?;
    let floor_is_dug = TilePos::from_world(floor).is_some_and(|p| grid.under(p).dug.is_some());
    Some(if floor_is_dug { floor } else { top })
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

/// Ein Teil des Bodens: alle Felder einer Ansicht mit derselben Textur. `true` = die
/// durchscheinenden Ränder, mit denen diese Textur in andere Nachbarfelder ausläuft.
#[derive(Component)]
struct GroundMesh(Layer, Texture, bool);

/// Bild und Materialien jeder Bodentextur: deckend für die Felder, durchscheinend für die Ränder.
#[derive(Resource)]
struct GroundTextures(Vec<(Handle<Image>, [Handle<StandardMaterial>; 2])>);

fn spawn_ground(
    mut commands: Commands,
    grid: Res<Grid>,
    view: Res<ViewLayer>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut textures = Vec::new();
    for texture in Texture::ALL {
        let image: Handle<Image> = asset_server.load(texture.file());
        // Die Farben kommen aus den Ecken des Meshes, die Textur gibt die Zeichnung dazu.
        let solid = StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(image.clone()),
            perceptual_roughness: 0.95,
            ..default()
        };
        let edge = StandardMaterial { alpha_mode: AlphaMode::Blend, ..solid.clone() };
        textures.push((image, [materials.add(solid), materials.add(edge)]));
    }
    for layer in [Layer::Surface, Layer::Underground] {
        let parts = build_meshes(&grid, layer, MapOverlay::Off);
        for (i, mesh) in parts.into_iter().enumerate() {
            let (texture, edge) = part_of(i);
            commands.spawn((
                GroundMesh(layer, texture, edge),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(textures[i % Texture::ALL.len()].1[edge as usize].clone()),
                Transform::default(),
                visibility_for(layer, view.0),
            ));
        }
    }
    commands.insert_resource(GroundTextures(textures));
}

/// Macht eine frisch geladene Bodentextur fertig: Sie wiederholt sich nahtlos, bekommt kleinere
/// Stufen (Mipmaps), damit sie von weitem nicht flimmert, und das Material gleicht aus, wie dunkel
/// sie im Mittel ist, damit die Feldfarben stimmen. Klappt auch mit gemalten Ersatzbildern.
fn prepare_textures(
    mut events: MessageReader<AssetEvent<Image>>,
    textures: Option<Res<GroundTextures>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(textures) = textures else {
        return;
    };
    for event in events.read() {
        let (AssetEvent::LoadedWithDependencies { id } | AssetEvent::Modified { id }) = *event else {
            continue;
        };
        let Some((_, pair)) = textures.0.iter().find(|(image, _)| image.id() == id) else {
            continue;
        };
        let Some(image) = images.get(id) else {
            continue;
        };
        if image.texture_descriptor.mip_level_count > 1 {
            continue;
        }
        let Some(prepared) = with_mipmaps(image) else {
            continue;
        };
        let gain = 1.0 / mean_brightness(&prepared).max(0.05);
        for material in pair {
            if let Some(mut material) = materials.get_mut(material) {
                material.base_color = Color::linear_rgb(gain, gain, gain);
            }
        }
        let _ = images.insert(id, prepared);
    }
}

/// Wie hell ein sRGB-Bild im Mittel ist, linear gerechnet.
fn mean_brightness(image: &Image) -> f32 {
    let Some(data) = &image.data else {
        return 1.0;
    };
    let pixels = (image.width() * image.height()) as usize;
    let linear = |byte: u8| Color::srgb_u8(byte, byte, byte).to_linear().red;
    let table: Vec<f32> = (0..=255).map(linear).collect();
    let sum: f32 = data[..pixels * 4]
        .chunks_exact(4)
        .map(|p| 0.3 * table[p[0] as usize] + 0.55 * table[p[1] as usize] + 0.15 * table[p[2] as usize])
        .sum();
    sum / pixels as f32
}

/// Eine Kopie mit allen Mipmap-Stufen, jede halb so groß wie die vorige. Nur für RGBA-Bilder
/// mit Kanten in Zweierpotenzen; andere bleiben, wie sie sind.
fn with_mipmaps(image: &Image) -> Option<Image> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let data = image.data.as_ref()?;
    if !width.is_power_of_two() || !height.is_power_of_two() || data.len() != width * height * 4 {
        return None;
    }
    let mut all = data.clone();
    let mut level = data.clone();
    let (mut w, mut h) = (width, height);
    let mut levels = 1;
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let at = |dx: usize, dy: usize| {
                        let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                        level[(sy * w + sx) * 4 + c] as u32
                    };
                    next[(y * nw + x) * 4 + c] = ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
                }
            }
        }
        all.extend_from_slice(&next);
        level = next;
        (w, h) = (nw, nh);
        levels += 1;
    }
    let mut prepared = image.clone();
    prepared.data = Some(all);
    prepared.texture_descriptor.mip_level_count = levels;
    prepared.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    Some(prepared)
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

/// Eine Übersichtskarte soll auch nachts gut lesbar sein: Solange sie an ist, ohne Licht und Schatten.
fn overlay_lighting(
    overlay: Res<MapOverlay>,
    grounds: Query<&MeshMaterial3d<StandardMaterial>, With<GroundMesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !overlay.is_changed() {
        return;
    }
    for material in &grounds {
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.unlit = *overlay == MapOverlay::Pasture;
        }
    }
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

fn rebuild_ground(
    grid: Res<Grid>,
    overlay: Res<MapOverlay>,
    mut meshes: ResMut<Assets<Mesh>>,
    grounds: Query<(&GroundMesh, &Mesh3d)>,
) {
    if (!grid.is_changed() && !overlay.is_changed()) || grid.is_added() {
        return;
    }
    for layer in [Layer::Surface, Layer::Underground] {
        let parts = build_meshes(&grid, layer, *overlay);
        for (i, part) in parts.into_iter().enumerate() {
            let (texture, edge) = part_of(i);
            for (ground, mesh) in &grounds {
                if ground.0 == layer && ground.1 == texture && ground.2 == edge {
                    // Schlägt nur fehl, wenn das Mesh gerade entfernt wurde.
                    let _ = meshes.insert(&mesh.0, part.clone());
                }
            }
        }
    }
}

/// Sammelt Vierecke mit flacher Beleuchtung, Eckfarben und Texturkoordinaten.
#[derive(Default)]
struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    /// Ein Viereck aus vier Ecken mit ihren Texturkoordinaten; die Reihenfolge wird so gedreht,
    /// dass es zur Normalen zeigt.
    fn quad(&mut self, corners: [Vec3; 4], uvs: [Vec2; 4], normal: Vec3, color: [f32; 3]) {
        let facing = (corners[1] - corners[0]).cross(corners[2] - corners[0]).dot(normal);
        let order = if facing >= 0.0 { [0, 1, 2, 3] } else { [0, 3, 2, 1] };
        let start = self.positions.len() as u32;
        let linear = Color::srgb(color[0], color[1], color[2]).to_linear().to_f32_array();
        for i in order {
            self.positions.push(corners[i].to_array());
            self.normals.push(normal.to_array());
            self.colors.push(linear);
            self.uvs.push(uvs[i].to_array());
        }
        self.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
    }

    /// Ein flaches Feld aus vier Dreiecken um die Mitte, jede Ecke mit eigener Farbe.
    fn fan(&mut self, middle: Vec3, middle_uv: Vec2, middle_color: [f32; 3], corners: [Vec3; 4], uvs: [Vec2; 4], colors: [[f32; 3]; 4]) {
        let linear = |c: [f32; 3]| Color::srgb(c[0], c[1], c[2]).to_linear().to_f32_array();
        let start = self.positions.len() as u32;
        self.positions.push(middle.to_array());
        self.uvs.push(middle_uv.to_array());
        self.colors.push(linear(middle_color));
        for i in 0..4 {
            self.positions.push(corners[i].to_array());
            self.uvs.push(uvs[i].to_array());
            self.colors.push(linear(colors[i]));
        }
        for _ in 0..5 {
            self.normals.push(Vec3::Y.to_array());
        }
        for i in 0..4u32 {
            let (a, b) = (start + 1 + i, start + 1 + (i + 1) % 4);
            let facing = (corners[(i as usize + 1) % 4] - middle).cross(corners[i as usize] - middle).dot(Vec3::Y);
            if facing >= 0.0 {
                self.indices.extend([start, b, a]);
            } else {
                self.indices.extend([start, a, b]);
            }
        }
    }

    /// Ein Streifen, der von `alpha[0]` an der ersten Kante zu `alpha[1]` an der zweiten ausblendet.
    fn strip(&mut self, corners: [Vec3; 4], uvs: [Vec2; 4], colors: [[f32; 3]; 4], alpha: [f32; 4]) {
        let start = self.positions.len() as u32;
        for i in 0..4 {
            let mut linear = Color::srgb(colors[i][0], colors[i][1], colors[i][2]).to_linear().to_f32_array();
            linear[3] = alpha[i];
            self.positions.push(corners[i].to_array());
            self.normals.push(Vec3::Y.to_array());
            self.colors.push(linear);
            self.uvs.push(uvs[i].to_array());
        }
        let facing = (corners[1] - corners[0]).cross(corners[2] - corners[0]).dot(Vec3::Y);
        if facing >= 0.0 {
            self.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
        } else {
            self.indices.extend([start, start + 2, start + 1, start, start + 3, start + 2]);
        }
    }

    fn build(mut self) -> Mesh {
        // Ein leeres Mesh mag Bevy nicht; ein unsichtbares Dreieck tut es auch.
        if self.positions.is_empty() {
            self.positions = vec![[0.0; 3]; 3];
            self.normals = vec![[0.0, 1.0, 0.0]; 3];
            self.colors = vec![[0.0, 0.0, 0.0, 1.0]; 3];
            self.uvs = vec![[0.0; 2]; 3];
            self.indices = vec![0, 1, 2];
        }
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
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
        Layer::Surface => 0.0,
        Layer::Underground => grid.under(p).dug.map_or(0.0, depth_y),
    }
}

/// Welche Textur das i-te Teil aus `build_meshes` hat und ob es die durchscheinenden Ränder sind.
fn part_of(i: usize) -> (Texture, bool) {
    (Texture::ALL[i % Texture::ALL.len()], i >= Texture::ALL.len())
}

/// Wie weit eine Textur an der Oberfläche in ein andersartiges Nachbarfeld ausläuft.
const BLEND_REACH: f32 = 0.45;

/// Alle Teile einer Ansicht: erst die Felder jeder Textur in der Reihenfolge von `Texture::ALL`,
/// dann ebenso ihre durchscheinenden Ränder. Jedes Feld wird aus vier Dreiecken um seine Mitte
/// gebaut: Die Mitte hat die Farbe des Feldes, die Ecken mischen sie mit gleichartigen Nachbarn.
/// An der Oberfläche läuft jede Textur dazu in andersartige Nachbarn aus, halb deckend an der
/// Kante und nach innen verblassend; von beiden Seiten zusammen gibt das einen weichen Übergang
/// statt einer geraden Kante. Gangwände bekommen immer die Erde des Untergrunds.
fn build_meshes(grid: &Grid, layer: Layer, overlay: MapOverlay) -> Vec<Mesh> {
    let mut builders: Vec<MeshBuilder> = Texture::ALL.iter().map(|_| MeshBuilder::default()).collect();
    let mut edges: Vec<MeshBuilder> = Texture::ALL.iter().map(|_| MeshBuilder::default()).collect();
    let slot = |texture: Texture| Texture::ALL.iter().position(|t| *t == texture).unwrap();
    let half = TILE_SIZE / 2.0;
    // Die Textur liegt durchgehend über der Karte, damit gleiche Nachbarn nahtlos ineinander übergehen.
    let uv = |point: Vec2| point / TEXTURE_SPAN;

    let index = |p: TilePos| (p.y * MAP_WIDTH + p.x) as usize;
    let tiles: Vec<(Texture, f32, [f32; 3])> = Grid::positions()
        .map(|p| {
            let color = match layer {
                Layer::Surface if overlay == MapOverlay::Pasture => rgb(pasture_color(grid.surface(p))),
                Layer::Surface => rgb(surface_color(grid, p)),
                Layer::Underground => rgb(underground_color(grid, p)),
            };
            (Texture::of(grid, layer, p), top_y(grid, layer, p), color)
        })
        .collect();

    for p in Grid::positions() {
        let center = p.center();
        let (texture, y, color) = tiles[index(p)];
        // Eine Ecke mischt alle Felder, die sie berühren und gleich aussehen und gleich hoch liegen.
        // Im Untergrund nicht: Dort muss man genau sehen, wo eine Bodenart aufhört.
        let corner_color = |sx: i32, sy: i32| {
            if layer == Layer::Underground {
                return color;
            }
            let mut sum = [0.0; 3];
            let mut count = 0.0;
            for (dx, dy) in [(0, 0), (sx, 0), (0, sy), (sx, sy)] {
                let q = TilePos::new(p.x + dx, p.y + dy);
                if !q.in_bounds() {
                    continue;
                }
                let (other, other_y, other_color) = tiles[index(q)];
                if other == texture && other_y == y {
                    sum = [0, 1, 2].map(|c| sum[c] + other_color[c]);
                    count += 1.0;
                }
            }
            sum.map(|c| c / count)
        };
        let offsets = [(-1, -1), (1, -1), (1, 1), (-1, 1)];
        let corners = offsets.map(|(sx, sy)| center + Vec2::new(sx as f32, sy as f32) * half);
        builders[slot(texture)].fan(
            to_world3(center, y),
            uv(center),
            color,
            corners.map(|c| to_world3(c, y)),
            corners.map(uv),
            offsets.map(|(sx, sy)| corner_color(sx, sy)),
        );

        // Im Untergrund bleiben die Bodenarten scharf getrennt, sie bestimmen das Graben.
        if layer == Layer::Surface {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let neighbor = TilePos::new(p.x + dx, p.y + dy);
                if !neighbor.in_bounds() || tiles[index(neighbor)].0 == texture || tiles[index(neighbor)].1 != y {
                    continue;
                }
                let out = Vec2::new(dx as f32, dy as f32);
                let ends = [(dx - dy, dy + dx), (dx + dy, dy - dx)];
                let edge = ends.map(|(sx, sy)| center + Vec2::new(sx as f32, sy as f32) * half);
                let inner = edge.map(|point| point + out * TILE_SIZE * BLEND_REACH);
                let points = [edge[0], edge[1], inner[1], inner[0]];
                let colors = ends.map(|(sx, sy)| corner_color(sx, sy));
                edges[slot(texture)].strip(
                    points.map(|point| to_world3(point, y + 0.15)),
                    points.map(uv),
                    [colors[0], colors[1], colors[1], colors[0]],
                    [0.5, 0.5, 0.0, 0.0],
                );
            }
        }

        // Wände dort, wo der Nachbar höher liegt, damit Gänge eingegraben wirken.
        let walls = &mut builders[slot(Texture::Soil)];
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let neighbor = TilePos::new(p.x + dx, p.y + dy);
            let neighbor_y = if neighbor.in_bounds() { tiles[index(neighbor)].1 } else { 0.0 };
            if neighbor_y <= y {
                continue;
            }
            let edge = Vec2::new(dx as f32, dy as f32) * half;
            let along = Vec2::new(-(dy as f32), dx as f32) * half;
            let normal = -to_world3(Vec2::new(dx as f32, dy as f32), 0.0);
            // Tiefere Wände etwas dunkler.
            let shade = 1.0 + y / 160.0;
            let wall = SOIL_WALL.map(|c| c * shade);
            // An der Wand läuft die Textur waagerecht an der Kante entlang und senkrecht nach unten.
            let side = |point: Vec2, h: f32| Vec2::new((point.x + point.y) / TEXTURE_SPAN, -h / TEXTURE_SPAN);
            walls.quad(
                [
                    to_world3(center + edge - along, y),
                    to_world3(center + edge + along, y),
                    to_world3(center + edge + along, neighbor_y),
                    to_world3(center + edge - along, neighbor_y),
                ],
                [
                    side(center + edge - along, y),
                    side(center + edge + along, y),
                    side(center + edge + along, neighbor_y),
                    side(center + edge - along, neighbor_y),
                ],
                normal,
                wall,
            );
        }
    }
    builders.into_iter().chain(edges).map(MeshBuilder::build).collect()
}
