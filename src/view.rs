mod view2d;
mod view3d;

use crate::{
  cache::{Cache, Saveable},
  ui,
};
use bevy::{prelude::*, render::camera::Viewport, window::PrimaryWindow};
use serde::{Deserialize, Serialize};
pub use view2d::EditorCamera2d;
use view2d::View2dPlugin;
pub use view3d::EditorCamera3d;
use view3d::View3dPlugin;

pub struct ViewPlugin;

impl ViewPlugin {
  fn startup(cache: Res<Cache>, mut next_state: ResMut<NextState<ViewState>>) {
    let state = cache.get::<ViewState>().unwrap_or(ViewState::Camera3D);
    next_state.set(state);
  }
}

impl Plugin for ViewPlugin {
  fn build(&self, app: &mut bevy::prelude::App) {
    app
      .add_plugins((View2dPlugin, View3dPlugin))
      .insert_state(ViewState::None)
      .add_systems(PostStartup, Self::startup)
      .add_systems(PostUpdate, EditorCamera::set_viewport);
  }
}

#[derive(Component)]
pub struct ActiveEditorCamera;

#[derive(Component, Default)]
pub struct EditorCamera;

impl EditorCamera {
  pub fn on_app_exit(
    app_exit: EventReader<AppExit>,
    mut cache: ResMut<Cache>,
    view_state: Res<State<ViewState>>,
  ) {
    if !app_exit.is_empty() {
      cache.store(**view_state);
    }
  }

  // make camera only render to view not obstructed by UI
  fn set_viewport(
    primary_window: Query<&mut Window, With<PrimaryWindow>>,
    q_egui_settings: Query<&bevy_egui::EguiSettings>,
    mut cameras: Query<&mut Camera>,
    ui_state: Res<ui::State>,
  ) {
    let Ok(window) = primary_window.get_single() else {
      warn!("Found no window");
      return;
    };

    let Ok(egui_settings) = q_egui_settings.get_single() else {
      warn!("Found no egui settings");
      return;
    };

    for mut cam in &mut cameras {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default, Serialize, Deserialize)]
pub enum ViewState {
  #[default]
  None,
  Camera2D,
  Camera3D,
}

impl Saveable for ViewState {
  const KEY: &str = "view_state";
}
