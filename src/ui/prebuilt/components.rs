use super::InspectorDnd;
use crate::{
  Ui,
  registry::components::{ComponentRegistry, RegisteredComponent},
  ui::components::{Card, horizontal_list},
  util::vfs::{VfsDir, VfsNode, VfsPath},
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
    current_path: &mut VfsPath,
    ui: &mut egui::Ui,
    size: impl Into<egui::Vec2>,
    label: &str,
    i: usize,
  ) -> bool {
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
      info!("here");
      current_path.push(String::from(label));
      true
    } else {
      false
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
  current_path: Local<'s, VfsPath>,

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

    if params.current_dir.is_none() {
      *params.current_dir = params
        .component_registry
        .vfs()
        .get_dir(&*params.current_path)
        .cloned();
    }

    let Some(current_dir) = &*params.current_dir else {
      return;
    };

    let components = current_dir.iter().filter(|node| {
      params.filter.is_empty() || {
        node
          .name()
          .to_lowercase()
          .contains(params.filter.to_lowercase().as_str())
      }
    });

    let mut clicked = false;

    horizontal_list(ui, num_columns, components, |ui, i, node| {
      let card_width = ui.available_width();
      let card_height = card_width;

      match node {
        VfsNode::Dir(dir) => {
          clicked = Self::ui_for_dir(
            &mut params.current_path,
            ui,
            (card_width, card_height),
            dir,
            i,
          );
        }
        VfsNode::Item { name, value } => {
          if let Some(component) = params.component_registry.get(value) {
            Self::ui_for_item(ui, (card_width, card_height), name, component);
          }
        }
      }
    });

    if clicked {
      *params.current_dir = params
        .component_registry
        .vfs()
        .get_dir(&*params.current_path)
        .cloned();
    }
  }

  fn unique() -> bool {
    true
  }

  fn scroll_bars(&self, _params: Self::Params<'_, '_>) -> [bool; 2] {
    [false, true]
  }
}
