use crate::view::ActiveEditorCamera;

use super::{NoParams, Ui};
use bevy::{prelude::*, render::camera::Viewport, window::PrimaryWindow};
use bevy_egui::egui;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct EditorView {
  viewport_rect: Rect,
  mouse_hovered: bool,
  was_rendered: bool,
}

impl EditorView {
  pub fn viewport(&self) -> egui::Rect {
    egui::Rect {
      max: egui::Pos2::new(self.viewport_rect.max.x, self.viewport_rect.max.y),
      min: egui::Pos2::new(self.viewport_rect.min.x, self.viewport_rect.min.y),
    }
  }

  pub fn hovered(&self) -> bool {
    self.mouse_hovered
  }

  fn on_preupdate(mut editor_view: Single<&mut Self>) {
    editor_view.was_rendered = false;
  }

  fn set_viewport(
    window: Single<&Window, With<PrimaryWindow>>,
    egui_settings: Single<&bevy_egui::EguiSettings>,
    editor_view: Single<&Self>,
    mut q_cameras: Query<&mut Camera, With<ActiveEditorCamera>>,
  ) {
    if editor_view.was_rendered {
      for mut camera in &mut q_cameras {
        camera.is_active = true;
        let scale_factor = window.scale_factor() * egui_settings.scale_factor;

        let viewport = editor_view.viewport();
        let viewport_pos = viewport.left_top().to_vec2() * scale_factor;
        let viewport_size = viewport.size() * scale_factor;

        let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
        let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

        // The desired viewport rectangle at its offset in "physical pixel space"
        let rect = physical_position + physical_size;

        let window_size = window.physical_size();
        if rect.x <= window_size.x && rect.y <= window_size.y {
          camera.viewport = Some(Viewport {
            physical_position,
            physical_size,
            depth: 0.0..1.0,
          });
        }
      }
    } else {
      for mut camera in &mut q_cameras {
        camera.is_active = false;
      }
    }
  }
}

impl Ui for EditorView {
  const NAME: &str = "Editor View";
  const UUID: uuid::Uuid = uuid!("c910a397-a017-4a29-99bc-6282b4b1a214");

  type Params<'w, 's> = NoParams;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn can_clear(&self, _params: Self::Params<'_, '_>) -> bool {
    false
  }

  fn unique() -> bool {
    true
  }

  fn render(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {
    self.was_rendered = true;

    let egui_rect = ui.clip_rect();

    self.viewport_rect = Rect {
      max: Vec2::new(egui_rect.max.x, egui_rect.max.y),
      min: Vec2::new(egui_rect.min.x, egui_rect.min.y),
    };

    self.mouse_hovered = ui.ui_contains_pointer();
  }

  fn init(app: &mut App) {
    app
      .add_systems(PreUpdate, Self::on_preupdate)
      .add_systems(PostUpdate, Self::set_viewport);
  }
}
