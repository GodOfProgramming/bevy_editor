use super::ParameterizedUi;
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui;
use std::marker::PhantomData;

#[derive(Resource)]
pub struct GameView {
  viewport_rect: egui::Rect,
  mouse_hovered: bool,
}

impl Default for GameView {
  fn default() -> Self {
    Self {
      viewport_rect: egui::Rect::NOTHING,
      mouse_hovered: false,
    }
  }
}

impl GameView {
  pub fn viewport(&self) -> egui::Rect {
    self.viewport_rect
  }

  pub fn hovered(&self) -> bool {
    self.mouse_hovered
  }
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  #[system_param(ignore)]
  _pd: PhantomData<(&'w (), &'s ())>,
}

impl ParameterizedUi for GameView {
  type Params<'w, 's> = Params<'w, 's>;

  fn render(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {
    self.viewport_rect = ui.clip_rect();
    self.mouse_hovered = ui.ui_contains_pointer();
  }

  fn can_clear(&self) -> bool {
    false
  }
}
