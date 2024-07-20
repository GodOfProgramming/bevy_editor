use std::ops::{Deref, DerefMut};

use bevy::{
    input::{keyboard::KeyboardInput, mouse::MouseMotion},
    prelude::*,
    window::CursorGrabMode,
};
use bevy_editor::EditorPlugin;

const UP: Vec3 = Vec3::Y;

const MOVE_SPEED: f32 = 0.05;
const MOUSE_SENSITIVITY: f32 = 0.5;
const DEADZONE: f32 = 0.05;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, EditorPlugin::<MainCamera>::default()))
        .add_systems(Startup, startup)
        .add_systems(
            Update,
            (
                global_input_handler,
                (movement_system, focus_camera_system).chain(),
            ),
        )
        .run();
}

#[derive(Component)]
struct MainCamera;

#[derive(Component, Default)]
struct EulerAngles {
    yaw: f32,
    pitch: f32,
    roll: f32,
}

#[derive(Component, Default, Clone)]
struct Front {
    dir: Vec3,
}

impl Front {
    pub fn first_person(&self) -> Self {
        self.clone()
    }

    pub fn third_person(&self) -> Self {
        Self {
            dir: Vec3::new(self.dir.x, 0.0, self.dir.z).normalize(),
        }
    }
}

impl Deref for Front {
    type Target = Vec3;
    fn deref(&self) -> &Self::Target {
        &self.dir
    }
}

impl DerefMut for Front {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.dir
    }
}

fn startup(mut commands: Commands) {
    commands.spawn((
        Name::new("Main Camera"),
        MainCamera,
        Camera3dBundle::default(),
        Front::default(),
        EulerAngles::default(),
    ));
}

fn global_input_handler(
    mut key_events: EventReader<KeyboardInput>,
    mut app_exit: EventWriter<AppExit>,
    mut windows: Query<&mut Window>,
) {
    for key in key_events.read().filter(|key| key.state.is_pressed()) {
        match key.key_code {
            KeyCode::Escape => {
                app_exit.send(AppExit::Success);
            }
            KeyCode::F3 => {
                let mut window = windows.single_mut();
                if window.cursor.visible {
                    debug!("settings cursor hidden");
                    window.cursor.visible = false;
                    window.cursor.grab_mode = CursorGrabMode::Locked;
                } else {
                    debug!("settings cursor visible");
                    window.cursor.visible = true;
                    window.cursor.grab_mode = CursorGrabMode::None;
                }
            }
            _ => (),
        }
    }
}

fn cam_first_person_target_fn(player_pos: Vec3, direction: Vec3) -> (Vec3, Vec3) {
    (player_pos, player_pos + direction)
}

fn movement_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    gamepads: Res<Gamepads>,
    gamepad_axis: Res<Axis<GamepadAxis>>,
    time: Res<Time>,
    mut query: Query<(&mut Transform, &Front), With<MainCamera>>,
) {
    let (mut transform, front) = query.single_mut();
    let front = front.first_person();

    let mut movement = Vec3::default();
    let mut moved = false;

    if keyboard_input.pressed(KeyCode::KeyW) {
        movement += front.dir;
        moved = true;
    } else if keyboard_input.pressed(KeyCode::KeyS) {
        movement -= front.dir;
        moved = true;
    }

    if keyboard_input.pressed(KeyCode::KeyA) {
        movement -= front.cross(UP);
        moved = true;
    } else if keyboard_input.pressed(KeyCode::KeyD) {
        movement += front.cross(UP);
        moved = true;
    }

    for gamepad in gamepads.iter() {
        let (x, y) = (
            gamepad_axis
                .get(GamepadAxis::new(gamepad, GamepadAxisType::LeftStickX))
                .unwrap_or_default(),
            gamepad_axis
                .get(GamepadAxis::new(gamepad, GamepadAxisType::LeftStickY))
                .unwrap_or_default(),
        );

        if x.abs() > DEADZONE {
            movement += front.cross(UP) * x;
            moved = true;
        }

        if y.abs() > DEADZONE {
            movement += front.dir * y;
            moved = true;
        }

        break;
    }

    if moved {
        let movement = movement.normalize() * MOVE_SPEED * time.delta().as_millis() as f32;
        transform.translation += movement;
    }
}

fn focus_camera_system(
    windows: Query<&Window>,
    mut mouse_motion: EventReader<MouseMotion>,
    gamepads: Res<Gamepads>,
    gamepad_input: Res<Axis<GamepadAxis>>,
    mut query: ParamSet<(
        Query<(&mut Transform, &mut Front, &mut EulerAngles), With<Camera3d>>,
        Query<&Transform, With<Camera>>,
    )>,
) {
    let cursor_visible = windows.single().cursor.visible;

    let player_pos = query.p1().single().translation;

    let mut cam_query = query.p0();
    let cam_query = cam_query.single_mut();

    let (mouse_x, mouse_y) = mouse_motion
        .read()
        .map(|motion| motion.delta)
        .reduce(|c, n| c + n)
        .map(|offsets| (offsets.x * MOUSE_SENSITIVITY, offsets.y * MOUSE_SENSITIVITY))
        .unwrap_or_default();

    let (gamepad_x, gamepad_y) = gamepads
        .iter()
        .next()
        .map(|gp| {
            (
                gamepad_input
                    .get(GamepadAxis::new(gp, GamepadAxisType::RightStickX))
                    .unwrap_or_default(),
                gamepad_input
                    .get(GamepadAxis::new(gp, GamepadAxisType::RightStickY))
                    .unwrap_or_default(),
            )
        })
        .map(|(x, y)| {
            (
                if x.abs() > DEADZONE { x } else { 0.0 },
                if y.abs() > DEADZONE { y } else { 0.0 },
            )
        })
        .unwrap_or_default();

    let (yaw_value, pitch_value) = if cursor_visible {
        (0.0, 0.0)
    } else {
        (mouse_x + gamepad_x, mouse_y - gamepad_y)
    };

    let (yaw_rad, pitch_rad) = {
        // set cam rotation
        let mut euler_angles = cam_query.2;

        euler_angles.yaw -= yaw_value;
        euler_angles.pitch -= pitch_value;

        euler_angles.yaw %= 360.0;

        euler_angles.pitch = euler_angles.pitch.clamp(-89.0, 89.0);
        (
            euler_angles.yaw.to_radians(),
            euler_angles.pitch.to_radians(),
        )
    };

    let yaw_sin = yaw_rad.sin();
    let pitch_sin = pitch_rad.sin();

    let yaw_cos = yaw_rad.cos();
    let pitch_cos = pitch_rad.cos();

    let direction = Vec3::new(pitch_cos * yaw_cos, pitch_sin, -pitch_cos * yaw_sin).normalize();

    // set cam front
    let mut front = cam_query.1;
    front.dir = direction;

    let mut cam_transform = cam_query.0;
    let (cam_pos, cam_focus) = cam_first_person_target_fn(player_pos, direction);

    // set cam position
    cam_transform.translation = cam_pos;

    // set cam look
    cam_transform.look_at(cam_focus, UP);
}
