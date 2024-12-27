use super::{PersistentId, UiComponent};
use crate::assets;
use bevy::prelude::*;
use bevy_egui::egui;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct Prefabs;

impl UiComponent for Prefabs {
  const COMPONENT_NAME: &str = stringify!(Prefabs);
  const ID: PersistentId = PersistentId(uuid!("fa977fad-ed99-4842-bab4-7c00641b39b0"));

  fn spawn(_world: &mut World) -> Self {
    default()
  }

  fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
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
