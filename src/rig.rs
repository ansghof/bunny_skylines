//! Figuren aus Einzelteilen: Die Rig-Datei (`*.rig.ron`) beschreibt, welche Bilder
//! wo sitzen und um welchen Punkt sie sich drehen. Animationen sprechen nur Teilnamen an.
//!
//! In der 3D-Ansicht steht jede Figur als Aufsteller in der Welt: Ein `Avatar` folgt der
//! Spielfigur und dreht sich zur Kamera, die Teile sind flache Bildtafeln darin.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use serde::Deserialize;

use crate::camera::figure_layer;
use crate::ground::{floor_y, to_world3};
use crate::map::{Grid, Layer, TilePos};
use crate::rabbit::OnLayer;

/// Abstand der Ebenen eines Rigs in Bildpixeln; groß genug, dass die Tiefenprüfung sie trennt.
const LAYER_SPACING: f32 = 1.0;

pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<RigDefinition>()
            .init_asset_loader::<RigLoader>()
            .init_resource::<PivotMeshes>()
            .add_systems(Startup, prepare_shadow)
            .add_systems(
                Update,
                (remove_orphans, reload_rigs, attach_rigs, size_parts, follow_owners).chain(),
            );
    }
}

#[derive(Asset, TypePath, Deserialize, Debug, Clone)]
pub struct RigDefinition {
    #[serde(rename = "pixel_massstab")]
    pub pixel_scale: f32,
    #[serde(rename = "teile")]
    pub parts: Vec<PartDefinition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PartDefinition {
    pub name: String,
    #[serde(rename = "bild")]
    pub image: String,
    pub position: (f32, f32),
    #[serde(rename = "drehpunkt")]
    pub pivot: (f32, f32),
    #[serde(rename = "drehung", default)]
    pub rotation: f32,
    #[serde(rename = "ebene")]
    pub layer: i32,
    #[serde(rename = "faerben", default = "yes")]
    pub tint: bool,
    #[serde(rename = "varianten", default)]
    pub variants: HashMap<String, String>,
}

fn yes() -> bool {
    true
}

#[derive(Default, TypePath)]
struct RigLoader;

impl AssetLoader for RigLoader {
    type Asset = RigDefinition;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<RigDefinition, BevyError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(ron::de::from_bytes(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &["rig.ron"]
    }
}

/// Welche Rig-Datei eine Figur benutzt. Die Teile werden aufgebaut, sobald die Datei geladen ist.
#[derive(Component)]
pub struct RigSource {
    pub rig: Handle<RigDefinition>,
    /// Ordner der Bilder, relativ zu `assets/`.
    pub folder: &'static str,
}

/// Der sichtbare Aufsteller einer Spielfigur.
#[derive(Component)]
pub struct Avatar {
    pub of: Entity,
}

/// Verweis von der Spielfigur auf ihren Aufsteller.
#[derive(Component)]
pub struct AvatarLink(pub Entity);

/// Wurzel aller Teile einer Figur; Hüpfen, Stauchen und Blickrichtung wirken hier.
#[derive(Component)]
pub struct RigRoot {
    pub pixel_scale: f32,
}

/// Ein Teil einer Figur mit seiner Grundstellung aus der Rig-Datei.
#[derive(Component)]
pub struct Part {
    pub name: String,
    pub base: Transform,
    pub tint: bool,
    pub image: Handle<Image>,
    pub variants: HashMap<String, Handle<Image>>,
    material: Handle<StandardMaterial>,
}

/// Teil, dessen Bildgröße noch nicht bekannt ist.
#[derive(Component)]
struct NeedsSize;

/// Ein Viereck pro Drehpunkt, so verschoben, dass der Drehpunkt im Ursprung liegt.
#[derive(Resource, Default)]
struct PivotMeshes(HashMap<(i32, i32), Handle<Mesh>>);

/// Weicher Schatten unter jeder Figur, damit sie nicht zu schweben scheint.
#[derive(Resource)]
struct ShadowAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

fn prepare_shadow(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ShadowAssets {
        mesh: meshes.add(Ellipse::new(1.0, 0.5).mesh().build().rotated_by(Quat::from_rotation_x(-FRAC_PI_2))),
        material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.1, 0.07, 0.04, 0.28),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    });
}

fn attach_rigs(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    shadow: Res<ShadowAssets>,
    rigs: Res<Assets<RigDefinition>>,
    mut pivot_meshes: ResMut<PivotMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    owners: Query<(Entity, &RigSource), Without<AvatarLink>>,
) {
    for (owner, source) in &owners {
        let Some(rig) = rigs.get(&source.rig) else {
            continue;
        };
        let avatar = commands
            .spawn((Avatar { of: owner }, Transform::default(), Visibility::default()))
            .id();
        // Größe grob nach der Figur: Bildbreite in Pixeln mal Maßstab.
        let width = rig.parts.iter().map(|p| p.position.0.abs()).fold(0.0, f32::max) * rig.pixel_scale;
        commands.spawn((
            Mesh3d(shadow.mesh.clone()),
            MeshMaterial3d(shadow.material.clone()),
            NotShadowCaster,
            Transform::from_xyz(0.0, 0.6, 0.0).with_scale(Vec3::new(width, 1.0, width)),
            ChildOf(avatar),
        ));
        let root = commands
            .spawn((
                RigRoot { pixel_scale: rig.pixel_scale },
                Transform::from_scale(Vec3::splat(rig.pixel_scale)),
                Visibility::default(),
                ChildOf(avatar),
            ))
            .id();

        for part in &rig.parts {
            let image: Handle<Image> = asset_server.load(format!("{}/{}", source.folder, part.image));
            let variants = part
                .variants
                .iter()
                .map(|(name, file)| (name.clone(), asset_server.load(format!("{}/{file}", source.folder))))
                .collect();
            let key = ((part.pivot.0 * 1000.0) as i32, (part.pivot.1 * 1000.0) as i32);
            let mesh = pivot_meshes
                .0
                .entry(key)
                .or_insert_with(|| {
                    let offset = Vec3::new(0.5 - part.pivot.0, 0.5 - part.pivot.1, 0.0);
                    meshes.add(Mesh::from(Rectangle::new(1.0, 1.0)).translated_by(offset))
                })
                .clone();
            // Jedes Teil hat eine eigene Farbe (Fell, Transparenz), also ein eigenes Material.
            let material = materials.add(StandardMaterial {
                base_color_texture: Some(image.clone()),
                alpha_mode: AlphaMode::AlphaToCoverage,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            let base = Transform {
                translation: Vec3::new(part.position.0, part.position.1, part.layer as f32 * LAYER_SPACING),
                rotation: Quat::from_rotation_z(part.rotation.to_radians()),
                // Bis die Bildgröße bekannt ist, unsichtbar klein.
                scale: Vec3::ZERO,
            };
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                NotShadowCaster,
                figure_layer(),
                base,
                Part {
                    name: part.name.clone(),
                    base,
                    tint: part.tint,
                    image,
                    variants,
                    material,
                },
                NeedsSize,
                ChildOf(root),
            ));
        }
        commands.entity(owner).insert(AvatarLink(avatar));
    }
}

