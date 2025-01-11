use crate::ui::{misc::UiInfo, Ui};
use bevy::{ecs::system::SystemParam, prelude::*, render::camera::Viewport, window::PrimaryWindow};
use bevy_egui::egui;
use std::marker::PhantomData;
use uuid::uuid;

#[derive(Component, Reflect)]
pub struct GameView<C>
where
  C: Component + Reflect,
{
  viewport_rect: Rect,
  was_rendered: bool,
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
      was_rendered: false,
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

  fn on_preupdate(mut game_view: Single<&mut Self>) {
    game_view.was_rendered = false;
  }

  fn set_viewport(
    window: Single<&Window, With<PrimaryWindow>>,
    egui_settings: Single<&bevy_egui::EguiSettings>,
    game_view: Single<(&Self, &UiInfo)>,
    mut q_cameras: Query<&mut Camera, With<C>>,
  ) {
    let (game_view, ui_info) = &*game_view;

    if ui_info.rendered() {
      for mut camera in &mut q_cameras {
        let scale_factor = window.scale_factor() * egui_settings.scale_factor;

        let viewport = game_view.viewport();
        let viewport_pos = viewport.left_top().to_vec2() * scale_factor;
        let viewport_size = viewport.size() * scale_factor;

        let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
        let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

        // The desired viewport rectangle at its offset in "physical pixel space"
        let rect = physical_position + physical_size;

        let window_size = window.physical_size();
        if rect.x <= window_size.x && rect.y <= window_size.y {
          let depth = camera
            .viewport
            .as_ref()
            .map(|vp| vp.depth.clone())
            .unwrap_or(0.0..1.0);

          camera.viewport = Some(Viewport {
            physical_position,
            physical_size,
            depth,
          });
        }
      }
    }
  }
}

#[derive(SystemParam)]
pub struct Params<'w, 's, C: Component> {
  q_cameras: Query<'w, 's, &'static mut Camera, With<C>>,
}

impl<C> Ui for GameView<C>
where
  C: Component + Reflect + TypePath,
{
  const NAME: &str = "Game View";
  const ID: uuid::Uuid = uuid!("f26513f6-86fa-48e2-9f6f-e094ad9dcbfb");

  type Params<'w, 's> = Params<'w, 's, C>;

  fn init(app: &mut App) {
    app
      .add_systems(PreUpdate, Self::on_preupdate)
      .add_systems(PostUpdate, Self::set_viewport);
  }

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn on_despawn(&mut self, mut params: Self::Params<'_, '_>) {
    for mut camera in &mut params.q_cameras {
      camera.is_active = false;
    }
  }

  fn render(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {
    self.was_rendered = true;

    let egui_rect = ui.clip_rect();
    self.viewport_rect = Rect {
      max: Vec2::new(egui_rect.max.x, egui_rect.max.y),
      min: Vec2::new(egui_rect.min.x, egui_rect.min.y),
    };
  }

  fn when_rendered(&mut self, mut params: Self::Params<'_, '_>) {
    for mut camera in &mut params.q_cameras {
      camera.is_active = true;
    }
  }

  fn when_not_rendered(&mut self, mut params: Self::Params<'_, '_>) {
    for mut camera in &mut params.q_cameras {
      camera.is_active = false;
    }
  }

  fn can_clear(&self, _params: Self::Params<'_, '_>) -> bool {
    false
  }

  fn unique() -> bool {
    true
  }

  fn popout() -> bool {
    false
  }
}
