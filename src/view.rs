pub mod view2d;
pub mod view3d;

use crate::{
  cache::{Cache, Saveable},
  ui::game_view::GameView,
  EditorState,
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
    let state = cache.get::<ViewState>().unwrap_or_default();
    next_state.set(state);
  }
}

impl Plugin for ViewPlugin {
  fn build(&self, app: &mut bevy::prelude::App) {
    app
      .register_type::<ViewState>()
      .add_plugins((View2dPlugin, View3dPlugin))
      .insert_state(ViewState::None)
      .add_systems(PostStartup, Self::startup)
      .add_systems(
        Update,
        EditorCamera::disable_cameras.run_if(in_state(ViewState::None)),
      )
      .add_systems(PostUpdate, EditorCamera::set_viewport);
  }
}

#[derive(Component)]
pub struct ActiveEditorCamera;

#[derive(Component, Default)]
#[require(RayCastPickable)]
pub struct EditorCamera;

impl EditorCamera {
  fn disable_cameras(mut q_cams: Query<&mut Camera>) {
    for mut cam in &mut q_cams {
      cam.is_active = false;
    }
  }

  pub fn on_app_exit(
    app_exit: EventReader<AppExit>,
    mut cache: ResMut<Cache>,
    view_state: Res<State<ViewState>>,
  ) {
    if !app_exit.is_empty() {
      cache.store(view_state.get());
    }
  }

  // make camera only render to view not obstructed by UI
  fn set_viewport(
    primary_window: Query<&mut Window, With<PrimaryWindow>>,
    q_egui_settings: Query<&bevy_egui::EguiSettings>,
    mut cameras: Query<&mut Camera>,
    game_view: Res<GameView>,
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

      let viewport = game_view.viewport();
      let viewport_pos = viewport.left_top().to_vec2() * scale_factor;
      let viewport_size = viewport.size() * scale_factor;

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

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default, Serialize, Deserialize, Reflect,
)]
pub enum ViewState {
  #[default]
  None,
  Camera2D,
  Camera3D,
}

impl Saveable for ViewState {
  const KEY: &str = "view_state";
}

fn can_run(
  view_state_condition: ViewState,
) -> impl FnMut(
  Option<Res<State<EditorState>>>,
  Option<Res<State<ViewState>>>,
  Query<&bevy_egui::EguiContext>,
) -> bool
     + Clone {
  move |editor_state: Option<Res<State<EditorState>>>,
        view_state: Option<Res<State<ViewState>>>,
        q_egui: Query<&bevy_egui::EguiContext>|
        -> bool {
    editor_state
      .map(|state| *state == EditorState::Editing)
      .unwrap_or_default()
      && view_state
        .map(|state| *state == view_state_condition)
        .unwrap_or_default()
      && q_egui
        .iter()
        .any(|ctx| ctx.get().memory(|mem| mem.focused().is_none()))
  }
}

fn mouse_actions_enabled(game_view: Res<GameView>) -> bool {
  game_view.hovered()
}
