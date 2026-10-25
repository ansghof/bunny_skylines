//! Figuren aus Einzelteilen: Die Rig-Datei (`*.rig.ron`) beschreibt, welche Bilder
//! wo sitzen und um welchen Punkt sie sich drehen. Animationen sprechen nur Teilnamen an.

use std::collections::HashMap;

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use serde::Deserialize;

pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<RigDefinition>()
            .init_asset_loader::<RigLoader>()
            .add_systems(Update, (reload_rigs, attach_rigs).chain());
    }
}

/// Welche Rig-Datei eine Figur benutzt. Die Teile werden aufgebaut, sobald die Datei geladen ist.
#[derive(Component)]
pub struct RigSource {
    pub rig: Handle<RigDefinition>,
    /// Ordner der Bilder, relativ zu `assets/`.
    pub folder: &'static str,
}

#[derive(Component)]
struct HasRig;

fn attach_rigs(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    rigs: Res<Assets<RigDefinition>>,
    owners: Query<(Entity, &RigSource), Without<HasRig>>,
) {
    for (owner, source) in &owners {
        if let Some(definition) = rigs.get(&source.rig) {
            spawn_rig(&mut commands, &asset_server, definition, source.folder, owner);
            commands.entity(owner).insert(HasRig);
        }
    }
}

/// Wird eine Rig-Datei gespeichert, bauen alle Figuren mit dieser Datei ihre Teile neu auf.
fn reload_rigs(
    mut commands: Commands,
    mut events: MessageReader<AssetEvent<RigDefinition>>,
    roots: Query<(Entity, &ChildOf), With<RigRoot>>,
    owners: Query<&RigSource>,
) {
    for event in events.read() {
        let AssetEvent::Modified { id } = event else {
            continue;
        };
        for (root, parent) in &roots {
            if owners.get(parent.0).is_ok_and(|source| source.rig.id() == *id) {
                commands.entity(root).despawn();
                commands.entity(parent.0).remove::<HasRig>();
            }
        }
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
}

/// Baut die Teile einer Figur als Kinder von `owner` auf. Bildpfade sind relativ zu `folder`.
fn spawn_rig(commands: &mut Commands, asset_server: &AssetServer, rig: &RigDefinition, folder: &str, owner: Entity) {
    let root = commands
        .spawn((
            RigRoot { pixel_scale: rig.pixel_scale },
            Transform::from_scale(Vec3::splat(rig.pixel_scale)),
            Visibility::default(),
            ChildOf(owner),
        ))
        .id();

    for part in &rig.parts {
        let image = asset_server.load(format!("{folder}/{}", part.image));
        let variants = part
            .variants
            .iter()
            .map(|(name, file)| (name.clone(), asset_server.load(format!("{folder}/{file}"))))
            .collect();
        let base = Transform {
            translation: Vec3::new(part.position.0, part.position.1, part.layer as f32 * 0.1),
            rotation: Quat::from_rotation_z(part.rotation.to_radians()),
            ..default()
        };
        commands.spawn((
            Sprite::from_image(image.clone()),
            Anchor(Vec2::new(part.pivot.0 - 0.5, part.pivot.1 - 0.5)),
            base,
            Part {
                name: part.name.clone(),
                base,
                tint: part.tint,
                image,
                variants,
            },
            ChildOf(root),
        ));
    }
}

/// Grundstellung eines Teils plus Animation, mit Farbe und Transparenz.
/// Wählt bei `variant` das Bild dieser Variante, sonst das Grundbild.
pub fn pose_part(
    part: &Part,
    transform: &mut Transform,
    sprite: &mut Sprite,
    offset: Vec2,
    angle_degrees: f32,
    variant: Option<&str>,
    tint: Color,
) {
    let mut pose = part.base;
    pose.translation += offset.extend(0.0);
    pose.rotation *= Quat::from_rotation_z(angle_degrees.to_radians());
    *transform = pose;

    let image = variant.and_then(|v| part.variants.get(v)).unwrap_or(&part.image);
    if sprite.image != *image {
        sprite.image = image.clone();
    }
    let alpha = tint.alpha();
    let color = if part.tint { tint } else { Color::WHITE.with_alpha(alpha) };
    if sprite.color != color {
        sprite.color = color;
    }
}