/// Sobald ein Bild geladen ist, bekommt sein Teil die passende Größe in Bildpixeln.
fn size_parts(
    mut commands: Commands,
    images: Res<Assets<Image>>,
    mut parts: Query<(Entity, &mut Part), With<NeedsSize>>,
) {
    for (entity, mut part) in &mut parts {
        if let Some(image) = images.get(&part.image) {
            let size = image.size_f32();
            part.base.scale = Vec3::new(size.x, size.y, 1.0);
            commands.entity(entity).remove::<NeedsSize>();
        }
    }
}

/// Wird eine Rig-Datei gespeichert, bauen alle Figuren mit dieser Datei ihre Teile neu auf.
fn reload_rigs(
    mut commands: Commands,
    mut events: MessageReader<AssetEvent<RigDefinition>>,
    owners: Query<(Entity, &RigSource, &AvatarLink)>,
) {
    for event in events.read() {
        let AssetEvent::Modified { id } = event else {
            continue;
        };
        for (owner, source, link) in &owners {
            if source.rig.id() == *id {
                commands.entity(link.0).despawn();
                commands.entity(owner).remove::<AvatarLink>();
            }
        }
    }
}

/// Aufsteller, deren Figur verschwunden ist (z. B. vom Fuchs erwischt), werden abgebaut.
fn remove_orphans(mut commands: Commands, avatars: Query<(Entity, &Avatar)>, owners: Query<(), With<RigSource>>) {
    for (entity, avatar) in &avatars {
        if owners.get(avatar.of).is_err() {
            commands.entity(entity).despawn();
        }
    }
}

/// Aufsteller stehen auf der Bodenhöhe ihrer Figur. Zur Kamera dreht sich die `RigRoot`,
/// damit der Schatten am Boden flach liegen bleibt.
fn follow_owners(
    grid: Res<Grid>,
    owners: Query<(&Transform, Option<&OnLayer>), Without<Avatar>>,
    mut avatars: Query<(&Avatar, &mut Transform)>,
) {
    for (avatar, mut transform) in &mut avatars {
        let Ok((owner, layer)) = owners.get(avatar.of) else {
            continue;
        };
        let point = owner.translation.truncate();
        let layer = layer.map_or(Layer::Surface, |l| l.0);
        let y = floor_y(&grid, layer, TilePos::from_world_clamped(point));
        transform.translation = to_world3(point, y);
    }
}

/// Grundstellung eines Teils plus Animation, mit Farbe und Transparenz.
/// Wählt bei `variant` das Bild dieser Variante, sonst das Grundbild.
#[allow(clippy::too_many_arguments)]
pub fn pose_part(
    part: &Part,
    transform: &mut Transform,
    materials: &mut Assets<StandardMaterial>,
    offset: Vec2,
    angle_degrees: f32,
    variant: Option<&str>,
    tint: Color,
    light: f32,
) {
    let mut pose = part.base;
    pose.translation += offset.extend(0.0);
    pose.rotation *= Quat::from_rotation_z(angle_degrees.to_radians());
    *transform = pose;

    let image = variant.and_then(|v| part.variants.get(v)).unwrap_or(&part.image);
    let alpha = tint.alpha();
    let base = if part.tint { tint } else { Color::WHITE };
    // Die Figuren sind unbeleuchtet gemalt; Tageslicht dunkelt sie hier ab.
    let srgb = base.to_srgba();
    let color = Color::srgba(srgb.red * light, srgb.green * light, srgb.blue * light, alpha);

    // Kantenglättung per Abdeckung lässt halbdurchsichtige Figuren fast verschwinden;
    // Figuren aus der anderen Ebene werden deshalb echt durchsichtig gemischt.
    let alpha_mode = if alpha < 1.0 {
        AlphaMode::Blend
    } else {
        AlphaMode::AlphaToCoverage
    };
    let Some(material) = materials.get(&part.material) else {
        return;
    };
    let same_image = material.base_color_texture.as_ref() == Some(image);
    if same_image && material.base_color == color && material.alpha_mode == alpha_mode {
        return;
    }
    if let Some(mut material) = materials.get_mut(&part.material) {
        material.base_color_texture = Some(image.clone());
        material.base_color = color;
        material.alpha_mode = alpha_mode;
    }
}
