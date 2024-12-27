use super::Ui;
use crate::assets;
use bevy::prelude::*;
use bevy_egui::egui;

#[derive(Default, Resource, Reflect)]
pub struct Prefabs;

impl Ui for Prefabs {
  fn title(&mut self) -> egui::WidgetText {
    stringify!(Prefabs).into()
  }

  fn render(&mut self, ui: &mut egui::Ui, world: &mut World) {
    world.resource_scope(|world, mut prefabs: Mut<assets::Prefabs>| {
      let mut prefab_ids = prefabs.keys().cloned().collect::<Vec<_>>();

      prefab_ids.sort();

      for id in prefab_ids {
        ui.horizontal(|ui| {
          ui.label(&id);
          if ui.button("Spawn").clicked() {
            prefabs.spawn(id, world);
          }
        });
      }
    });
  }
}
