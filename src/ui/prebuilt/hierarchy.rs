use crate::ui::{InspectorSelection, RawUi, SelectedEntities};
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector::hierarchy::hierarchy_ui;
use uuid::{uuid, Uuid};

#[derive(Default, Component, Reflect)]
pub struct Hierarchy;

impl RawUi for Hierarchy {
  const NAME: &str = stringify!(Hierarchy);
  const ID: Uuid = uuid!("860ac319-5c6e-4a2e-83ae-8bb0000d5cb4");

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    default()
  }

  fn unique() -> bool {
    true
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
