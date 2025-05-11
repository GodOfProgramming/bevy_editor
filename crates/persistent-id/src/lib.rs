use bevy::prelude::*;
use derive_more::derive::From;
use uuid::Uuid;

pub trait Identifiable<T = Uuid>
where
  T: Default,
{
  const ID: T;
  const TYPE_NAME: &'static str;
}

#[derive(Default, Deref, DerefMut, Component, Clone, Copy, Hash, PartialEq, Eq, Reflect, From)]
pub struct PersistentId<T = Uuid>(#[reflect(ignore)] pub T)
where
  T: Default;
