use crate::{
  cache::{Cache, Saveable},
  hide_cursor,
  input::EditorActions,
  show_cursor,
};
use bevy::{input::mouse::MouseMotion, prelude::*};
use leafwing_input_manager::prelude::ActionState;
use serde::{Deserialize, Serialize};

use super::{ActiveEditorCamera, EditorCamera, EditorCamera2d, ViewState};

pub const UP: Vec3 = Vec3::Y;

pub struct View3dPlugin;

impl Plugin for View3dPlugin {
  fn build(&self, app: &mut App) {
    app
      .register_type::<CameraSettings>()
      .add_systems(Startup, Self::spawn_camera)
      .add_systems(OnEnter(ViewState::Camera3D), EditorCamera3d::on_enter)
      .add_systems(
        Update,
        (
          EditorCamera3d::movement_system,
          (
            EditorCamera3d::handle_input,
            EditorCamera3d::orbit_system,
            EditorCamera3d::zoom_system,
            EditorCamera3d::pan_system,
          )
            .run_if(super::mouse_actions_enabled),
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
      settings,
      transform,
    } = cache.get().unwrap_or_default();

    commands.spawn((
      Name::new("Editor Camera 3D"),
      EditorCamera3d,
      settings,
      transform,
      Camera {
        order: isize::MIN,
        ..default()
      },
    ));
  }
}

#[derive(Component, Default)]
#[require(EditorCamera, Camera3d, CameraSettings)]
pub struct EditorCamera3d;

impl EditorCamera3d {
  fn on_enter(
    mut commands: Commands,
    mut q_3d_cams: Query<(Entity, &mut Camera), (With<EditorCamera3d>, Without<EditorCamera2d>)>,
    mut q_2d_cams: Query<(Entity, &mut Camera), (With<EditorCamera2d>, Without<EditorCamera3d>)>,
  ) {
    info!("Switched to 3d camera");

    for (entity, mut cam) in &mut q_3d_cams {
      commands.entity(entity).insert(ActiveEditorCamera);
      cam.is_active = true;
    }

    for (entity, mut cam) in &mut q_2d_cams {
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
    mut q_cam: Single<(&CameraSettings, &mut Transform), With<EditorCamera3d>>,
    time: Res<Time>,
  ) {
    for action_state in &q_action_states {
      let (ref cam_settings, ref mut cam_transform) = &mut *q_cam;

      let forward = cam_transform.forward().as_vec3();
      let mut movement = Vec3::ZERO;

      if action_state.pressed(&EditorActions::MoveNorth) {
        movement += forward;
      }

      if action_state.pressed(&EditorActions::MoveSouth) {
        movement -= forward;
      }

      if action_state.pressed(&EditorActions::MoveWest) {
        movement -= forward.cross(UP);
      }

      if action_state.pressed(&EditorActions::MoveEast) {
        movement += forward.cross(UP);
      }

      let moved = movement != Vec3::ZERO;

      if moved {
        let movement = movement.normalize() * cam_settings.move_speed * time.delta_secs();
        cam_transform.translation += movement;
      }
    }
  }

  fn orbit_system(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut q_cam: Single<(&CameraSettings, &mut Transform), With<EditorCamera3d>>,
    mut mouse_motion: EventReader<MouseMotion>,
    time: Res<Time>,
  ) {
    let should_orbit = q_action_states
      .iter()
      .any(|state| state.pressed(&EditorActions::OrbitCamera));

    if !should_orbit {
      return;
    }

    let (ref settings, ref mut transform) = &mut *q_cam;

    let orbit = mouse_motion
      .read()
      .map(|motion| motion.delta)
      .reduce(|c, n| c + n)
      .map(|mouse| mouse * settings.orbit_sensitivity * time.delta_secs())
      .unwrap_or_default();

    let right = transform.right();
    transform.rotate_axis(right, -orbit.y);
    transform.rotate_axis(Dir3::new(UP).unwrap(), -orbit.x);
  }

  fn pan_system(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut q_cam: Single<(&CameraSettings, &mut Transform), With<EditorCamera3d>>,
    mut mouse_motion: EventReader<MouseMotion>,
    time: Res<Time>,
  ) {
    let should_pan = q_action_states
      .iter()
      .any(|state| state.pressed(&EditorActions::PanCamera));

    if !should_pan {
      return;
    }

    let (ref cam_settings, ref mut cam_transform) = &mut *q_cam;

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

  pub fn on_app_exit(
    mut cache: ResMut<Cache>,
    q_cam: Query<(&Transform, &CameraSettings), With<EditorCamera3d>>,
  ) {
    for (cam_transform, cam_settings) in &q_cam {
      cache.store(&CameraSaveData {
        settings: cam_settings.clone(),
        transform: cam_transform.clone(),
      });
    }
  }
}

#[derive(Default, Serialize, Deserialize)]
struct CameraSaveData {
  settings: CameraSettings,
  transform: Transform,
}

impl Saveable for CameraSaveData {
  const KEY: &str = "camera3d";
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
