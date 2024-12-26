use crate::{
  cache::{Cache, Saveable},
  hide_cursor,
  input::EditorActions,
  show_cursor,
};
use bevy::{input::mouse::MouseMotion, prelude::*};
use leafwing_input_manager::prelude::ActionState;
use serde::{Deserialize, Serialize};
use std::f32::consts::{FRAC_PI_2, TAU};

use super::{ActiveEditorCamera, EditorCamera, ViewState};

pub const UP: Vec3 = Vec3::Y;

pub struct View3dPlugin;

impl Plugin for View3dPlugin {
  fn build(&self, app: &mut App) {
    app
      .register_type::<CameraState>()
      .register_type::<CameraSettings>()
      .add_systems(Startup, Self::spawn_camera)
      .add_systems(OnEnter(ViewState::Camera3D), EditorCamera3d::on_enter)
      .add_systems(
        Update,
        (
          (
            EditorCamera3d::movement_system,
            (
              EditorCamera3d::handle_input,
              EditorCamera3d::orbit_self_system,
              EditorCamera3d::zoom_system,
              EditorCamera3d::pan_system,
            )
              .run_if(super::mouse_actions_enabled),
          ),
          EditorCamera3d::look,
        )
          .chain()
          .run_if(super::can_run(ViewState::Camera3D)),
      );
  }
}

impl View3dPlugin {
  fn spawn_camera(mut commands: Commands, cache: Res<Cache>) {
    info!("Spawning 3d camera");

    let CameraSaveData {
      state,
      settings,
      transform,
    } = cache.get().unwrap_or_default();

    commands.spawn((
      Name::new("Editor Camera 3D"),
      EditorCamera3d,
      state,
      settings,
      transform,
    ));
  }
}

#[derive(Default, Serialize, Deserialize)]
struct CameraSaveData {
  state: CameraState,
  settings: CameraSettings,
  transform: Transform,
}

impl Saveable for CameraSaveData {
  const KEY: &str = "camera3d";
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone)]
pub struct CameraState {
  face: Vec3,
  pitch: f32,
  yaw: f32,
}

impl Default for CameraState {
  fn default() -> Self {
    Self {
      face: Vec3::X * 10.0,
      pitch: default(),
      yaw: default(),
    }
  }
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone)]
pub struct CameraSettings {
  move_speed: f32,
  orbit_sensitivity: f32,
  zoom_sensitivity: f32,
  pan_sensitivity: f32,
}

impl Default for CameraSettings {
  fn default() -> Self {
    CameraSettings {
      move_speed: 10.0,
      orbit_sensitivity: 0.05,
      zoom_sensitivity: 5.0,
      pan_sensitivity: 0.2,
    }
  }
}

#[derive(Component, Default)]
#[require(EditorCamera, Camera3d, CameraState, CameraSettings)]
pub struct EditorCamera3d;

impl EditorCamera3d {
  fn on_enter(
    mut commands: Commands,
    mut q_3d_cams: Query<(Entity, &mut Camera), With<EditorCamera3d>>,
    mut q_other_cams: Query<(Entity, &mut Camera), Without<EditorCamera3d>>,
  ) {
    info!("Switched to 3d camera");

    for (entity, mut cam) in &mut q_3d_cams {
      commands.entity(entity).insert(ActiveEditorCamera);
      cam.is_active = true;
    }

    for (entity, mut cam) in &mut q_other_cams {
      commands.entity(entity).remove::<ActiveEditorCamera>();
      cam.is_active = false;
    }
  }

  pub fn handle_input(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut windows: Query<&mut Window>,
  ) {
    for action_state in &q_action_states {
      if action_state.just_pressed(&EditorActions::OrbitCamera)
        || action_state.just_pressed(&EditorActions::PanCamera)
      {
        let Ok(mut window) = windows.get_single_mut() else {
          return;
        };

        hide_cursor(&mut window);
        continue;
      }

      if (action_state.just_released(&EditorActions::OrbitCamera)
        && action_state.released(&EditorActions::PanCamera))
        || (action_state.just_released(&EditorActions::PanCamera)
          && action_state.released(&EditorActions::OrbitCamera))
      {
        let Ok(mut window) = windows.get_single_mut() else {
          return;
        };

        show_cursor(&mut window);
      }
    }
  }

