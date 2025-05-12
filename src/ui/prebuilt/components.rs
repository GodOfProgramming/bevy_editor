use crate::{Ui, registry::components::ComponentRegistry, ui::components::BorderedBox};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui::{self};
use std::marker::PhantomData;
use uuid::uuid;

#[derive(Component, Reflect)]
pub struct Components {
  components_per_row: usize,
}

impl Components {
  fn draw_components(
    &mut self,
    ui: &mut egui::Ui,
    params: &Params<'_, '_>,
    max_cell_size: f32,
    cell_content_size: f32,
    border_thickness: f32,
    icon_thickness: f32,
  ) {
    for (i, (id, comp)) in params.component_registry.iter().enumerate() {
      ui.allocate_ui(egui::Vec2::from([max_cell_size, max_cell_size]), |ui| {
        ui.dnd_drag_source(egui::Id::new(id), *id, |ui| {
          ui.vertical_centered(|ui| {
            BorderedBox::new((0.0, 0.0), (cell_content_size, cell_content_size))
              .with_thickness(border_thickness)
              .draw(ui, |ui| {
                ui.centered_and_justified(|ui| {
                  let text =
                    egui::RichText::new(egui_phosphor::regular::PUZZLE_PIECE).size(icon_thickness);
                  ui.label(text);
                });
              });
            ui.label(comp.name());
          });
        });
      });

      if (i + 1) % self.components_per_row == 0 {
        ui.end_row();
      }
    }
  }
}

impl Default for Components {
  fn default() -> Self {
    Self {
      components_per_row: 10,
    }
  }
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  component_registry: Res<'w, ComponentRegistry>,
  _pd: PhantomData<&'s ()>,
}

impl Ui for Components {
  const NAME: &str = "Components";

  const ID: uuid::Uuid = uuid!("5b376389-2acf-4945-807b-94ee16c09088");

  type Params<'w, 's> = Params<'w, 's>;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
    let rect = ui.clip_rect();

    let width = rect.width();

    let max_cell_size = if self.components_per_row > 1 {
      width / self.components_per_row as f32
    } else {
      width
    };
    let border_thickness = max_cell_size / 25.0;

    let cell_content_size = max_cell_size - border_thickness;
    let icon_thickness = cell_content_size / 3.0;

    egui::Grid::new("components")
      .num_columns(self.components_per_row)
      .min_col_width(max_cell_size)
      .max_col_width(max_cell_size)
      .show(ui, |ui| {
        self.draw_components(
          ui,
          &params,
          max_cell_size,
          cell_content_size,
          border_thickness,
          icon_thickness,
        );
      });
  }

  fn unique() -> bool {
    true
  }
}
