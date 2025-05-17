use bevy::{ecs::system::SystemId, prelude::*, utils::TypeIdMap};
use derive_new::new;
use std::any::TypeId;

#[derive(Component)]
pub struct Interactable;

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

#[derive(new, Deref, Component, Clone, Copy)]
pub struct ClickEventType(TypeId);

#[derive(new, Deref, Component, Clone, Copy)]
pub struct HoverEventType(TypeId);

#[derive(new, Deref, Component, Clone, Copy)]
pub struct LeaveEventType(TypeId);

type EventSystemIdType = SystemId<In<(Entity, Box<dyn Reflect>)>, Result>;

#[derive(Default, Resource)]
pub struct UiEvents {
  inner: TypeIdMap<EventSystemIdType>,
}

impl UiEvents {
  pub fn add<E: 'static>(&mut self, id: EventSystemIdType) {
    self.inner.insert(TypeId::of::<E>(), id);
  }

  pub fn get(&self, t: &TypeId) -> Option<&EventSystemIdType> {
    self.inner.get(t)
  }
}
