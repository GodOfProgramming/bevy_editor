use crate::ui::{InspectorSelection, Ui};
use crate::view::{self, EditorCamera};
use crate::{view::ActiveEditorCamera, EditorState, LogInfo};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui;
use bevy_inspector_egui::reflect_inspector::ui_for_value;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct ControlPanel;

impl ControlPanel {
  fn play_button(&self, ui: &mut egui::Ui, params: &mut Params) {
    if ui.button("▶").clicked() {
      params.next_editor_state.set(EditorState::Testing);
    }
  }

  fn pause_button(&self, ui: &mut egui::Ui, params: &mut Params) {
    if ui.button("⏸").clicked() {
      params.next_editor_state.set(EditorState::Editing);
    }
  }

  fn camera_selector(&self, ui: &mut egui::Ui, params: &mut Params) -> ActiveEditorCamera {
    let type_registry = params.type_registry.as_ref().read();
    let mut editor_camera = *params.editor_camera.get();
    let prev_view = editor_camera;

    ui.push_id("camera-selector", |ui| {
      ui.horizontal(|ui| {
        ui.label("Active Camera");
        ui_for_value(&mut editor_camera, ui, &type_registry);
      });
    });

    if prev_view != editor_camera {
      params.next_view_state.set(editor_camera);
    }

    editor_camera
  }

  fn look_at_origin_button(&self, ui: &mut egui::Ui, params: &mut Params) {
    if ui.button("Look At Origin").clicked() {
      for mut cam in &mut params.q_editor_camera_transforms {
        cam.look_at(Vec3::ZERO, view::UP);
      }
    }
  }

  fn entity_commands(
    &self,
    ui: &mut egui::Ui,
    params: &mut Params,
    editor_camera_type: ActiveEditorCamera,
  ) {
    let InspectorSelection::Entities(selected_entities) = params.selection.as_ref() else {
      return;
    };

    if selected_entities.len() == 1 {
      let entity = selected_entities.iter().next().unwrap();

      if editor_camera_type == ActiveEditorCamera::Cam2D
        || editor_camera_type == ActiveEditorCamera::Cam3D
      {
        self.move_to_target_button(ui, params, entity);

        if editor_camera_type == ActiveEditorCamera::Cam3D {
          self.look_at_target_button(ui, params, entity);
        }
      }
    }
  }

  fn move_to_target_button(&self, ui: &mut egui::Ui, params: &mut Params, entity: Entity) {
    if ui.button("Move To Selected").clicked() {
      'move_block: {
        let Ok(transform) = params.q_transforms.get(entity) else {
          break 'move_block;
        };

        let entity_pos = transform.translation;

        for mut cam in &mut params.q_editor_camera_transforms {
          cam.translation = entity_pos;
        }
      }
    }
  }

  fn look_at_target_button(&self, ui: &mut egui::Ui, params: &mut Params, entity: Entity) {
    if ui.button("Look At Selected").clicked() {
      'look_block: {
        let Ok(transform) = params.q_transforms.get(entity) else {
          break 'look_block;
        };

        let entity_pos = transform.translation;

        for mut cam_transform in &mut params.q_editor_camera_transforms {
          cam_transform.look_at(entity_pos, view::UP);
        }
      }
    }
  }

  fn log_level_selector(&self, ui: &mut egui::Ui, params: &mut Params) {
    ui.push_id("log-level-selector", |ui| {
      ui.horizontal(|ui| {
        let type_registry = params.type_registry.as_ref().read();

        ui.label("Log Level");
        ui_for_value(&mut params.log_info.level, ui, &type_registry);
      });
    });
  }
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  type_registry: Res<'w, AppTypeRegistry>,
  selection: Res<'w, InspectorSelection>,
  editor_state: Res<'w, State<EditorState>>,
  next_editor_state: ResMut<'w, NextState<EditorState>>,
  editor_camera: Res<'w, State<ActiveEditorCamera>>,
  next_view_state: ResMut<'w, NextState<ActiveEditorCamera>>,
  log_info: ResMut<'w, LogInfo>,
  q_transforms: Query<'w, 's, &'static Transform, Without<EditorCamera>>,
  q_editor_camera_transforms: Query<'w, 's, &'static mut Transform, With<EditorCamera>>,
}

impl Ui for ControlPanel {
  const NAME: &str = "Control Panel";
  const ID: uuid::Uuid = uuid!("9473f6e1-a595-41e2-8e29-a4f041580fa6");

  type Params<'w, 's> = Params<'w, 's>;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
    match params.editor_state.as_ref().get() {
      EditorState::Editing => {
        self.play_button(ui, &mut params);
        let editor_camera_type = self.camera_selector(ui, &mut params);

        if editor_camera_type == ActiveEditorCamera::Cam3D {
          self.look_at_origin_button(ui, &mut params);
        }

        self.entity_commands(ui, &mut params, editor_camera_type);
      }
      EditorState::Testing => {
        self.pause_button(ui, &mut params);
      }
      _ => (),
    }

    self.log_level_selector(ui, &mut params);
  }
}
