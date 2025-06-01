use super::InspectorDnd;
use crate::{
  Ui,
  registry::components::{ComponentRegistry, RegisteredComponent},
  ui::components::{Card, horizontal_list},
  util::{VfsDir, VfsNode},
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
  fn ui_for_dir(
    current_dir: &mut Option<VfsDir<TypeId>>,
    ui: &mut egui::Ui,
    size: impl Into<egui::Vec2>,
    label: &str,
    i: usize,
    dir: &VfsDir<TypeId>,
  ) {
    let size = size.into();
    let response = Card::new(size)
      .with_label(label)
      .show(ui, |ui| {
        let text = egui::RichText::new(egui_phosphor::regular::FOLDER).size(size.x / 3.0);
        ui.label(text);

        ui.interact(ui.min_rect(), ui.id().with(i), egui::Sense::click())
      })
      .inner
      .on_hover_cursor(egui::CursorIcon::PointingHand);

    if response.double_clicked() {
      *current_dir = Some(dir.clone());
    }
  }

  fn ui_for_item(
    ui: &mut egui::Ui,
    size: impl Into<egui::Vec2>,
    label: &str,
    component: &RegisteredComponent,
  ) {
    let size = size.into();
    let id = component.type_id();
    ui.dnd_drag_source(egui::Id::new(id), InspectorDnd::AddComponent(id), |ui| {
      Card::new(size).with_label(label).show(ui, |ui| {
        let text = egui::RichText::new(egui_phosphor::regular::PUZZLE_PIECE).size(size.x / 3.0);
        ui.label(text);
      });
    });
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

  current_dir: Local<'s, Option<VfsDir<TypeId>>>,

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
      .root_dir()
      .iter()
      .filter(|(ident, _)| {
        params.filter.is_empty() || {
          ident
            .to_lowercase()
            .contains(params.filter.to_lowercase().as_str())
        }
      });

    horizontal_list(ui, num_columns, components, |ui, i, (name, vdir)| {
      let card_width = ui.available_width();
      let card_height = card_width;

      match vdir {
        VfsNode::Directory(dir) => {
          Self::ui_for_dir(
            &mut params.current_dir,
            ui,
            (card_width, card_height),
            name,
            i,
            dir,
          );
        }
        VfsNode::Item(type_id) => {
          if let Some(component) = params.component_registry.get(type_id) {
            Self::ui_for_item(ui, (card_width, card_height), name, component);
          }
        }
      }
    });
  }

  fn unique() -> bool {
    true
  }

  fn scroll_bars(&self, _params: Self::Params<'_, '_>) -> [bool; 2] {
    [false, true]
  }
}
