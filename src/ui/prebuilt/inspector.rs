use crate::ui::{InspectorSelection, RawUi};
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector::{
  by_type_id::{ui_for_asset, ui_for_resource},
  ui_for_entities_shared_components, ui_for_entity_with_children,
};
use uuid::{Uuid, uuid};

#[derive(Default, Component, Reflect)]
pub struct Inspector;

impl RawUi for Inspector {
  const NAME: &str = stringify!(Inspector);
  const ID: Uuid = uuid!("10bb68b8-c247-4792-89e9-61d1b9682a72");

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    let type_registry = world.resource::<AppTypeRegistry>().0.clone();
    let type_registry = type_registry.read();

    world.resource_scope(
      |world, selection: Mut<InspectorSelection>| match selection.as_ref() {
        InspectorSelection::Entities(selected_entities) => match selected_entities.as_slice() {
          &[entity] => ui_for_entity_with_children(world, entity, ui),
          entities => ui_for_entities_shared_components(world, entities, ui),
        },
        InspectorSelection::Resource(type_id, name) => {
          ui.label(name);
          ui_for_resource(world, *type_id, ui, name, &type_registry)
        }
        InspectorSelection::Asset(type_id, name, handle) => {
          ui.label(name);
          ui_for_asset(world, *type_id, *handle, ui, &type_registry);
        }
      },
    );
  }
}
