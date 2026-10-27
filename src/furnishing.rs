//! Einrichtung im Bau: Heunester in Schlafkammern, Futterhaufen, die mit dem Vorrat wachsen,
//! und ein Lichtschein unter jedem Eingang, alles in der Untergrund-Ansicht. Dazu Kleefelder an der Oberfläche.
//! Nester und Lichtschein liegen flach am Boden, Futterhaufen stehen als Aufsteller.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::camera::{Orbit, figure_layer};
use crate::colony::Colony;
use crate::ground::{depth_y, to_world3};
use crate::map::{Chamber, Feature, Grid, Layer, TilePos, ViewLayer};

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
    Clover,
    /// Ein Bau an der Oberfläche: Hecke, Fluchtloch, Sonnenplatz, Kräuter, Spielwiese.
    Built(Feature),
    /// Erdpfropf im Gang, offen oder verschlossen.
    Plug,
    /// Platz der Nachtwache.
    Watch,
    /// Ein Baum im Wald.
    Tree,
    /// Ein Stück alter Steinwall.
    Stones,
    /// Ein Stück Trampelpfad; die Zahl sagt, zu welchen Nachbarn er führt (N 1, O 2, S 4, W 8).
    Trail(u8),
}

impl Kind {
    /// In welcher Ansicht diese Einrichtung zu sehen ist.
    fn layer(self) -> Layer {
        match self {
            Kind::Clover | Kind::Built(_) | Kind::Tree | Kind::Stones | Kind::Trail(_) => Layer::Surface,
            Kind::Nest | Kind::Pile | Kind::Light | Kind::Plug | Kind::Watch => Layer::Underground,
        }
    }

    /// Steht aufrecht wie eine Figur, statt flach am Boden zu liegen.
    fn upright(self) -> bool {
        matches!(self, Kind::Pile | Kind::Plug | Kind::Tree | Kind::Stones | Kind::Built(Feature::Hedge))
    }
}

fn visibility_in(layer: Layer, view: Layer) -> Visibility {
    if layer == view { Visibility::Visible } else { Visibility::Hidden }
}

/// Bild und Größe in Bildpixeln, wie in `art_src/einrichtung` gezeichnet.
const IMAGES: [(&str, Vec2); 24] = [
    ("heunest.png", Vec2::new(112.0, 72.0)),
    ("futter_klein.png", Vec2::new(80.0, 56.0)),
    ("futter_mittel.png", Vec2::new(104.0, 72.0)),
    ("futter_voll.png", Vec2::new(124.0, 88.0)),
    ("lichtschein.png", Vec2::new(128.0, 128.0)),
    ("klee.png", Vec2::new(128.0, 128.0)),
    ("hecke.png", Vec2::new(128.0, 96.0)),
    ("fluchtloch.png", Vec2::new(96.0, 64.0)),
    ("sonnenplatz.png", Vec2::new(128.0, 128.0)),
    ("kraeuter.png", Vec2::new(128.0, 128.0)),
    ("spielwiese.png", Vec2::new(128.0, 128.0)),
    ("erdpfropf_offen.png", Vec2::new(128.0, 80.0)),
    ("erdpfropf_zu.png", Vec2::new(128.0, 96.0)),
    ("nachtwache.png", Vec2::new(128.0, 128.0)),
    // Bäume doppelt so groß wie gezeichnet, damit sie über die Wiese ragen.
    ("baum.png", Vec2::new(320.0, 440.0)),
    ("steinwall.png", Vec2::new(140.0, 114.0)),
    ("moehren.png", Vec2::new(128.0, 128.0)),
    // Der Festplatz ist 3 × 3 Felder groß.
    ("festplatz.png", Vec2::new(384.0, 384.0)),
    ("pfad_punkt.png", Vec2::new(128.0, 128.0)),
    ("pfad_ende.png", Vec2::new(128.0, 128.0)),
    ("pfad_gerade.png", Vec2::new(128.0, 128.0)),
    ("pfad_kurve.png", Vec2::new(128.0, 128.0)),
    ("pfad_t.png", Vec2::new(128.0, 128.0)),
    ("pfad_kreuz.png", Vec2::new(128.0, 128.0)),
];

/// Die Pfadstücke, wie gezeichnet (oben = Norden), mit den Nachbarn, zu denen sie führen.
const TRAIL_PIECES: [(&str, u8); 6] = [
    ("pfad_punkt.png", 0),
    ("pfad_ende.png", 1),
    ("pfad_gerade.png", 1 | 4),
    ("pfad_kurve.png", 1 | 2),
    ("pfad_t.png", 1 | 2 | 4),
    ("pfad_kreuz.png", 15),
];

/// Ein Mal im Uhrzeigersinn gedreht: aus Norden wird Osten, aus Osten Süden …
fn turn(mask: u8) -> u8 {
    ((mask << 1) | (mask >> 3)) & 15
}

/// Welches Pfadstück, wie oft im Uhrzeigersinn gedreht, zu diesen Nachbarn passt.
fn trail_piece(mask: u8) -> (&'static str, u8) {
    for (image, base) in TRAIL_PIECES {
        let mut turned = base;
        for turns in 0..4 {
            if turned == mask {
                return (image, turns);
            }
            turned = turn(turned);
        }
    }
    ("pfad_punkt.png", 0)
}

