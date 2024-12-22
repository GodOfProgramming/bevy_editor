use std::hash::{DefaultHasher, Hash, Hasher};

use bevy::{prelude::*, window::CursorGrabMode};

#[macro_export]
macro_rules! here {
  () => {{
    use std::io::Write;
    println!("{}({})", file!(), line!());
    std::io::stdout().flush().ok();
  }};
}

pub fn show_cursor(window: &mut Window) {
  window.cursor_options.visible = true;
  window.cursor_options.grab_mode = CursorGrabMode::None;
}

pub fn hide_cursor(window: &mut Window) {
  window.cursor_options.visible = false;
  window.cursor_options.grab_mode = CursorGrabMode::Locked;
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

pub struct ValueCache<T> {
  value: T,
  dirty: bool,
}

impl<T> ValueCache<T> {
  pub fn new(value: T) -> Self {
    Self {
      value,
      dirty: false,
    }
  }

  pub fn is_dirty(&self) -> bool {
    self.dirty
  }

  pub fn dirty(&mut self) {
    self.dirty = true;
  }

  pub fn emplace(&mut self, value: T) {
    self.value = value;
    self.dirty = false;
  }

  pub fn value(&self) -> &T {
    &self.value
  }

  pub fn value_mut(&mut self) -> &mut T {
    &mut self.value
  }
}

impl<T> Default for ValueCache<T>
where
  T: Default,
{
  fn default() -> Self {
    Self::new(T::default())
  }
}
