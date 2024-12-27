use super::{InspectorSelection, PersistentId, SelectedEntities, UiComponent};
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector::hierarchy::hierarchy_ui;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct Hierarchy;

impl UiComponent for Hierarchy {
  const COMPONENT_NAME: &str = stringify!(Hierarchy);
  const ID: PersistentId = PersistentId(uuid!("860ac319-5c6e-4a2e-83ae-8bb0000d5cb4"));

  fn spawn(_world: &mut World) -> Self {
    default()
  }

  fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    world.resource_scope(|world, mut selection: Mut<InspectorSelection>| {
      if let InspectorSelection::Entities(selected_entities) = selection.as_mut() {
        hierarchy_ui(world, ui, selected_entities);
      } else {
        let mut selected_entities = SelectedEntities::default();
        if hierarchy_ui(world, ui, &mut selected_entities) {
          *selection = InspectorSelection::Entities(selected_entities);
        }
      }
    });
  }
}
