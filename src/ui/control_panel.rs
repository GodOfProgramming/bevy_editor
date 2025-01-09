use super::{InspectorSelection, Ui};
use crate::{
  view::{view2d, view3d, EditorCamera2d, EditorCamera3d, ViewState},
  EditorState, LogInfo,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui;
use bevy_inspector_egui::reflect_inspector::ui_for_value;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct ControlPanel;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  type_registry: Res<'w, AppTypeRegistry>,
  selection: Res<'w, InspectorSelection>,
  editor_state: Res<'w, State<EditorState>>,
  next_editor_state: ResMut<'w, NextState<EditorState>>,
  view_state: Res<'w, State<ViewState>>,
  next_view_state: ResMut<'w, NextState<ViewState>>,
  log_info: ResMut<'w, LogInfo>,
  q_transforms: ParamSet<
    'w,
    's,
    (
      Query<'w, 's, &'static Transform>,
      Query<'w, 's, &'static mut Transform, With<EditorCamera2d>>,
      Query<'w, 's, &'static mut Transform, With<EditorCamera3d>>,
    ),
  >,
}

impl Ui for ControlPanel {
  const NAME: &str = "Control Panel";
  const UUID: uuid::Uuid = uuid!("9473f6e1-a595-41e2-8e29-a4f041580fa6");

  type Params<'w, 's> = Params<'w, 's>;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
    let type_registry = params.type_registry.as_ref().read();

    match params.editor_state.as_ref().get() {
      EditorState::Editing => {
        if ui.button("▶").clicked() {
          params.next_editor_state.set(EditorState::Testing);
        }

        let mut view = params.view_state.get().clone();
        let prev_view = view;
        ui.push_id("view-selector", |ui| {
          ui_for_value(&mut view, ui, &type_registry);
        });
        if prev_view != view {
          params.next_view_state.set(view);
        }

        let InspectorSelection::Entities(selected_entities) = params.selection.as_ref() else {
          return;
        };

        if selected_entities.len() == 1 {
          if ui.button("Move To Selected").clicked() {
            'move_block: {
              let entity = selected_entities.iter().next().unwrap();

              let all_transforms = params.q_transforms.p0();
              let Ok(transform) = all_transforms.get(entity) else {
                break 'move_block;
              };

              let entity_pos = transform.translation;

              match view {
                ViewState::Camera2D => {
                  for mut cam_transform in &mut params.q_transforms.p1() {
                    cam_transform.translation = entity_pos;
                  }
                }
                ViewState::Camera3D => {
                  for mut cam_transform in &mut params.q_transforms.p2() {
                    cam_transform.translation = entity_pos;
                  }
                }
                _ => (),
              }
            }
          }

          if ui.button("Look At Selected").clicked() {
            'look_block: {
              let entity = selected_entities.iter().next().unwrap();
              let all_transforms = params.q_transforms.p0();
              let Ok(transform) = all_transforms.get(entity) else {
                break 'look_block;
              };

              let entity_pos = transform.translation;

              match view {
                ViewState::Camera2D => {
                  for mut cam_transform in &mut params.q_transforms.p1() {
                    cam_transform.look_at(entity_pos, view2d::UP);
                  }
                }
                ViewState::Camera3D => {
                  for mut cam_transform in &mut params.q_transforms.p2() {
                    cam_transform.look_at(entity_pos, view3d::UP);
                  }
                }
                _ => (),
              }
            }
          }
        }
      }
      EditorState::Testing => {
        if ui.button("⏸").clicked() {
          params.next_editor_state.set(EditorState::Editing);
        }
      }
      _ => (),
    };

    ui.push_id("log-level-selector", |ui| {
      ui_for_value(&mut params.log_info.level, ui, &type_registry);
    });
  }
}
