use std::marker::PhantomData;

use super::{NoParams, Ui};
use bevy::prelude::*;
use bevy_egui::egui;
use uuid::uuid;

#[derive(Component, Reflect)]
pub struct GameView<C>
where
  C: Component + Reflect,
{
  viewport_rect: Rect,
  mouse_hovered: bool,
  #[reflect(ignore)]
  _pd: PhantomData<C>,
}

impl<C> Default for GameView<C>
where
  C: Component + Reflect,
{
  fn default() -> Self {
    Self {
      viewport_rect: default(),
      mouse_hovered: default(),
      _pd: PhantomData,
    }
  }
}

impl<C> GameView<C>
where
  C: Component + Reflect,
{
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

impl<C> Ui for GameView<C>
where
  C: Component + Reflect + TypePath,
{
  const NAME: &str = "Game View";
  const UUID: uuid::Uuid = uuid!("f26513f6-86fa-48e2-9f6f-e094ad9dcbfb");

  type Params<'w, 's> = NoParams;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
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

  fn unique() -> bool {
    true
  }
}
