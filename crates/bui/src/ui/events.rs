use bevy::{ecs::system::SystemId, prelude::*, utils::TypeIdMap};
use derive_new::new;
use std::any::TypeId;

#[derive(Component)]
pub struct Interactable;

#[derive(new, Deref, Component, Clone, Copy)]
pub struct ClickEventType(TypeId);

#[derive(new, Deref, Component, Clone, Copy)]
pub struct HoverEventType(TypeId);

#[derive(new, Deref, Component, Clone, Copy)]
pub struct LeaveEventType(TypeId);

#[derive(Default, Resource)]
pub struct UiEvents {
  inner: TypeIdMap<SystemId<In<Box<dyn Reflect>>, Result>>,
}

impl UiEvents {
  pub fn add<E: Event>(&mut self, id: SystemId<In<Box<dyn Reflect>>, Result>) {
    self.inner.insert(TypeId::of::<E>(), id);
  }

  pub fn get(&self, t: &TypeId) -> Option<&SystemId<In<Box<dyn Reflect>>, Result>> {
    self.inner.get(t)
  }
}
