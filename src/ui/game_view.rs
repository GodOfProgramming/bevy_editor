use super::ParameterizedUi;
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui;
use std::marker::PhantomData;

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
  #[allow(unused)]
  pub fn viewport(&self) -> egui::Rect {
    self.viewport_rect
  }

  #[allow(unused)]
  pub fn hovered(&self) -> bool {
    self.mouse_hovered
  }
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  ui_state: ResMut<'w, super::State>,
  _pd: PhantomData<&'s ()>,
}

impl ParameterizedUi for GameView {
  type Params<'w, 's> = Params<'w, 's>;

  fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
    params.ui_state.viewport_rect = ui.clip_rect();
    params.ui_state.game_view_hovered = ui.ui_contains_pointer();
  }

  fn can_clear(&self) -> bool {
    false
  }
}
