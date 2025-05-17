pub mod attrs;
pub mod elements;

use bevy::{prelude::*, reflect::Reflectable};

pub trait Element: Reflectable {}

pub trait Attribute: Reflectable {
  fn insert_into(&self, entity: EntityWorldMut);
}

impl<T> Attribute for T
where
  T: Reflectable + Component + Clone,
{
  fn insert_into(&self, mut entity: EntityWorldMut) {
    entity.insert(self.clone());
  }
}