/// Zu welchen Nachbarn ein Pfadfeld weiterführt: anderen Pfaden und Eingängen.
fn trail_mask(grid: &Grid, p: TilePos) -> u8 {
    let leads = |dx: i32, dy: i32| {
        let q = TilePos::new(p.x + dx, p.y + dy);
        q.in_bounds() && (grid.surface(q).trail || grid.surface(q).entrance)
    };
    // Norden ist +y auf der Karte.
    u8::from(leads(0, 1)) | u8::from(leads(1, 0)) << 1 | u8::from(leads(0, -1)) << 2 | u8::from(leads(-1, 0)) << 3
}

fn feature_image(feature: Feature) -> &'static str {
    match feature {
        Feature::Hedge => "hecke.png",
        Feature::BoltHole => "fluchtloch.png",
        Feature::SunSpot => "sonnenplatz.png",
        Feature::Herbs => "kraeuter.png",
        Feature::Playground => "spielwiese.png",
        Feature::Carrots => "moehren.png",
        Feature::Festival => "festplatz.png",
    }
}

#[derive(Component)]
struct Furnishing(Layer);

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
                    let fill = (food_left / colony.storage_per_tile).clamp(0.0, 1.0);
                    food_left -= fill * colony.storage_per_tile;
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
                Some(Chamber::Story | Chamber::Nursery) | None => {}
            }
        }
        // Der Erdpfropf steht quer im Gang, damit man ihn gut sieht; geschlossen als volle Wand.
        if tile.plug && tile.dug.is_some() {
            let image = if grid.plugs_closed { "erdpfropf_zu.png" } else { "erdpfropf_offen.png" };
            wanted.insert((p, Kind::Plug), image);
        }
        if tile.watch && tile.dug.is_some() {
            wanted.insert((p, Kind::Watch), "nachtwache.png");
        }
        let surface = grid.surface(p);
        if surface.tree {
            wanted.insert((p, Kind::Tree), "baum.png");
        }
        if surface.stones {
            wanted.insert((p, Kind::Stones), "steinwall.png");
        }
        if surface.trail && !surface.entrance {
            let mask = trail_mask(&grid, p);
            wanted.insert((p, Kind::Trail(mask)), trail_piece(mask).0);
        }
        if surface.entrance {
            wanted.insert((p, Kind::Light), "lichtschein.png");
        }
        // Abgefressener Klee ist nur noch Farbe am Boden, bis er nachwächst.
        if surface.clover && surface.grass > 0.25 {
            wanted.insert((p, Kind::Clover), "klee.png");
        }
        // Kräuter und Möhren sieht man nur, solange sie nicht abgefressen sind; alles andere immer.
        // Ein großer Bau hat ein einziges Bild, auf seinem Mittelfeld.
        let big_part = surface.part_of.is_some_and(|center| center != p);
        if let Some(feature) = surface.feature
            && !big_part
            && (!matches!(feature, Feature::Herbs | Feature::Carrots) || surface.grass > 0.25)
        {
            wanted.insert((p, Kind::Built(feature)), feature_image(feature));
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

    for (key, image) in wanted {
        if placed.0.contains_key(&key) {
            continue;
        }
        let (pos, kind) = key;
        let floor = match kind.layer() {
            Layer::Surface => 0.0,
            Layer::Underground => grid.under(pos).dug.map_or(0.0, depth_y),
        };
        // Leicht über dem Boden, damit nichts flimmert.
        let lift = match kind {
            // Pfade ganz unten, darüber Klee, Kräuter und Bauten.
            Kind::Trail(_) => 0.3,
            Kind::Light | Kind::Clover | Kind::Built(_) => 0.5,
            _ => 1.0,
        };
        let size = image_size(image);
        let at = Transform::from_translation(to_world3(pos.center(), floor + lift));
        let mut entity = commands.spawn((
            Furnishing(kind.layer()),
            MeshMaterial3d(assets.materials[image].clone()),
            NotShadowCaster,
            visibility_in(kind.layer(), view.0),
        ));
        if kind.upright() {
            entity.insert((Mesh3d(assets.upright.clone()), Upright, at.with_scale(size.extend(1.0))));
            // Futterhaufen stehen in Kammern und werden mit den Figuren gezeichnet, damit sie aus
            // jedem Winkel zur Kamera schauen können, ohne in der Kammerwand zu verschwinden.
            if matches!(kind, Kind::Pile | Kind::Plug) {
                entity.insert(figure_layer());
            }
        } else {
            // Flache Vierecke liegen in x-z: die Bildhöhe gehört auf z. Pfadstücke werden gedreht,
            // im Uhrzeigersinn von oben gesehen, damit sie zu ihren Nachbarn führen.
            let turns = if let Kind::Trail(mask) = kind { trail_piece(mask).1 } else { 0 };
            let rotation = Quat::from_rotation_y(-(turns as f32) * std::f32::consts::FRAC_PI_2);
            entity.insert((
                Mesh3d(assets.flat.clone()),
                at.with_rotation(rotation).with_scale(Vec3::new(size.x, 1.0, size.y)),
            ));
        }
        placed.0.insert(key, (entity.id(), image));
    }
}

fn show_with_view(view: Res<ViewLayer>, mut furnishing: Query<(&Furnishing, &mut Visibility)>) {
    if !view.is_changed() {
        return;
    }
    for (furnishing, mut v) in &mut furnishing {
        *v = visibility_in(furnishing.0, view.0);
    }
}

fn face_camera(orbit: Res<Orbit>, mut upright: Query<&mut Transform, With<Upright>>) {
    let facing = orbit.billboard();
    for mut transform in &mut upright {
        transform.rotation = facing;
    }
}
