//! Spielkamera: Verschieben mit WASD/Pfeiltasten oder gedrückter mittlerer Maustaste,
//! Zoomen mit dem Mausrad. Läuft in Echtzeit, also auch während der Pause.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

const PAN_SPEED: f32 = 800.0;
const ZOOM_STEP: f32 = 0.1;
const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 4.0;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CursorWorld>()
            .add_systems(Startup, spawn_camera)
            .add_systems(PreUpdate, update_cursor)
            .add_systems(Update, (pan_keyboard, pan_mouse, zoom));
    }
}

#[derive(Component)]
pub struct MainCamera;

/// Mausposition in Weltkoordinaten, `None` wenn die Maus außerhalb des Fensters ist.
#[derive(Resource, Default)]
pub struct CursorWorld(pub Option<Vec2>);

fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));
}

fn update_cursor(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut cursor: ResMut<CursorWorld>,
) {
    let (camera, transform) = *camera;
    cursor.0 = window
        .cursor_position()
        .and_then(|position| camera.viewport_to_world_2d(transform, position).ok());
}

fn current_scale(projection: &Projection) -> f32 {
    match projection {
        Projection::Orthographic(ortho) => ortho.scale,
        _ => 1.0,
    }
}

fn pan_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    mut camera: Single<(&mut Transform, &Projection), With<MainCamera>>,
) {
    let mut direction = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        direction.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        direction.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        direction.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        direction.x += 1.0;
    }
    if direction == Vec2::ZERO {
        return;
    }

    // Beim Herauszoomen schneller verschieben, damit es sich gleich anfühlt.
    let scale = current_scale(camera.1);
    let delta = direction.normalize() * PAN_SPEED * scale * time.delta_secs();
    camera.0.translation += delta.extend(0.0);
}

fn pan_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut camera: Single<(&mut Transform, &Projection), With<MainCamera>>,
) {
    if !buttons.pressed(MouseButton::Middle) || motion.delta == Vec2::ZERO {
        return;
    }

    // Bildschirm-Y zeigt nach unten, Welt-Y nach oben.
    let scale = current_scale(camera.1);
    camera.0.translation.x -= motion.delta.x * scale;
    camera.0.translation.y += motion.delta.y * scale;
}

fn zoom(scroll: Res<AccumulatedMouseScroll>, mut projection: Single<&mut Projection, With<MainCamera>>) {
    if scroll.delta.y == 0.0 {
        return;
    }
    let Projection::Orthographic(ortho) = projection.as_mut() else {
        return;
    };

    // Touchpads liefern Pixel statt Rasterstufen.
    let steps = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 100.0,
    };
    ortho.scale = (ortho.scale * (1.0 - steps * ZOOM_STEP)).clamp(MIN_ZOOM, MAX_ZOOM);
}
