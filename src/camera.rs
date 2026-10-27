//! Schräge Kamera, die um einen Punkt auf dem Boden kreist:
//! WASD/Pfeiltasten oder mittlere Maustaste verschieben, rechte Maustaste dreht und neigt,
//! Mausrad zoomt. Läuft in Echtzeit, also auch während der Pause.

use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::ground::pick;
use crate::map::{Grid, Layer, ViewLayer};

const PAN_SPEED: f32 = 1.0;
const ROTATE_SPEED: f32 = 0.006;
const ZOOM_STEP: f32 = 0.1;
const MIN_DISTANCE: f32 = 250.0;
const MAX_DISTANCE: f32 = 3200.0;
const MIN_PITCH: f32 = 0.35;
const MAX_PITCH: f32 = 1.45;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CursorWorld>()
            .insert_resource(Orbit {
                focus: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.95,
                distance: 1100.0,
            })
            .add_systems(Startup, spawn_camera)
            .add_systems(PreUpdate, update_cursor)
            .add_systems(Update, (pan_keyboard, pan_and_rotate_mouse, zoom, apply_orbit).chain())
            .add_systems(
                PostUpdate,
                place_figures
                    .after(bevy::transform::TransformSystems::Propagate)
                    .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
            );
    }
}

#[derive(Component)]
pub struct MainCamera;

/// Zweite Kamera, die Figuren aus der anderen Ebene über den Boden hinweg zeichnet: Von oben
/// sieht man die Kaninchen in den Gängen durch die Erde.
#[derive(Component)]
struct FigureCamera;

/// Ebene für alles, was die Figuren-Kamera zeichnet.
pub const FIGURE_LAYER: usize = 1;

/// Ein Aufsteller, der zur Kamera schaut: Teile von Figuren, getragene Beute, Futterhaufen.
#[derive(Component)]
pub struct Figure;

pub fn figure_layer() -> (Figure, RenderLayers) {
    (Figure, RenderLayers::layer(0))
}

/// So weit rücken Figuren auf ihrem Sichtstrahl zur Kamera hin. Ein Aufsteller, der voll zur
/// Kamera schaut, kippt bei schräger Sicht nach hinten; so steckt er nicht in der Wand oder im
/// Boden hinter sich, und Wände davor verdecken ihn trotzdem.
const FIGURE_PULL: f32 = 30.0;

/// Figuren in der angezeigten Ebene zeichnet die Hauptkamera mit dem Gelände zusammen, ein Stück
/// zur Kamera gerückt: Jeder Punkt wandert auf seinem Sichtstrahl, auf dem Bildschirm bleiben
/// Ort und Größe gleich, nur die Tiefe ändert sich. Figuren aus den Gängen zeigt bei Sicht von
/// oben die Figuren-Kamera durch die Erde hindurch.
fn place_figures(
    view: Res<ViewLayer>,
    camera: Single<&GlobalTransform, (With<MainCamera>, Without<Figure>)>,
    parents: Query<&ChildOf>,
    anchors: Query<&GlobalTransform, Without<Figure>>,
    mut figures: Query<(Entity, &mut GlobalTransform, &mut RenderLayers), With<Figure>>,
) {
    let eye = camera.translation();
    for (entity, mut global, mut layers) in &mut figures {
        // Gemessen wird an den Füßen: am obersten Vorfahren, sonst an der Figur selbst.
        let mut top = entity;
        while let Ok(parent) = parents.get(top) {
            top = parent.parent();
        }
        let foot = anchors.get(top).map_or(global.translation(), |anchor| anchor.translation());
        let in_tunnel = foot.y < -2.0;
        let wanted = if view.0 == Layer::Surface && in_tunnel {
            RenderLayers::layer(FIGURE_LAYER)
        } else {
            RenderLayers::layer(0)
        };
        if *layers != wanted {
            *layers = wanted;
        }
        if layers.intersects(&RenderLayers::layer(FIGURE_LAYER)) {
            continue;
        }
        let distance = foot.distance(eye);
        let k = ((distance - FIGURE_PULL) / distance.max(1.0)).clamp(0.5, 1.0);
        let pull = bevy::math::Affine3A::from_translation(eye * (1.0 - k)) * bevy::math::Affine3A::from_scale(Vec3::splat(k));
        *global = GlobalTransform::from(pull * global.affine());
    }
}

