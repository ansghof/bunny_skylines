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
        app.init_asset::<RigDefinition>().init_asset_loader::<RigLoader>();
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
pub struct RigRoot;

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
pub fn spawn_rig(
    commands: &mut Commands,
    asset_server: &AssetServer,
    rig: &RigDefinition,
    folder: &str,
    owner: Entity,
) -> Entity {
    let root = commands
        .spawn((
            RigRoot,
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
    root
}
