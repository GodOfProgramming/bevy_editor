use super::{NoParams, Ui};
use bevy::prelude::*;
use bevy_egui::egui;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct GameView {
  viewport_rect: Rect,
  mouse_hovered: bool,
}

impl GameView {
  pub fn viewport(&self) -> egui::Rect {
    egui::Rect {
      max: egui::Pos2::new(self.viewport_rect.max.x, self.viewport_rect.max.y),
      min: egui::Pos2::new(self.viewport_rect.min.x, self.viewport_rect.min.y),
    }
  }

  pub fn hovered(&self) -> bool {
    self.mouse_hovered
  }
}

impl Ui for GameView {
  type Params<'w, 's> = NoParams;
  const UUID: uuid::Uuid = uuid!("c910a397-a017-4a29-99bc-6282b4b1a214");

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn title(&mut self, _params: Self::Params<'_, '_>) -> egui::WidgetText {
    "Game View".into()
  }

  fn render(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {
    let egui_rect = ui.clip_rect();
    self.viewport_rect = Rect {
      max: Vec2::new(egui_rect.max.x, egui_rect.max.y),
      min: Vec2::new(egui_rect.min.x, egui_rect.min.y),
    };
    self.mouse_hovered = ui.ui_contains_pointer();
  }

  fn can_clear(&self, _params: Self::Params<'_, '_>) -> bool {
    false
  }
}
