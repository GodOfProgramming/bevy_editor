use bevy::prelude::*;
use derive_new::new;
use egui_dock::{DockState, NodeIndex, SurfaceIndex};

use super::{
  managers::UiManager,
  misc::{DockExtensions, MissingUi},
  PersistentId,
};

#[derive(Event, new)]
pub struct SaveLayoutEvent {
  name: String,
  dock: DockState<Entity>,
}

impl SaveLayoutEvent {
  pub fn on_event(
    mut events: EventReader<SaveLayoutEvent>,
    mut ui_manager: ResMut<UiManager>,
    q_uuids: Query<&PersistentId, Without<MissingUi>>,
    q_missing: Query<&MissingUi>,
  ) {
    for save_event in events.read() {
      let dock = save_event.dock.decouple(&q_uuids, &q_missing);
      ui_manager.save_layout(&save_event.name, dock);
    }
  }
}

#[derive(Event, new, Clone, Copy)]
pub struct AddUiEvent(SurfaceIndex, NodeIndex, Entity);

impl AddUiEvent {
  pub fn on_event(mut events: EventReader<AddUiEvent>, mut ui_manager: ResMut<UiManager>) {
    for event in events.read() {
      let AddUiEvent(surface, node, tab) = *event;

      let Some(surface) = ui_manager.surface_mut(surface) else {
        continue;
      };

      let Some(nodes) = surface.node_tree_mut() else {
        continue;
      };

      let node = &mut nodes[node];
      node.append_tab(tab);
    }
  }
}

#[derive(Event, new, Clone, Copy)]
pub struct RemoveUiEvent(Entity);

impl RemoveUiEvent {
  pub fn on_event(mut events: EventReader<RemoveUiEvent>, mut commands: Commands) {
    for event in events.read() {
      let RemoveUiEvent(tab) = *event;
      commands.entity(tab).despawn();
    }
  }
}
