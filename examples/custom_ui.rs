use bevy::prelude::*;
use bevy_editor::{Editor, Ui, misc::NoParams};
use bevy_egui::egui;
use uuid::uuid;

fn main() {
  let mut editor = Editor::default();

  editor
    .register_ui::<CustomPanel>()
    .register_component::<SomeComponent>();

  editor.launch();
}

#[derive(Component, Reflect, Default)]
struct SomeComponent;

#[derive(Reflect, Component)]
struct CustomPanel;

impl Ui for CustomPanel {
  const NAME: &str = "Custom Panel";

  const ID: uuid::Uuid = uuid!("b2d3a7ea-a68c-4788-a9e5-16b51d94ce52");

  type Params<'w, 's> = NoParams;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    Self
  }

  fn render(&mut self, ui: &mut bevy_egui::egui::Ui, _params: Self::Params<'_, '_>) {
    let rect = ui.available_rect_before_wrap();
    let bg_fill = ui.style().visuals.window_fill();
    egui::Frame::default().fill(bg_fill).show(ui, |ui| {
      ui.set_min_size(rect.size());
    });
  }
}
