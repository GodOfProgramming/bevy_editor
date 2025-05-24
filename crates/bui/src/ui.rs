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

  fn construct(&self, params: Self::Params<'_, '_>) -> impl Bundle;
}

impl<T> Attribute for T
where
  T: Component + Clone,
{
  type Params<'w, 's> = NoParams;

  fn construct(&self, _params: Self::Params<'_, '_>) -> impl Bundle {
    self.clone()
  }
}

pub trait SerializableAttribute: 'static {
  type Out<'de>: Serialize + Deserialize<'de> + Reflect;

  fn transform(&self) -> Self::Out<'_>;
}

pub(super) type AttrParams<'w, 's, T> = AttrState<<T as Attribute>::Params<'w, 's>>;

#[derive(Resource, Deref, DerefMut)]
pub(super) struct AttrState<P>(SystemState<P>)
where
  P: SystemParam + 'static;

pub(super) trait AttributeExtensions: Attribute {
  fn register_params(world: &mut World) {
    let state = SystemState::<<Self as Attribute>::Params<'_, '_>>::new(world);
    world.insert_resource(AttrState(state));
  }
}

impl<T> AttributeExtensions for T where T: Attribute {}
