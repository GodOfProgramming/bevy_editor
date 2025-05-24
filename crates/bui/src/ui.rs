pub mod attrs;
pub mod elements;
pub mod events;
mod generated;

use bevy::{
  ecs::system::{SystemParam, SystemState},
  prelude::*,
  reflect::{Reflectable, erased_serde::Serialize},
};
use serde::Deserialize;

#[derive(SystemParam)]
pub struct NoParams;

pub trait Element: Reflectable {}

pub trait Attribute: 'static {
  type Params<'w, 's>: for<'world, 'system> SystemParam<
    Item<'world, 'system> = Self::Params<'world, 'system>,
  >;

  fn construct(self, params: Self::Params<'_, '_>) -> Result<impl Bundle>;
}

impl<T> Attribute for T
where
  T: Component + Clone,
{
  type Params<'w, 's> = NoParams;

  fn construct(self, _params: Self::Params<'_, '_>) -> Result<impl Bundle> {
    Ok(self)
  }
}

pub trait SerializableAttribute: 'static {
  type Resources<'w>: Resources<'w>;

  type Out<'de>: Serialize + Deserialize<'de> + Reflect;

  fn serialize(&self, resources: Self::Resources<'_>) -> Result<Self::Out<'_>>;

  fn name_override(&self) -> Option<String> {
    None
  }

  fn prefix_override(&self) -> Option<String> {
    None
  }
}

pub trait Resources<'w>: Sized {
  fn from_world(world: &'w World) -> Option<Self>;
}

impl<'w> Resources<'w> for () {
  fn from_world(_world: &'w World) -> Option<Self> {
    Some(())
  }
}

impl<'w, T> Resources<'w> for &'w T
where
  T: Resource,
{
  fn from_world(world: &'w World) -> Option<Self> {
    world.get_resource::<T>()
  }
}

#[derive(Deref, DerefMut)]
pub struct ResourceCollection<R>(R);

impl<'w, T> Resources<'w> for ResourceCollection<T>
where
  T: IntoResourceCollection<'w>,
{
  fn from_world(world: &'w World) -> Option<Self> {
    <T as IntoResourceCollection<'w>>::from_world(world).map(ResourceCollection)
  }
}

pub trait IntoResourceCollection<'w>: Sized {
  fn from_world(world: &'w World) -> Option<Self>;
}

impl<'w, T0, T1> IntoResourceCollection<'w> for (&'w T0, &'w T1)
where
  T0: Resource,
  T1: Resource,
{
  fn from_world(world: &'w World) -> Option<Self> {
    world.get_resource::<T0>().zip(world.get_resource::<T1>())
  }
}

#[derive(Resource, Deref, DerefMut)]
pub(super) struct TypeState<P>(SystemState<P>)
where
  P: SystemParam + 'static;

pub(super) type AttrParams<'w, 's, T> = TypeState<<T as Attribute>::Params<'w, 's>>;

pub(super) trait AttributeExtensions: Attribute {
  fn register_params(world: &mut World) {
    let state = SystemState::<<Self as Attribute>::Params<'_, '_>>::new(world);
    world.insert_resource(TypeState(state));
  }
}

impl<T> AttributeExtensions for T where T: Attribute {}
