use crate::{
  Ui,
  registry::components::{ComponentRegistry, RegisteredComponent},
  ui::components::{Card, horizontal_list},
  util::VDir,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui::{self};
use itertools::Itertools;
use std::marker::PhantomData;
use uuid::uuid;

use super::InspectorDnd;

#[derive(Component, Reflect)]
pub struct Components {
  components_per_row: usize,
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

  current_dir: Local<'s, Option<&'static VDir<RegisteredComponent>>>,

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

    horizontal_list(ui, num_columns, components, |ui, (id, comp)| {
      let card_width = ui.available_width();
      let card_height = card_width;

      let id = *id;
      ui.dnd_drag_source(egui::Id::new(id), InspectorDnd::AddComponent(id), |ui| {
        Card::new((card_width, card_height))
          .with_label(comp.name())
          .show(ui, |ui| {
            let text =
              egui::RichText::new(egui_phosphor::regular::PUZZLE_PIECE).size(card_width / 3.0);
            ui.label(text);
          });
      });
    });

    let components = if let Some(current_dir) = &*params.current_dir {
      current_dir
    } else {
      params.component_registry.root_dir()
    };

    horizontal_list(ui, num_columns, components.subdirs(), |ui, (name, comp)| {
      let card_width = ui.available_width();
      let card_height = card_width;

      let resp = Card::new((card_width, card_height))
        .with_label(name)
        .show(ui, |ui| {
          let text = egui::RichText::new(egui_phosphor::regular::FOLDER).size(card_width / 3.0);
          ui.label(text);
        });

      if resp.clicked() {
        info!("Clicked?");
      }

      if resp.double_clicked() {
        info!("Enter new folder {name}");
      }
    });

    horizontal_list(ui, num_columns, components.items(), |ui, (name, comp)| {
      let card_width = ui.available_width();
      let card_height = card_width;

      let id = comp.type_id();
      ui.dnd_drag_source(egui::Id::new(id), InspectorDnd::AddComponent(id), |ui| {
        Card::new((card_width, card_height))
          .with_label(name)
          .show(ui, |ui| {
            let text =
              egui::RichText::new(egui_phosphor::regular::PUZZLE_PIECE).size(card_width / 3.0);
            ui.label(text);
          });
      });
    });
  }

  fn unique() -> bool {
    true
  }

  fn scroll_bars(&self, _params: Self::Params<'_, '_>) -> [bool; 2] {
    [false, true]
  }
}
