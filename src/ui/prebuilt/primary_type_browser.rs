use crate::{
  Ui,
  registry::components::ComponentRegistry,
  ui::components::{Card, horizontal_list},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui;
use uuid::uuid;

use super::InspectorDnd;

#[derive(Default, Component, Reflect)]
pub struct PrimaryTypeBrowser {
  components_per_row: usize,
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  component_registry: Res<'w, ComponentRegistry>,

  filter: Local<'s, String>,
}

impl Ui for PrimaryTypeBrowser {
  const NAME: &str = "Primary Type Browser";

  const ID: uuid::Uuid = uuid!("3c1e4565-fd52-498f-892f-dfabbab3c7ef");

  type Params<'w, 's> = Params<'w, 's>;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    Self {
      components_per_row: 10,
    }
  }

  fn render(&mut self, ui: &mut bevy_egui::egui::Ui, mut params: Self::Params<'_, '_>) {
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

    horizontal_list(ui, num_columns, components, |ui, _, (id, comp)| {
      let card_width = ui.available_width();
      let card_height = card_width;

      let id = *id;
      ui.dnd_drag_source(egui::Id::new(id), InspectorDnd::SetPrimaryType(id), |ui| {
        Card::new((card_width, card_height))
          .with_label(comp.name())
          .show(ui, |ui| {
            let text = egui::RichText::new(egui_phosphor::regular::SPARKLE).size(card_width / 3.0);
            ui.label(text);
          });
      });
    });
  }

  fn unique() -> bool {
    true
  }

  fn scroll_bars(&self, _params: Self::Params<'_, '_>) -> [bool; 2] {
    [false, false]
  }
}
