use crate::{
  Ui,
  registry::components::{ComponentRegistry, RegisteredComponent},
  ui::components::BorderedBox,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui::{self};
use std::{any::TypeId, marker::PhantomData};
use uuid::uuid;

#[derive(Component, Reflect)]
pub struct Components {
  components_per_row: usize,
}

impl Components {
  fn draw_card(
    &self,
    ui: &mut egui::Ui,
    id: TypeId,
    comp: &RegisteredComponent,
    card_size: impl Into<egui::Vec2>,
  ) {
    let card_size = card_size.into();
    let border_thickness = card_size.x / 25.0;
    let cell_content_size = card_size.x - border_thickness;
    let icon_thickness = cell_content_size / 3.0;

    ui.dnd_drag_source(egui::Id::new(id), id, |ui| {
      ui.vertical_centered(|ui| {
        ui.set_width(card_size.x);
        ui.set_height(card_size.y);

        self.draw_card_contents(
          ui,
          comp,
          cell_content_size,
          border_thickness,
          icon_thickness,
        );
      });
    });
  }

  fn draw_card_contents(
    &self,
    ui: &mut egui::Ui,
    comp: &RegisteredComponent,
    cell_content_size: f32,
    border_thickness: f32,
    icon_thickness: f32,
  ) {
    BorderedBox::new((0.0, 0.0), (cell_content_size, cell_content_size))
      .with_thickness(border_thickness)
      .draw(ui, |ui| {
        ui.centered_and_justified(|ui| {
          let text = egui::RichText::new(egui_phosphor::regular::PUZZLE_PIECE).size(icon_thickness);
          ui.label(text);
        });
      });

    ui.label(comp.name());
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
  filter: Local<'s, String>,
  _pd: PhantomData<&'s ()>,
}

impl Ui for Components {
  const NAME: &str = "Components";

  const ID: uuid::Uuid = uuid!("5b376389-2acf-4945-807b-94ee16c09088");

  type Params<'w, 's> = Params<'w, 's>;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
    ui.text_edit_singleline(&mut *params.filter);

    let num_columns = self.components_per_row.max(1);

    let components = params
      .component_registry
      .iter()
      .filter(|(_id, comp)| {
        params.filter.is_empty()
          || comp
            .name()
            .to_lowercase()
            .contains(params.filter.to_lowercase().as_str())
      })
      .collect::<Vec<_>>();

    for chunk in components.chunks(num_columns) {
      ui.columns(num_columns, |uis| {
        for (ui, (id, comp)) in uis.iter_mut().zip(chunk.iter()) {
          let card_width = ui.available_width();
          let card_height = card_width;
          self.draw_card(ui, **id, comp, (card_width, card_height));
        }
      });
    }
  }

  fn unique() -> bool {
    true
  }

  fn scroll_bars(&self, _params: Self::Params<'_, '_>) -> [bool; 2] {
    [false, true]
  }
}
