use super::{EditorCamera, ViewState};
use crate::{
  cache::{Cache, Saveable},
  input::EditorActions,
  set_cursor_icon,
  view::ActiveEditorCamera,
};
use bevy::{
  input::mouse::MouseMotion,
  picking::pointer::PointerLocation,
  prelude::*,
  window::{PrimaryWindow, SystemCursorIcon},
};
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

pub const UP: Vec3 = Vec3::Y;

pub struct View2dPlugin;

impl Plugin for View2dPlugin {
  fn build(&self, app: &mut App) {
    app
      .register_type::<CameraSettings>()
      .register_type::<CameraState>()
      .add_systems(Startup, Self::spawn_camera)
      .add_systems(OnEnter(ViewState::Camera2D), EditorCamera2d::on_enter)
      .add_systems(
        Update,
        (
          EditorCamera2d::movement_system,
          (
            EditorCamera2d::handle_input,
            (EditorCamera2d::zoom_system, EditorCamera2d::pan_system),
          )
            .chain()
            .run_if(super::mouse_actions_enabled),
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
      orthographic_scale,
    } = cache.get().unwrap_or_default();

    let mut projection = OrthographicProjection::default_2d();

    if let Some(scale) = orthographic_scale {
      projection.scale = scale;
    }

    commands.spawn((
      Name::new("Editor Camera 2D"),
      EditorCamera2d,
      CameraState::default(),
      settings,
      transform,
      projection,
    ));
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
    mut commands: Commands,
    mut q_cam_states: Query<(&mut CameraState, &Camera), With<EditorCamera2d>>,
    q_action_states: Query<&ActionState<EditorActions>>,
    primary_window: Single<Entity, With<PrimaryWindow>>,
    q_pointers: Query<&PointerLocation>,
  ) {
    for action_state in &q_action_states {
      if action_state.just_pressed(&EditorActions::PanCamera) {
        set_cursor_icon(&mut commands, *primary_window, SystemCursorIcon::Grab);

        for (mut cam_state, camera) in &mut q_cam_states {
          cam_state.pan_viewport_start = q_pointers
            .iter()
            .next()
            .and_then(|p| p.location.as_ref().zip(camera.viewport.as_ref()))
            .map(|(location, viewport)| location.position - viewport.physical_position.as_vec2());
        }

        continue;
      }

      if action_state.just_released(&EditorActions::PanCamera) {
        set_cursor_icon(&mut commands, *primary_window, SystemCursorIcon::default())
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
    mut q_cam: Single<
      (
        &CameraSettings,
        &mut Transform,
        &GlobalTransform,
        &mut CameraState,
        &Camera,
      ),
      With<EditorCamera2d>,
    >,
    mut mouse_motion: EventReader<MouseMotion>,
  ) {
    let should_pan = q_action_states
      .iter()
      .any(|state| state.pressed(&EditorActions::PanCamera));

    if !should_pan {
      return;
    }

    let (ref cam_settings, ref mut cam_transform, ref cam_g_transform, ref mut cam_state, ref cam) =
      &mut *q_cam;

    let pan_motion = mouse_motion
      .read()
      .map(|motion| motion.delta)
      .reduce(|c, n| c + n)
      .unwrap_or_default()
      * cam_settings.pan_sensitivity;

    if let Some((pan_vp_new_pos, pan_world_old_pos, pan_world_new_pos)) = cam_state
      .pan_viewport_start
      .map(|p| (p, p + pan_motion))
      .and_then(|(op, np)| {
        cam
          .viewport_to_world_2d(&cam_g_transform, op)
          .ok()
          .zip(cam.viewport_to_world_2d(&cam_g_transform, np).ok())
          .map(|(ow, nw)| (np, ow, nw))
      })
    {
      let delta = pan_world_new_pos - pan_world_old_pos;

      cam_state.pan_viewport_start = Some(pan_vp_new_pos);
      cam_transform.translation -= delta.extend(0.0);
    }
  }

  pub fn on_app_exit(
    mut cache: ResMut<Cache>,
    q_cam: Query<(&Transform, &CameraSettings, &OrthographicProjection), With<EditorCamera2d>>,
  ) {
    for (cam_transform, cam_settings, cam_ortho) in &q_cam {
      cache.store(&CameraSaveData {
        settings: cam_settings.clone(),
        transform: cam_transform.clone(),
        orthographic_scale: Some(cam_ortho.scale),
      });
    }
  }
}

#[derive(Default, Serialize, Deserialize)]
struct CameraSaveData {
  settings: CameraSettings,
  transform: Transform,
  orthographic_scale: Option<f32>,
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

#[derive(Default, Component, Reflect, Serialize, Deserialize, Clone)]
pub struct CameraState {
  pan_viewport_start: Option<Vec2>,
  pan_world_pos: Vec2,
  pan_world_new_pos: Vec2,
  pan_value: Vec2,
}