/// Wo die Kamera hinschaut und von wo.
#[derive(Resource)]
pub struct Orbit {
    pub focus: Vec3,
    /// Drehung um die senkrechte Achse; 0 = Blick nach Norden.
    pub yaw: f32,
    /// Neigung über dem Boden.
    pub pitch: f32,
    pub distance: f32,
}

impl Orbit {
    /// Drehung für Aufsteller: Sie schauen genau zur Kamera, auch von schräg oben,
    /// damit sie nie flach oder verzerrt wirken.
    pub fn billboard(&self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(-self.pitch)
    }

    /// Die Richtung „rechts auf dem Bildschirm“ in Kartenkoordinaten.
    pub fn screen_right(&self) -> Vec2 {
        Vec2::new(self.yaw.cos(), self.yaw.sin())
    }

    /// Vorwärts und rechts auf dem Boden, aus Sicht der Kamera.
    fn ground_axes(&self) -> (Vec3, Vec3) {
        let forward = Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos());
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        (forward, right)
    }
}

/// Mausposition auf der Karte in Kartenkoordinaten, `None` wenn die Maus außerhalb des Fensters ist.
#[derive(Resource, Default)]
pub struct CursorWorld(pub Option<Vec2>);

fn spawn_camera(mut commands: Commands) {
    // Ohne Tonemapping bleiben die gemalten Farben der Figuren so, wie sie gezeichnet sind.
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        DebandDither::Disabled,
        MainCamera,
        Transform::default(),
    ));
    // Zeichnet danach über das Bild der Hauptkamera, mit eigener Tiefe: Der Boden verdeckt nichts.
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Tonemapping::None,
        DebandDither::Disabled,
        RenderLayers::layer(FIGURE_LAYER),
        FigureCamera,
        // Die Anzeige gehört über alles.
        IsDefaultUiCamera,
        Transform::default(),
    ));
}

pub fn update_cursor(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    view: Res<ViewLayer>,
    grid: Res<Grid>,
    mut cursor: ResMut<CursorWorld>,
) {
    let (camera, transform) = *camera;
    cursor.0 = window
        .cursor_position()
        .and_then(|position| camera.viewport_to_world(transform, position).ok())
        .and_then(|ray| pick(&grid, view.0, ray));
}

fn pan_keyboard(keys: Res<ButtonInput<KeyCode>>, time: Res<Time<Real>>, mut orbit: ResMut<Orbit>) {
    let mut input = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        input.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        input.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        input.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        input.x += 1.0;
    }
    if input == Vec2::ZERO {
        return;
    }
    // Weiter weg verschiebt schneller, damit es sich gleich anfühlt.
    let (forward, right) = orbit.ground_axes();
    let step = PAN_SPEED * orbit.distance * time.delta_secs();
    let input = input.normalize();
    orbit.focus += (forward * input.y + right * input.x) * step;
}

fn pan_and_rotate_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut orbit: ResMut<Orbit>,
) {
    if motion.delta == Vec2::ZERO {
        return;
    }
    if buttons.pressed(MouseButton::Middle) {
        let (forward, right) = orbit.ground_axes();
        let scale = orbit.distance / 900.0;
        orbit.focus += (-right * motion.delta.x + forward * motion.delta.y) * scale;
    }
    if buttons.pressed(MouseButton::Right) {
        orbit.yaw -= motion.delta.x * ROTATE_SPEED;
        orbit.pitch = (orbit.pitch + motion.delta.y * ROTATE_SPEED).clamp(MIN_PITCH, MAX_PITCH);
    }
}

fn zoom(scroll: Res<AccumulatedMouseScroll>, mut orbit: ResMut<Orbit>) {
    if scroll.delta.y == 0.0 {
        return;
    }
    // Touchpads liefern Pixel statt Rasterstufen.
    let steps = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 100.0,
    };
    orbit.distance = (orbit.distance * (1.0 - steps * ZOOM_STEP)).clamp(MIN_DISTANCE, MAX_DISTANCE);
}

fn apply_orbit(orbit: Res<Orbit>, mut cameras: Query<&mut Transform, Or<(With<MainCamera>, With<FigureCamera>)>>) {
    let offset = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    ) * orbit.distance;
    let view = Transform::from_translation(orbit.focus + offset).looking_at(orbit.focus, Vec3::Y);
    // Beide Kameras müssen exakt gleich stehen, sonst rutschen Figuren gegen den Boden.
    for mut camera in &mut cameras {
        *camera = view;
    }
}
