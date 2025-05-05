use bevy::{
  ecs::{component::Component, resource::Resource, world::FromWorld},
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

pub struct RegisteredComponent {
  name: &'static str,
  spawn_fn: fn(entity: Entity, &mut World),
}

impl RegisteredComponent {
  pub fn name(&self) -> &str {
    self.name
  }

  pub fn spawn(&self, entity: Entity, world: &mut World) {
    (self.spawn_fn)(entity, world);
  }
}

pub trait RegistrableComponent: GetTypeRegistration + FromWorld {
  fn register(component_registry: &mut ComponentRegistry);
}

impl<T> RegistrableComponent for T
where
  T: Reflect + GetTypeRegistration + FromWorld + Component,
{
  fn register(component_registry: &mut ComponentRegistry) {
    component_registry.mapping.insert(
      TypeId::of::<T>(),
      RegisteredComponent {
        name: T::get_type_registration().type_info().type_path(),
        spawn_fn: |entity, world| {
          let comp = T::from_world(world);
          world.entity_mut(entity).insert(comp);
        },
      },
    );
  }
}
