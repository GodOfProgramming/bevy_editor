use super::ViewPlugin;
use crate::{ui, EditorState};
use bevy::{
  input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel},
  prelude::*,
  render::camera::Viewport,
  window::PrimaryWindow,
};
use std::f32::consts::{FRAC_PI_2, TAU};

const UP: Vec3 = Vec3::Y;

pub struct View3dPlugin;

impl ViewPlugin for View3dPlugin {}

impl Plugin for View3dPlugin {
  fn build(&self, app: &mut App) {
    app
      .register_type::<CameraState>()
      .register_type::<CameraSettings>()
      .add_systems(Startup, Self::spawn_camera)
      .add_systems(
        Update,
        ((movement_system, look_system), cam_free_fly)
          .chain()
          .run_if(in_state(EditorState::Inspecting)),
      )
      .add_systems(PostUpdate, set_camera_viewport);
  }
}

impl View3dPlugin {
  fn spawn_camera(mut commands: Commands) {
    commands.spawn(EditorCamera);
  }
}

#[derive(Component, Default)]
#[require(Camera3d, CameraState, CameraSettings, RayCastPickable)]
pub struct EditorCamera;

#[derive(Component, Reflect)]
pub struct CameraState {
  face: Vec3,
  pitch: f32,
  yaw: f32,
  zoom: f32,
}

impl Default for CameraState {
  fn default() -> Self {
    Self {
      face: Vec3::X * 10.0,
      pitch: default(),
      yaw: default(),
      zoom: default(),
    }
  }
}

#[derive(Component, Reflect)]
pub struct CameraSettings {
  /// Radians per pixel of mouse motion
  pub orbit_sensitivity: f32,
  /// Exponent per pixel of mouse motion
  pub zoom_sensitivity: f32,
  /// For devices with a notched scroll wheel, like desktop mice
  pub scroll_line_sensitivity: f32,
  /// For devices with smooth scrolling, like touchpads
  pub scroll_pixel_sensitivity: f32,
}

impl Default for CameraSettings {
  fn default() -> Self {
    CameraSettings {
      orbit_sensitivity: 0.05f32.to_radians(), // 0.1 degree per pixel
      zoom_sensitivity: 0.01,
      scroll_line_sensitivity: 16.0, // 1 "line" == 16 "pixels of motion"
      scroll_pixel_sensitivity: 1.0,
    }
  }
}

fn movement_system(
  keyboard_input: Res<ButtonInput<KeyCode>>,
  time: Res<Time>,
  mut query: Query<(&CameraState, &mut Transform), With<Camera>>,
) {
  const MOVE_SPEED: f32 = 0.05;

  let (cam_state, mut cam_transform) = query.single_mut();

  let mut movement = Vec3::ZERO;

  if keyboard_input.pressed(KeyCode::KeyW) {
    movement += cam_state.face;
  } else if keyboard_input.pressed(KeyCode::KeyS) {
    movement -= cam_state.face;
  }

  if keyboard_input.pressed(KeyCode::KeyA) {
    movement -= cam_state.face.cross(UP);
  } else if keyboard_input.pressed(KeyCode::KeyD) {
    movement += cam_state.face.cross(UP);
  }

  let moved = movement != Vec3::ZERO;

  if moved {
    let movement = movement.normalize() * MOVE_SPEED * time.delta().as_millis() as f32;
    cam_transform.translation += movement;
  }
}

fn look_system(
  mut mouse_motion: EventReader<MouseMotion>,
  mut q_cam: Query<(&CameraSettings, &mut CameraState), With<Camera>>,
) {
  let (settings, mut state) = q_cam.single_mut();

  let orbit = mouse_motion
    .read()
    .map(|motion| motion.delta)
    .reduce(|c, n| c + n)
    .map(|mouse| mouse * settings.orbit_sensitivity)
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

fn apply_scroll_effect(
  mut mouse_scroll: EventReader<MouseWheel>,
  mut query: Query<(&CameraSettings, &mut CameraState), With<Camera>>,
) {
  let mut total_scroll_lines = Vec2::ZERO;
  let mut total_scroll_pixels = Vec2::ZERO;
  for scroll_event in mouse_scroll.read() {
    match scroll_event.unit {
      MouseScrollUnit::Line => {
        total_scroll_lines.x += scroll_event.x;
        total_scroll_lines.y -= scroll_event.y;
      }
      MouseScrollUnit::Pixel => {
        total_scroll_pixels.x += scroll_event.x;
        total_scroll_pixels.y -= scroll_event.y;
      }
    }
  }

  let (cam_settings, mut cam_state) = query.single_mut();

  let adjusted_total_lines =
    -total_scroll_lines.y * cam_settings.scroll_line_sensitivity * cam_settings.zoom_sensitivity;
  let adjusted_total_pixels =
    -total_scroll_pixels.y * cam_settings.scroll_pixel_sensitivity * cam_settings.zoom_sensitivity;

  let total_zoom = adjusted_total_lines + adjusted_total_pixels;

  cam_state.zoom += total_zoom.exp();
}

fn cam_free_fly(mut q_cam: Query<(&mut Transform, &CameraState), With<Camera>>) {
  let (mut cam_transform, cam_state) = q_cam.single_mut();

  cam_transform.translation += cam_state.face * cam_state.zoom;

  let cam_target = cam_transform.translation + cam_state.face;

  cam_transform.look_at(cam_target, UP);
}

fn cam_look_at_target<T>(
  mut query: ParamSet<(
    Query<(&mut Transform, &CameraState), With<Camera>>,
    Query<&Transform, With<T>>,
  )>,
) where
  T: Component,
{
  let q_transforms = query.p1();
  let Ok(target_transform) = q_transforms.get_single() else {
    return;
  };

  let target_pos = target_transform.translation;
  let target_magnitude = target_transform.scale.length();

  let mut q_cam = query.p0();
  let (mut cam_transform, cam_state) = q_cam.single_mut();
  let cam_pos = target_pos - cam_state.face * 5.0 * target_magnitude;

  // set cam position
  cam_transform.translation = cam_pos;

  // set cam look
  cam_transform.look_at(target_pos, UP);
}

// make camera only render to view not obstructed by UI
fn set_camera_viewport(
  ui_state: Res<ui::State>,
  primary_window: Query<&mut Window, With<PrimaryWindow>>,
  q_egui_settings: Query<&bevy_egui::EguiSettings>,
  mut cameras: Query<&mut Camera, With<EditorCamera>>,
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
