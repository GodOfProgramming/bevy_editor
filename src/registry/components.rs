use bevy::{
  ecs::{
    component::{Component, ComponentId},
    resource::Resource,
    world::FromWorld,
  },
  prelude::*,
  reflect::{GetTypeRegistration, Reflect},
  utils::TypeIdMap,
};
use std::any::TypeId;

#[derive(Default, Resource)]
pub struct ComponentRegistry {
  mapping: TypeIdMap<RegisteredComponent>,
}

impl ComponentRegistry {
  pub fn get(&self, type_id: &TypeId) -> Option<&RegisteredComponent> {
    self.mapping.get(type_id)
  }

  pub fn iter(&self) -> impl Iterator<Item = (&TypeId, &RegisteredComponent)> {
    self.mapping.iter()
  }
}

#[derive(Clone)]
pub struct RegisteredComponent {
  name: &'static str,
  spawn_fn: fn(entity: Entity, &mut World),
  id: ComponentId,
}

impl RegisteredComponent {
  pub fn name(&self) -> &str {
    self.name
  }

  pub fn spawn(&self, entity: Entity, world: &mut World) {
    (self.spawn_fn)(entity, world);
  }

  pub fn id(&self) -> ComponentId {
    self.id
  }
}

pub trait RegistrableComponent: GetTypeRegistration + FromWorld + Component {
  fn register(component_registry: &mut ComponentRegistry, id: ComponentId);
}

impl<T> RegistrableComponent for T
where
  T: Reflect + GetTypeRegistration + FromWorld + Component,
{
  fn register(component_registry: &mut ComponentRegistry, id: ComponentId) {
    component_registry.mapping.insert(
      TypeId::of::<T>(),
      RegisteredComponent {
        name: T::get_type_registration().type_info().type_path(),
        spawn_fn: |entity, world| {
          let comp = T::from_world(world);
          world.entity_mut(entity).insert(comp);
        },
        id,
      },
    );
  }
}
