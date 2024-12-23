use crate::{input::EditorActions, ui, EditorState};
use bevy::{
  input::mouse::MouseMotion, prelude::*, render::camera::Viewport, window::PrimaryWindow,
};
use leafwing_input_manager::prelude::ActionState;
use std::f32::consts::{FRAC_PI_2, TAU};

const UP: Vec3 = Vec3::Y;

pub struct View3dPlugin;

impl Plugin for View3dPlugin {
  fn build(&self, app: &mut App) {
    app
      .register_type::<CameraState>()
      .register_type::<CameraSettings>()
      .add_systems(Startup, Self::spawn_camera)
      .add_systems(
        Update,
        (
          (
            EditorCamera::movement_system,
            EditorCamera::orbit_self_system,
            EditorCamera::zoom_system,
            EditorCamera::pan_system,
          ),
          EditorCamera::free_fly,
        )
          .chain()
          .run_if(in_state(EditorState::Editing)),
      )
      .add_systems(PostUpdate, EditorCamera::set_viewport);
  }
}

impl View3dPlugin {
  fn spawn_camera(mut commands: Commands) {
    commands.spawn((Name::new("Editor Camera"), EditorCamera));
  }
}

#[derive(Component, Reflect)]
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

#[derive(Component, Reflect)]
pub struct CameraSettings {
  move_speed: f32,
  orbit_sensitivity: f32,
  zoom_sensitivity: f32,
  pan_sensitivity: f32,
}

#[derive(Component, Default)]
#[require(Camera3d, CameraState, CameraSettings, RayCastPickable)]
pub struct EditorCamera;

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

impl EditorCamera {
  fn movement_system(
    q_action_states: Query<&ActionState<EditorActions>>,
    mut q_cam: Query<(&CameraState, &CameraSettings, &mut Transform), With<EditorCamera>>,
    time: Res<Time>,
  ) {
    for action_state in &q_action_states {
      let (cam_state, cam_settings, mut cam_transform) = q_cam.single_mut();

      let mut movement = Vec3::ZERO;

      if action_state.pressed(&EditorActions::MoveForward) {
        movement += cam_state.face;
      }

      if action_state.pressed(&EditorActions::MoveBack) {
        movement -= cam_state.face;
      }

      if action_state.pressed(&EditorActions::MoveLeft) {
        movement -= cam_state.face.cross(UP);
      }

      if action_state.pressed(&EditorActions::MoveRight) {
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
    mut q_cam: Query<(&CameraSettings, &mut CameraState), With<EditorCamera>>,
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
    mut q_cam: Query<(&CameraSettings, &mut Transform), With<EditorCamera>>,
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
    mut q_cam: Query<(&CameraSettings, &mut Projection), With<EditorCamera>>,
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
        Projection::Orthographic(_orthographic_projection) => {}
      }
    }
  }

  fn free_fly(mut q_cam: Query<(&mut Transform, &CameraState), With<EditorCamera>>) {
    let (mut cam_transform, cam_state) = q_cam.single_mut();
    let cam_target = cam_transform.translation + cam_state.face;
    cam_transform.look_at(cam_target, UP);
  }

  // make camera only render to view not obstructed by UI
  fn set_viewport(
    primary_window: Query<&mut Window, With<PrimaryWindow>>,
    q_egui_settings: Query<&bevy_egui::EguiSettings>,
    mut cameras: Query<&mut Camera, With<EditorCamera>>,
    ui_state: Res<ui::State>,
  ) {
    let Ok(mut cam) = cameras.get_single_mut() else {
      warn!("Found no camera");
      return;
    };

    let Ok(window) = primary_window.get_single() else {
      warn!("Found no window");
      return;
    };

    let Ok(egui_settings) = q_egui_settings.get_single() else {
      warn!("Found no egui settings");
      return;
    };

    let scale_factor = window.scale_factor() * egui_settings.scale_factor;

    let viewport_pos = ui_state.viewport_rect.left_top().to_vec2() * scale_factor;
    let viewport_size = ui_state.viewport_rect.size() * scale_factor;

    let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
    let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

    // The desired viewport rectangle at its offset in "physical pixel space"
    let rect = physical_position + physical_size;

    let window_size = window.physical_size();
    if rect.x <= window_size.x && rect.y <= window_size.y {
      cam.viewport = Some(Viewport {
        physical_position,
        physical_size,
        depth: 0.0..1.0,
      });
    }
  }
}
