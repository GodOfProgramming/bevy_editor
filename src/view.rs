pub mod view2d;
pub mod view3d;

use crate::{
  cache::{Cache, Saveable},
  ui::{misc::UiInfo, prebuilt::editor_view::EditorView},
  Editing,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use view2d::View2d;
use view3d::View3d;

pub const UP: Vec3 = Vec3::Y;

pub struct EditorViewPlugin;

impl EditorViewPlugin {
  fn set_initial_state(cache: Res<Cache>, mut next_state: ResMut<NextState<ActiveEditorCamera>>) {
    let state = cache.get::<ActiveEditorCamera>().unwrap_or_default();
    next_state.set(state);
  }
}

impl Plugin for EditorViewPlugin {
  fn build(&self, app: &mut bevy::prelude::App) {
    app
      .configure_sets(
        Update,
        (
          CameraInput::Keyboard
            .run_if(CameraInput::ui_not_focused)
            .in_set(Editing),
          CameraInput::Mouse
            .run_if(CameraInput::mouse_hovered)
            .in_set(Editing),
          View2d.run_if(in_state(ActiveEditorCamera::Cam2D)),
          View3d.run_if(in_state(ActiveEditorCamera::Cam3D)),
        ),
      )
      .register_type::<ActiveEditorCamera>()
      .register_type::<view2d::CameraSettings>()
      .register_type::<view2d::CameraState>()
      .insert_state(ActiveEditorCamera::None)
      .add_systems(PostStartup, Self::set_initial_state)
      .add_systems(OnEnter(ActiveEditorCamera::None), despawn_editor_cameras)
      .add_systems(OnEnter(ActiveEditorCamera::Cam2D), view2d::enable)
      .add_systems(OnExit(ActiveEditorCamera::Cam2D), view2d::save_settings)
      .add_systems(OnEnter(ActiveEditorCamera::Cam3D), view3d::enable)
      .add_systems(OnExit(ActiveEditorCamera::Cam3D), view3d::save_settings)
      .add_systems(
        Update,
        (
          view2d::movement_system.in_set(CameraInput::Keyboard),
          (
            view2d::mouse_input_actions,
            (view2d::zoom_system, view2d::pan_system),
          )
            .chain()
            .in_set(CameraInput::Mouse),
        )
          .chain()
          .in_set(View2d),
      )
      .add_systems(
        Update,
        (
          (
            view3d::mouse_input_actions,
            (
              view3d::orbit_system,
              view3d::zoom_system,
              view3d::pan_system,
            ),
          )
            .chain()
            .in_set(CameraInput::Mouse),
          view3d::movement_system.in_set(CameraInput::Keyboard),
        )
          .chain()
          .in_set(View3d),
      );
  }
}

#[derive(Default, Component, Reflect)]
#[require(RayCastPickable)]
pub struct EditorCamera;

impl EditorCamera {}

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default, Serialize, Deserialize, Reflect,
)]
pub enum ActiveEditorCamera {
  #[default]
  None,
  Cam2D,
  Cam3D,
}

impl Saveable for ActiveEditorCamera {
  const KEY: &str = "view_state";
}

#[derive(SystemSet, PartialEq, Eq, Hash, Clone, Debug)]
enum CameraInput {
  Keyboard,
  Mouse,
}

impl CameraInput {
  fn ui_not_focused(q_egui: Query<&bevy_egui::EguiContext>) -> bool {
    !q_egui
      .iter()
      .any(|ctx| ctx.get().memory(|mem| mem.focused().is_some()))
  }

  fn mouse_hovered(editor_view_ui_info: Single<&UiInfo, With<EditorView>>) -> bool {
    editor_view_ui_info.hovered()
  }
}

fn despawn_editor_cameras(mut commands: Commands, q_cams: Query<Entity, With<EditorCamera>>) {
  info!("Despawning all editor cameras");
  for entity in &q_cams {
    commands.entity(entity).despawn();
  }
}

pub fn save_view_state(mut cache: ResMut<Cache>, view_state: Res<State<ActiveEditorCamera>>) {
  cache.store(view_state.get());
}

pub fn disable_camera<C: Component>(mut q_camera: Query<&mut Camera, With<C>>) {
  for mut camera in &mut q_camera {
    camera.is_active = false;
  }
}
