use std::hash::{DefaultHasher, Hash, Hasher};

use bevy::{prelude::*, window::CursorGrabMode};

pub fn show_cursor(window: &mut Window) {
  window.cursor.visible = true;
  window.cursor.grab_mode = CursorGrabMode::None;
}

pub fn hide_cursor(window: &mut Window) {
  window.cursor.visible = false;
  window.cursor.grab_mode = CursorGrabMode::Locked;
}

pub trait HashValue {
  fn hash_value(&self) -> u64;
}

impl<T> HashValue for T
where
  T: Hash,
{
  fn hash_value(&self) -> u64 {
    let mut hasher = DefaultHasher::new();
    self.hash(&mut hasher);
    hasher.finish()
  }
}