  fn movement_system(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut q_cam: Query<(&CameraState, &CameraSettings, &mut Transform), With<EditorCamera3d>>,
    time: Res<Time>,
  ) {
    for action_state in &q_action_states {
      let (cam_state, cam_settings, mut cam_transform) = q_cam.single_mut();

      let mut movement = Vec3::ZERO;

      if action_state.pressed(&EditorActions::MoveNorth) {
        movement += cam_state.face;
      }

      if action_state.pressed(&EditorActions::MoveSouth) {
        movement -= cam_state.face;
      }

      if action_state.pressed(&EditorActions::MoveWest) {
        movement -= cam_state.face.cross(UP);
      }

      if action_state.pressed(&EditorActions::MoveEast) {
        movement += cam_state.face.cross(UP);
      }

      let moved = movement != Vec3::ZERO;

      if moved {
        let movement = movement.normalize() * cam_settings.move_speed * time.delta_secs();
        cam_transform.translation += movement;
      }
    }
  }

  fn orbit_self_system(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut q_cam: Query<(&CameraSettings, &mut CameraState), With<EditorCamera3d>>,
    mut mouse_motion: EventReader<MouseMotion>,
    time: Res<Time>,
  ) {
    let should_orbit = q_action_states
      .iter()
      .any(|state| state.pressed(&EditorActions::OrbitCamera));

    if !should_orbit {
      return;
    }

    let (settings, mut state) = q_cam.single_mut();

    let orbit = mouse_motion
      .read()
      .map(|motion| motion.delta)
      .reduce(|c, n| c + n)
      .map(|mouse| mouse * settings.orbit_sensitivity * time.delta_secs())
      .unwrap_or_default();

    let (yaw_rad, pitch_rad) = {
      state.yaw -= orbit.x;
      state.pitch -= orbit.y;

      state.yaw %= TAU;

      state.pitch = state.pitch.clamp(
        -FRAC_PI_2 + 1.0f32.to_radians(),
        FRAC_PI_2 - 1.0f32.to_radians(),
      );
      (state.yaw, state.pitch)
    };

    let yaw_sin = yaw_rad.sin();
    let pitch_sin = pitch_rad.sin();

    let yaw_cos = yaw_rad.cos();
    let pitch_cos = pitch_rad.cos();

    // set cam face
    state.face = Vec3::new(pitch_cos * yaw_cos, pitch_sin, -pitch_cos * yaw_sin).normalize();
  }

  fn pan_system(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut q_cam: Query<(&CameraSettings, &mut Transform), With<EditorCamera3d>>,
    mut mouse_motion: EventReader<MouseMotion>,
    time: Res<Time>,
  ) {
    let should_pan = q_action_states
      .iter()
      .any(|state| state.pressed(&EditorActions::PanCamera));

    if !should_pan {
      return;
    }

    let (cam_settings, mut cam_transform) = q_cam.single_mut();

    let pan = mouse_motion
      .read()
      .map(|motion| motion.delta)
      .reduce(|c, n| c + n)
      .unwrap_or_default();

    let sensitivity = cam_settings.pan_sensitivity * time.delta_secs();
    let horizontal = cam_transform.right() * pan.x * sensitivity;
    let vertical = cam_transform.up() * pan.y * sensitivity;

    cam_transform.translation += horizontal;
    cam_transform.translation -= vertical;
  }

  fn zoom_system(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut q_cam: Query<(&CameraSettings, &mut Projection), With<EditorCamera3d>>,
    time: Res<Time>,
  ) {
    let Ok((cam_settings, mut projection)) = q_cam.get_single_mut() else {
      return;
    };

    for action_state in &q_action_states {
      let zoom = 1.0
        - action_state.clamped_value(&EditorActions::Zoom)
          * cam_settings.zoom_sensitivity
          * time.delta_secs();

      match &mut *projection {
        Projection::Perspective(perspective_projection) => {
          perspective_projection.fov *= zoom;
        }
        Projection::Orthographic(orthographic_projection) => {
          orthographic_projection.scale *= zoom;
        }
      }
    }
  }

  fn look(mut q_cam: Query<(&mut Transform, &CameraState), With<EditorCamera3d>>) {
    let (mut cam_transform, cam_state) = q_cam.single_mut();
    let cam_target = cam_transform.translation + cam_state.face;
    cam_transform.look_at(cam_target, UP);
  }

  pub fn on_app_exit(
    app_exit: EventReader<AppExit>,
    mut cache: ResMut<Cache>,
    q_cam: Query<(&Transform, &CameraState, &CameraSettings), With<EditorCamera3d>>,
  ) {
    if !app_exit.is_empty() {
      for (cam_transform, cam_state, cam_settings) in &q_cam {
        cache.store(CameraSaveData {
          state: cam_state.clone(),
          settings: cam_settings.clone(),
          transform: cam_transform.clone(),
        });
      }
    }
  }
}
