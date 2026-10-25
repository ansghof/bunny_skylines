//! Einrichtung im Bau: Heunester in Schlafkammern, Futterhaufen, die mit dem Vorrat wachsen,
//! und ein Lichtschein unter jedem Eingang. Nur in der Untergrund-Ansicht zu sehen.
//! Nester und Lichtschein liegen flach am Boden, Futterhaufen stehen als Aufsteller.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::camera::{Orbit, figure_layer};
use crate::colony::{Colony, FOOD_PER_STORAGE_TILE};
use crate::ground::{depth_y, to_world3};
use crate::map::{Chamber, Grid, Layer, TilePos, ViewLayer};

const FOLDER: &str = "einrichtung";
/// Bilder sind in doppelter Spielgröße gemalt.
const PIXEL_SCALE: f32 = 0.5;

pub struct FurnishingPlugin;

impl Plugin for FurnishingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Placed>()
            .add_systems(Startup, prepare_assets)
            .add_systems(Update, (place_furnishing, show_with_view, face_camera).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
    Nest,
    Pile,
    Light,
}

/// Bild und Größe in Bildpixeln, wie in `art_src/einrichtung` gezeichnet.
const IMAGES: [(&str, Vec2); 5] = [
    ("heunest.png", Vec2::new(112.0, 72.0)),
    ("futter_klein.png", Vec2::new(80.0, 56.0)),
    ("futter_mittel.png", Vec2::new(104.0, 72.0)),
    ("futter_voll.png", Vec2::new(124.0, 88.0)),
    ("lichtschein.png", Vec2::new(128.0, 128.0)),
];

#[derive(Component)]
struct Furnishing;

/// Steht aufrecht und dreht sich zur Kamera.
#[derive(Component)]
struct Upright;

#[derive(Resource)]
struct FurnishingAssets {
    /// Flach liegendes Viereck mit Mittelpunkt im Ursprung.
    flat: Handle<Mesh>,
    /// Aufrechtes Viereck, unten in der Mitte verankert.
    upright: Handle<Mesh>,
    materials: HashMap<&'static str, Handle<StandardMaterial>>,
}

/// Welche Einrichtung auf welchem Feld steht und mit welchem Bild.
#[derive(Resource, Default)]
struct Placed(HashMap<(TilePos, Kind), (Entity, &'static str)>);

fn prepare_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let materials = IMAGES
        .iter()
        .map(|(file, _)| {
            let light = *file == "lichtschein.png";
            let material = materials.add(StandardMaterial {
                base_color_texture: Some(asset_server.load(format!("{FOLDER}/{file}"))),
                // Der Lichtschein hellt auf, statt zu verdecken.
                alpha_mode: if light { AlphaMode::Add } else { AlphaMode::AlphaToCoverage },
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            (*file, material)
        })
        .collect();
    commands.insert_resource(FurnishingAssets {
        flat: meshes.add(Mesh::from(Rectangle::new(1.0, 1.0)).rotated_by(Quat::from_rotation_x(-FRAC_PI_2))),
        upright: meshes.add(Mesh::from(Rectangle::new(1.0, 1.0)).translated_by(Vec3::new(0.0, 0.5, 0.0))),
        materials,
    });
}

fn image_size(file: &str) -> Vec2 {
    IMAGES.iter().find(|(f, _)| *f == file).map_or(Vec2::ONE, |(_, size)| *size) * PIXEL_SCALE
}

fn place_furnishing(
    mut commands: Commands,
    assets: Res<FurnishingAssets>,
    grid: Res<Grid>,
    colony: Res<Colony>,
    view: Res<ViewLayer>,
    mut placed: ResMut<Placed>,
) {
    if !grid.is_changed() && !colony.is_changed() {
        return;
    }

    // Was jetzt dastehen soll; Futterhaufen bekommen ihr Bild nach Füllstand.
    let mut wanted: HashMap<(TilePos, Kind), &'static str> = HashMap::new();
    let mut food_left = colony.food;
    for p in Grid::positions() {
        let tile = grid.under(p);
        if tile.dug.is_some() {
            match tile.chamber {
                Some(Chamber::Sleeping) => {
                    wanted.insert((p, Kind::Nest), "heunest.png");
                }
                Some(Chamber::Storage) => {
                    // Der Vorrat füllt die Felder der Reihe nach auf.
                    let fill = (food_left / FOOD_PER_STORAGE_TILE).clamp(0.0, 1.0);
                    food_left -= fill * FOOD_PER_STORAGE_TILE;
                    let image = match fill {
                        f if f <= 0.0 => None,
                        f if f < 0.34 => Some("futter_klein.png"),
                        f if f < 0.67 => Some("futter_mittel.png"),
                        _ => Some("futter_voll.png"),
                    };
                    if let Some(image) = image {
                        wanted.insert((p, Kind::Pile), image);
                    }
                }
                None => {}
            }
        }
        if grid.surface(p).entrance {
            wanted.insert((p, Kind::Light), "lichtschein.png");
        }
    }

    // Weg, was nicht mehr gebraucht wird oder ein anderes Bild braucht.
    placed.0.retain(|key, (entity, image)| {
        let keep = wanted.get(key) == Some(image);
        if !keep {
            commands.entity(*entity).despawn();
        }
        keep
    });

    let visibility = if view.0 == Layer::Underground {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for (key, image) in wanted {
        if placed.0.contains_key(&key) {
            continue;
        }
        let (pos, kind) = key;
        let floor = grid.under(pos).dug.map_or(0.0, depth_y);
        // Leicht über dem Boden, damit nichts flimmert.
        let lift = if kind == Kind::Light { 0.5 } else { 1.0 };
        let size = image_size(image);
        let at = Transform::from_translation(to_world3(pos.center(), floor + lift));
        let mut entity = commands.spawn((
            Furnishing,
            MeshMaterial3d(assets.materials[image].clone()),
            NotShadowCaster,
            visibility,
        ));
        match kind {
            // Futterhaufen stehen wie Figuren und werden mit ihnen gezeichnet, damit sie aus
            // jedem Winkel zur Kamera schauen können, ohne in der Kammerwand zu verschwinden.
            Kind::Pile => entity.insert((
                Mesh3d(assets.upright.clone()),
                Upright,
                figure_layer(),
                at.with_scale(size.extend(1.0)),
            )),
            // Flache Vierecke liegen in x-z: die Bildhöhe gehört auf z.
            Kind::Nest | Kind::Light => entity.insert((
                Mesh3d(assets.flat.clone()),
                at.with_scale(Vec3::new(size.x, 1.0, size.y)),
            )),
        };
        placed.0.insert(key, (entity.id(), image));
    }
}

fn show_with_view(view: Res<ViewLayer>, mut furnishing: Query<&mut Visibility, With<Furnishing>>) {
    if !view.is_changed() {
        return;
    }
    let visibility = if view.0 == Layer::Underground {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut v in &mut furnishing {
        *v = visibility;
    }
}

fn face_camera(orbit: Res<Orbit>, mut upright: Query<&mut Transform, With<Upright>>) {
    let facing = orbit.billboard();
    for mut transform in &mut upright {
        transform.rotation = facing;
    }
}
