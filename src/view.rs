pub mod view2d;
pub mod view3d;

use crate::{
  cache::{Cache, Saveable},
  ui::prebuilt::editor_view::EditorView,
  EditorState,
};
use bevy::prelude::*;
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
      );
  }
}

#[derive(Component)]
pub struct ActiveEditorCamera;

#[derive(Default, Component, Reflect)]
#[require(RayCastPickable)]
pub struct EditorCamera;

impl EditorCamera {
  fn disable_cameras(mut q_cams: Query<&mut Camera>) {
    for mut cam in &mut q_cams {
      cam.is_active = false;
    }
  }

  pub fn on_app_exit(mut cache: ResMut<Cache>, view_state: Res<State<ViewState>>) {
    cache.store(view_state.get());
  }

  // make camera only render to view not obstructed by UI
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

fn mouse_actions_enabled(q_editor_views: Query<&EditorView>) -> bool {
  q_editor_views.iter().any(|gv| gv.hovered())
}
