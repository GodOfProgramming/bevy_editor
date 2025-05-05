use bevy::{
  ecs::{component::Component, world::FromWorld},
  reflect::Reflect,
};

#[derive(Default)]
pub struct ComponentRegistry {}

pub trait RegistrableComponent: FromWorld {
  fn register(component_registry: &mut ComponentRegistry);
}

impl<T> RegistrableComponent for T
where
  T: Reflect + FromWorld,
{
  fn register(component_registry: &mut ComponentRegistry) {}
}
