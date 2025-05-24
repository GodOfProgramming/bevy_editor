use bevy::{ecs::system::SystemId, prelude::*, utils::TypeIdMap};
use derive_new::new;
use serde::{Deserialize, Serialize};
use std::any::TypeId;

#[derive(Component)]
pub struct EventProducer;

#[derive(Event, Deref, DerefMut)]
pub struct UiEvent<T> {
  entity: Entity,

  #[deref]
  inner: T,
}

impl<T> UiEvent<T> {
  pub fn new(entity: Entity, event: T) -> Self {
    Self {
      entity,
      inner: event,
    }
  }

  pub fn entity(&self) -> Entity {
    self.entity
  }
}

#[derive(new, Deref, Component, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct ClickEventType(TypeId);

#[derive(new, Deref, Component, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct HoverEventType(TypeId);

#[derive(new, Deref, Component, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct LeaveEventType(TypeId);

type EventSystemIdType = SystemId<In<(Entity, Box<dyn Reflect>)>, Result>;

#[derive(Default, Resource)]
pub struct UiEvents {
  event_systems: TypeIdMap<EventSystemIdType>,
}

impl UiEvents {
  pub fn add<E: 'static>(&mut self, id: EventSystemIdType) {
    self.event_systems.insert(TypeId::of::<E>(), id);
  }

  pub fn get(&self, t: &TypeId) -> Option<&EventSystemIdType> {
    self.event_systems.get(t)
  }
}
