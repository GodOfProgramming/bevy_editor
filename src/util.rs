use std::{
  collections::BTreeMap,
  hash::{DefaultHasher, Hash, Hasher},
};

use bevy::{
  prelude::*,
  reflect::GetTypeRegistration,
  state::state::FreelyMutableState,
  utils::HashMap,
  window::{CursorGrabMode, PrimaryWindow},
  winit::cursor::CursorIcon,
};
use serde::{Serialize, Serializer};

#[macro_export]
macro_rules! here {
  () => {{
    use std::io::Write;
    println!("{}({})", file!(), line!());
    std::io::stdout().flush().ok();
  }};
}

pub fn short_name_of<T>() -> &'static str
where
  T: GetTypeRegistration,
{
  T::get_type_registration()
    .type_info()
    .type_path_table()
    .short_path()
}

pub fn show_cursor(window: &mut Window) {
  window.cursor_options.visible = true;
  window.cursor_options.grab_mode = CursorGrabMode::None;
}

pub fn hide_cursor(window: &mut Window) {
  window.cursor_options.visible = false;
  window.cursor_options.grab_mode = CursorGrabMode::Locked;
}

pub fn set_cursor_icon(commands: &mut Commands, entity: Entity, cursor: impl Into<CursorIcon>) {
  commands.entity(entity).insert(cursor.into());
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

pub trait WorldExtensions {
  fn primary_window_mut(&mut self) -> Mut<Window>;

  fn get_state<T>(&mut self) -> T
  where
    T: FreelyMutableState + Copy;

  fn set_state<T>(&mut self, state: T)
  where
    T: FreelyMutableState + Copy;
}

impl WorldExtensions for World {
  fn primary_window_mut(&mut self) -> Mut<Window> {
    let mut q_window = self.query_filtered::<&mut Window, With<PrimaryWindow>>();
    q_window.single_mut(self)
  }

  fn get_state<T>(&mut self) -> T
  where
    T: FreelyMutableState + Copy,
  {
    **self.resource::<State<T>>()
  }

  fn set_state<T>(&mut self, state: T)
  where
    T: FreelyMutableState + Copy,
  {
    self.resource_mut::<NextState<T>>().set(state);
  }
}

pub fn sorted_keys<S, K: Ord + Serialize, V: Serialize>(
  value: &HashMap<K, V>,
  serializer: S,
) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  let ordered: BTreeMap<_, _> = value.iter().collect();
  ordered.serialize(serializer)
}
