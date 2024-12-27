use super::ParameterizedUi;
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui;
use std::marker::PhantomData;

#[derive(Default, Resource, Reflect)]
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

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  #[system_param(ignore)]
  _pd: PhantomData<(&'w (), &'s ())>,
}

impl ParameterizedUi for GameView {
  type Params<'w, 's> = Params<'w, 's>;

  fn title(&mut self) -> egui::WidgetText {
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

  fn can_clear(&self) -> bool {
    false
  }
}
