use bevy::{ecs::system::SystemId, prelude::*, utils::TypeIdMap};
use derive_new::new;
use std::any::TypeId;

#[derive(Component)]
pub struct EventProducer;

pub trait BuiEvent: Sized {
  fn create(world: &mut World, default_value: Option<Self>) -> Self;
}

impl<T> BuiEvent for T
where
  T: FromWorld,
{
  fn create(world: &mut World, default_value: Option<Self>) -> Self {
    default_value.unwrap_or_else(|| T::from_world(world))
  }
}

#[derive(Event, Deref, DerefMut)]
pub struct EntityEvent<T> {
  entity: Entity,

  #[deref]
  inner: T,
}

impl<T> EntityEvent<T> {
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

#[derive(new, Component, Reflect)]
#[reflect(Component)]
pub struct ClickEventType(TypeId, #[reflect(ignore)] Option<Box<dyn Reflect>>);

impl ClickEventType {
  pub fn type_id(&self) -> TypeId {
    self.0
  }

  pub fn initializer(&self) -> Option<&dyn Reflect> {
    self.1.as_deref()
  }
}

#[derive(new, Component, Reflect)]
#[reflect(Component)]
pub struct HoverEventType(TypeId, #[reflect(ignore)] Option<Box<dyn Reflect>>);

impl HoverEventType {
  pub fn type_id(&self) -> TypeId {
    self.0
  }

  pub fn initializer(&self) -> Option<&dyn Reflect> {
    self.1.as_deref()
  }
}

#[derive(new, Component, Reflect)]
#[reflect(Component)]
pub struct LeaveEventType(TypeId, #[reflect(ignore)] Option<Box<dyn Reflect>>);

impl LeaveEventType {
  pub fn type_id(&self) -> TypeId {
    self.0
  }

  pub fn initializer(&self) -> Option<&dyn Reflect> {
    self.1.as_deref()
  }
}

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
