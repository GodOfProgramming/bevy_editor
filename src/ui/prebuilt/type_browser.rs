use crate::{Ui, ui::InspectorSelection};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui;
use bui::PrimaryType;
use itertools::Itertools;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct TypeBrowser;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  commands: Commands<'w, 's>,

  app_type_registry: Res<'w, AppTypeRegistry>,
  selection: ResMut<'w, InspectorSelection>,

  filter: Local<'s, String>,
}

impl Ui for TypeBrowser {
  const NAME: &str = "Type Browser";

  const ID: uuid::Uuid = uuid!("3c1e4565-fd52-498f-892f-dfabbab3c7ef");

  type Params<'w, 's> = Params<'w, 's>;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    Self
  }

  fn render(&mut self, ui: &mut bevy_egui::egui::Ui, mut params: Self::Params<'_, '_>) {
    ui.text_edit_singleline(&mut *params.filter);

    let type_registry = params.app_type_registry.read();
    let tr_iter = type_registry.iter().filter_map(|tr| {
      let name = tr.type_info().type_path();
      (params.filter.is_empty()
        || name
          .to_lowercase()
          .contains(params.filter.to_lowercase().as_str()))
      .then(|| (name, tr.type_id()))
    });
    let tr_iter = tr_iter.sorted_by(|a, b| a.0.cmp(b.0));
    let total_rows = tr_iter.len();
    let row_height_sans_spacing = ui.spacing().interact_size.y;

    let available_size = ui.available_size();
    egui::ScrollArea::new([false, true]).show_rows(
      ui,
      row_height_sans_spacing,
      total_rows,
      |ui, range| {
        ui.set_min_size(available_size);
        ui.set_max_size(available_size);

        for (name, type_id) in tr_iter.skip(range.start) {
          if ui.button(name).clicked() {
            if let InspectorSelection::Entities(entities) = &*params.selection {
              for entity in entities.iter() {
                params
                  .commands
                  .entity(entity)
                  .insert(PrimaryType::from(type_id));
              }
            }
          }
        }
      },
    );
  }

  fn handle_tab_response(&mut self, _params: Self::Params<'_, '_>, response: &egui::Response) {
    response.clone().on_hover_text("Any selected entities from the Hierarchy panel will have a PrimaryType applied to them with the selected type. This makes them serializable UI");
  }

  fn unique() -> bool {
    true
  }
}
