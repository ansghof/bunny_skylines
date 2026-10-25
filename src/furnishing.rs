//! Einrichtung im Bau: Heunester in Schlafkammern, Futterhaufen, die mit dem Vorrat wachsen,
//! und ein Lichtschein unter jedem Eingang. Nur in der Untergrund-Ansicht zu sehen.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::colony::{Colony, FOOD_PER_STORAGE_TILE};
use crate::map::{Chamber, Grid, Layer, TilePos, ViewLayer};

const FOLDER: &str = "einrichtung";
/// Bilder sind in doppelter Spielgröße gemalt.
const PIXEL_SCALE: f32 = 0.5;

pub struct FurnishingPlugin;

impl Plugin for FurnishingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Placed>()
            .add_systems(Update, (place_furnishing, show_with_view).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
    Nest,
    Pile,
    Light,
}

#[derive(Component)]
struct Furnishing;

/// Welche Einrichtung auf welchem Feld steht.
#[derive(Resource, Default)]
struct Placed(HashMap<(TilePos, Kind), Entity>);

fn place_furnishing(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    grid: Res<Grid>,
    colony: Res<Colony>,
    view: Res<ViewLayer>,
    mut placed: ResMut<Placed>,
    mut sprites: Query<&mut Sprite, With<Furnishing>>,
) {
    if !grid.is_changed() && !colony.is_changed() {
        return;
    }

    // Was jetzt dastehen soll; Futterhaufen bekommen ihr Bild nach Füllstand.
    let mut wanted: HashMap<(TilePos, Kind), Option<&str>> = HashMap::new();
    let mut food_left = colony.food;
    for p in Grid::positions() {
        let tile = grid.under(p);
        if tile.dug.is_some() {
            match tile.chamber {
                Some(Chamber::Sleeping) => {
                    wanted.insert((p, Kind::Nest), Some("heunest.png"));
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
                    if image.is_some() {
                        wanted.insert((p, Kind::Pile), image);
                    }
                }
                None => {}
            }
        }
        if grid.surface(p).entrance {
            wanted.insert((p, Kind::Light), Some("lichtschein.png"));
        }
    }

    placed.0.retain(|key, entity| {
        let keep = wanted.contains_key(key);
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
        let Some(image) = image else { continue };
        let handle: Handle<Image> = asset_server.load(format!("{FOLDER}/{image}"));
        if let Some(&entity) = placed.0.get(&key) {
            if let Ok(mut sprite) = sprites.get_mut(entity)
                && sprite.image != handle
            {
                sprite.image = handle;
            }
            continue;
        }
        let (pos, kind) = key;
        // Lichtschein unter allem, Nester und Haufen über dem Boden, aber unter den Kaninchen.
        let z = if kind == Kind::Light { 0.5 } else { 1.0 };
        let entity = commands
            .spawn((
                Furnishing,
                Sprite::from_image(handle),
                Transform::from_translation(pos.center().extend(z)).with_scale(Vec3::splat(PIXEL_SCALE)),
                visibility,
            ))
            .id();
        placed.0.insert(key, entity);
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
