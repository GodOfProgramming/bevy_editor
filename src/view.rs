mod view2d;
mod view3d;

use crate::cache::{Cache, Saveable};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
pub use view3d::{EditorCamera3d, View3dPlugin};

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
      .insert_state(ViewState::None)
      .add_systems(PostStartup, Self::startup)
      .add_plugins(View3dPlugin);
  }
}

#[derive(Component)]
pub struct ActiveEditorCamera;

#[derive(Component, Default)]
#[require(RayCastPickable)]
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
