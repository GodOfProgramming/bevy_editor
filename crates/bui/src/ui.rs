pub mod attrs;
pub mod elements;
pub mod events;
mod generated;

use bevy::{prelude::*, reflect::Reflectable};

pub trait Element: Reflectable {}

#[reflect_trait]
pub trait Attribute {
  fn insert_into(&self, entity: EntityWorldMut);
}

impl<T> Attribute for T
where
  T: Component + Clone,
{
  fn insert_into(&self, mut entity: EntityWorldMut) {
    entity.insert(self.clone());
  }
}
