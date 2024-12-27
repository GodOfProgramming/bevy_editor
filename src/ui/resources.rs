use std::marker::PhantomData;

use super::{InspectorSelection, ParameterizedUi};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui;

#[derive(Default, Resource, Reflect)]
pub struct Resources;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  type_registry: Res<'w, AppTypeRegistry>,
  selection: ResMut<'w, InspectorSelection>,

  #[system_param(ignore)]
  _pd: PhantomData<&'s ()>,
}

impl ParameterizedUi for Resources {
  type Params<'w, 's> = Params<'w, 's>;

  fn title(&mut self) -> egui::WidgetText {
    stringify!(Resources).into()
  }

  fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
    let type_registry = params.type_registry.read();

    let mut resources: Vec<_> = type_registry
      .iter()
      .filter(|registration| registration.data::<ReflectResource>().is_some())
      .map(|registration| {
        (
          registration.type_info().type_path_table().short_path(),
          registration.type_id(),
        )
      })
      .collect();
    resources.sort_by(|(name_a, _), (name_b, _)| name_a.cmp(name_b));

    for (resource_name, type_id) in resources {
      let selected = match *params.selection {
        InspectorSelection::Resource(selected, _) => selected == type_id,
        _ => false,
      };

      if ui.selectable_label(selected, resource_name).clicked() {
        *params.selection = InspectorSelection::Resource(type_id, resource_name.to_string());
      }
    }
  }
}
