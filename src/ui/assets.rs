use super::{InspectorSelection, Ui};
use bevy::{asset::ReflectAsset, prelude::*};
use bevy_egui::egui;

#[derive(Default, Resource, Reflect)]
pub struct Assets;

impl Ui for Assets {
  fn title(&mut self) -> egui::WidgetText {
    stringify!(Assets).into()
  }

  fn render(&mut self, ui: &mut egui::Ui, world: &mut World) {
    let type_registry = world.resource::<AppTypeRegistry>().0.clone();
    let type_registry = type_registry.read();

    let mut assets = type_registry
      .iter()
      .filter_map(|registration| {
        let reflect_asset = registration.data::<ReflectAsset>()?;
        Some((
          registration.type_info().type_path_table().short_path(),
          registration.type_id(),
          reflect_asset,
        ))
      })
      .collect::<Vec<_>>();

    assets.sort_by(|(name_a, ..), (name_b, ..)| name_a.cmp(name_b));

    world.resource_scope(|world, mut selection: Mut<InspectorSelection>| {
      for (asset_name, asset_type_id, reflect_asset) in assets {
        let handles = reflect_asset.ids(world).collect::<Vec<_>>();

        ui.collapsing(format!("{asset_name} ({})", handles.len()), |ui| {
          for handle in handles {
            let selected = match selection.as_ref() {
              InspectorSelection::Asset(_, _, selected_id) => *selected_id == handle,
              _ => false,
            };

            if ui
              .selectable_label(selected, format!("{:?}", handle))
              .clicked()
            {
              *selection = InspectorSelection::Asset(asset_type_id, asset_name.to_string(), handle);
            }
          }
        });
      }
    });
  }
}
