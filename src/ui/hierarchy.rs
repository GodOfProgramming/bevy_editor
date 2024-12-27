use super::{InspectorSelection, SelectedEntities, Ui};
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector::hierarchy::hierarchy_ui;
use uuid::uuid;

#[derive(Default, Resource, Reflect)]
pub struct Hierarchy;

impl Ui for Hierarchy {
  const UUID: uuid::Uuid = uuid!("860ac319-5c6e-4a2e-83ae-8bb0000d5cb4");

  fn title(&mut self) -> egui::WidgetText {
    stringify!(Hierarchy).into()
  }

  fn render(&mut self, ui: &mut egui::Ui, world: &mut World) {
    world.resource_scope(|world, mut selection: Mut<InspectorSelection>| {
      if let InspectorSelection::Entities(selected_entities) = selection.as_mut() {
        hierarchy_ui(world, ui, selected_entities);
      } else {
        let mut selected_entities = SelectedEntities::default();
        if hierarchy_ui(world, ui, &mut selected_entities) {
          *selection = InspectorSelection::Entities(selected_entities);
        }
      }
    })
  }
}