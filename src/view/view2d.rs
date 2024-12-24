use super::{EditorCamera, ViewState};
use crate::{
  cache::{Cache, Saveable},
  hide_cursor,
  input::EditorActions,
  show_cursor,
  view::ActiveEditorCamera,
};
use bevy::{input::mouse::MouseMotion, prelude::*};
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

const UP: Vec3 = Vec3::Y;

pub struct View2dPlugin;

impl Plugin for View2dPlugin {
  fn build(&self, app: &mut App) {
    app
      .register_type::<CameraSettings>()
      .add_systems(Startup, Self::spawn_camera)
      .add_systems(OnEnter(ViewState::Camera2D), EditorCamera2d::on_enter)
      .add_systems(
        Update,
        (
          EditorCamera2d::handle_input,
          EditorCamera2d::movement_system,
          EditorCamera2d::zoom_system,
          EditorCamera2d::pan_system,
        )
          .run_if(super::can_run(ViewState::Camera2D)),
      );
  }
}

impl View2dPlugin {
  fn spawn_camera(mut commands: Commands, cache: Res<Cache>) {
    info!("Spawning 2d camera");

    let CameraSaveData {
      settings,
      transform,
    } = cache.get().unwrap_or_default();

    commands.spawn((
      Name::new("Editor Camera 2D"),
      EditorCamera2d,
      settings,
      transform,
    ));
  }
}

#[derive(Default, Serialize, Deserialize)]
struct CameraSaveData {
  settings: CameraSettings,
  transform: Transform,
}

impl Saveable for CameraSaveData {
  const KEY: &str = "camera2d";
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone)]
pub struct CameraSettings {
  move_speed: f32,
  zoom_sensitivity: f32,
  pan_sensitivity: f32,
}

impl Default for CameraSettings {
  fn default() -> Self {
    CameraSettings {
      move_speed: 128.0,
      zoom_sensitivity: 10.0,
      pan_sensitivity: 10.0,
    }
  }
}

#[derive(Component, Default)]
#[require(EditorCamera, Camera2d, CameraSettings)]
pub struct EditorCamera2d;

impl EditorCamera2d {
  fn on_enter(
    mut commands: Commands,
    mut q_2d_cams: Query<(Entity, &mut Camera), With<EditorCamera2d>>,
    mut q_other_cams: Query<(Entity, &mut Camera), Without<EditorCamera2d>>,
  ) {
    info!("Switched to 2d camera");

    for (entity, mut cam) in &mut q_2d_cams {
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
      if action_state.just_pressed(&EditorActions::PanCamera) {
        let Ok(mut window) = windows.get_single_mut() else {
          return;
        };

        hide_cursor(&mut window);
        continue;
      }

      if action_state.just_released(&EditorActions::PanCamera) {
        let Ok(mut window) = windows.get_single_mut() else {
          return;
        };

        show_cursor(&mut window);
      }
    }
  }

  fn movement_system(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut q_cam: Query<(&CameraSettings, &mut Transform), With<EditorCamera2d>>,
    time: Res<Time>,
  ) {
    for action_state in &q_action_states {
      let (cam_settings, mut cam_transform) = q_cam.single_mut();

      let mut movement = Vec3::ZERO;

      if action_state.pressed(&EditorActions::MoveNorth) {
        movement += UP;
      }

      if action_state.pressed(&EditorActions::MoveSouth) {
        movement -= UP;
      }

      if action_state.pressed(&EditorActions::MoveWest) {
        movement -= Vec3::X;
      }

      if action_state.pressed(&EditorActions::MoveEast) {
        movement += Vec3::X;
      }

      let moved = movement != Vec3::ZERO;

      if moved {
        let movement = movement.normalize() * cam_settings.move_speed * time.delta_secs();
        cam_transform.translation += movement;
      }
    }
  }

  fn zoom_system(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut q_cam: Query<(&CameraSettings, &mut OrthographicProjection), With<EditorCamera2d>>,
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

      projection.scale *= zoom;
    }
  }

  fn pan_system(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut q_cam: Query<(&CameraSettings, &mut Transform), With<EditorCamera2d>>,
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

  pub fn on_app_exit(
    app_exit: EventReader<AppExit>,
    mut cache: ResMut<Cache>,
    q_cam: Query<(&Transform, &CameraSettings), With<EditorCamera2d>>,
  ) {
    if !app_exit.is_empty() {
      for (cam_transform, cam_settings) in &q_cam {
        cache.store(CameraSaveData {
          settings: cam_settings.clone(),
          transform: cam_transform.clone(),
        });
      }
    }
  }
}
