//! Schräge Kamera, die um einen Punkt auf dem Boden kreist:
//! WASD/Pfeiltasten oder mittlere Maustaste verschieben, rechte Maustaste dreht und neigt,
//! Mausrad zoomt. Läuft in Echtzeit, also auch während der Pause.

use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::ground::floor_y_for_view;
use crate::map::ViewLayer;

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
            .add_systems(Update, (pan_keyboard, pan_and_rotate_mouse, zoom, apply_orbit).chain());
    }
}

#[derive(Component)]
pub struct MainCamera;

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
    /// Drehung für Aufsteller: Sie schauen genau zur Kamera, damit ihre Ebenen nicht verrutschen.
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
}

fn update_cursor(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    view: Res<ViewLayer>,
    mut cursor: ResMut<CursorWorld>,
) {
    let (camera, transform) = *camera;
    // Schnittpunkt des Mausstrahls mit der Bodenhöhe der aktuellen Ansicht.
    let floor = floor_y_for_view(view.0);
    cursor.0 = window
        .cursor_position()
        .and_then(|position| camera.viewport_to_world(transform, position).ok())
        .and_then(|ray| {
            let distance = (floor - ray.origin.y) / ray.direction.y;
            (distance > 0.0).then(|| {
                let hit = ray.origin + *ray.direction * distance;
                Vec2::new(hit.x, -hit.z)
            })
        });
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

fn apply_orbit(orbit: Res<Orbit>, mut camera: Single<&mut Transform, With<MainCamera>>) {
    let offset = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    ) * orbit.distance;
    **camera = Transform::from_translation(orbit.focus + offset).looking_at(orbit.focus, Vec3::Y);
}
