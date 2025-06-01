use crate::cache::{Cache, Saveable};
use bevy::{
  log::{
    BoxedLayer, Level,
    tracing_subscriber::{self, Layer, reload},
  },
  platform::collections::HashMap,
  prelude::*,
  reflect::GetTypeRegistration,
  window::CursorGrabMode,
  winit::cursor::CursorIcon,
};
use derive_more::derive::{Deref, DerefMut};
use profiling::tracing::level_filters::LevelFilter;
use serde::{Deserialize, Serialize, Serializer};
use std::{collections::BTreeMap, hint::unreachable_unchecked, ops::Deref};

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

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct LogInfo {
  level: LogLevel,
}

impl Saveable for LogInfo {
  const KEY: &str = "logging";
}

impl LogInfo {
  pub fn on_app_exit(logging_settings: Res<LoggingSettings>, mut cache: ResMut<Cache>) {
    cache.store(&LogInfo {
      level: logging_settings.level,
    });
  }
}

#[derive(Resource)]
pub struct LoggingSettings {
  level: LogLevel,
  filter_handle: reload::Handle<LevelFilter, tracing_subscriber::Registry>,
}

impl LoggingSettings {
  pub fn level(&self) -> LogLevel {
    self.level
  }

  pub fn set_level(&mut self, level: LogLevel) {
    self.level = level;
    self
      .filter_handle
      .modify(|filter| *filter = level.into())
      .inspect_err(|err| {
        eprintln!("Failed to set log level filter: {err}");
      })
      .ok();
  }

  pub fn restore(mut logging: ResMut<Self>, cache: Res<Cache>) {
    let Some(log_info) = cache.get::<LogInfo>() else {
      error!("Failed to get log info, using default logging settings");
      return;
    };

    logging.set_level(log_info.level);
  }
}

pub fn dynamic_log_layer(app: &mut App) -> Option<BoxedLayer> {
  let level = LogLevel::Info;
  let (filter, handle) = reload::Layer::new(level.into());
  app.insert_resource(LoggingSettings {
    level,
    filter_handle: handle,
  });

  Some(filter.boxed())
}

#[derive(Reflect, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
  Trace,
  Debug,
  #[default]
  Info,
  Warn,
  Error,
}

impl From<LogLevel> for Level {
  fn from(value: LogLevel) -> Self {
    match value {
      LogLevel::Trace => Level::TRACE,
      LogLevel::Debug => Level::DEBUG,
      LogLevel::Info => Level::INFO,
      LogLevel::Warn => Level::WARN,
      LogLevel::Error => Level::ERROR,
    }
  }
}

impl From<LogLevel> for LevelFilter {
  fn from(value: LogLevel) -> Self {
    match value {
      LogLevel::Trace => LevelFilter::TRACE,
      LogLevel::Debug => LevelFilter::DEBUG,
      LogLevel::Info => LevelFilter::INFO,
      LogLevel::Warn => LevelFilter::WARN,
      LogLevel::Error => LevelFilter::ERROR,
    }
  }
}

#[derive(Clone)]
pub enum VfsNode<T> {
  Directory(VfsDir<T>),
  Item(T),
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum VfsIdent {
  Dir(String),
  Item(String),
}

impl Deref for VfsIdent {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    match self {
      VfsIdent::Dir(s) => s,
      VfsIdent::Item(s) => s,
    }
  }
}

#[derive(Deref, DerefMut)]
pub struct VfsDir<T> {
  name: String,
  parent: Option<Vec<String>>,

  #[deref]
  #[deref_mut]
  entries: BTreeMap<VfsIdent, VfsNode<T>>,
}

impl<T> Default for VfsDir<T> {
  fn default() -> Self {
    Self {
      name: String::new(),
      parent: None,
      entries: default(),
    }
  }
}

impl<T> Clone for VfsDir<T>
where
  T: Clone,
{
  fn clone(&self) -> Self {
    Self {
      name: self.name.clone(),
      parent: self.parent.clone(),
      entries: self.entries.clone(),
    }
  }
}

impl<T> VfsDir<T> {
  pub fn new_root() -> Self {
    Self::default()
  }

  pub fn new_child(name: impl Into<String>, path: impl Into<Vec<String>>) -> Self {
    Self {
      name: name.into(),
      parent: Some(path.into()),
      entries: default(),
    }
  }

  pub fn iter(&self) -> impl Iterator<Item = (&VfsIdent, &VfsNode<T>)> {
    self.entries.iter()
  }

  pub fn add<I, S>(&mut self, path: I, name: impl Into<String>, item: T)
  where
    I: Iterator<Item = S>,
    S: AsRef<str>,
  {
    let dir = self.get_dir(0, path);
    dir.insert(VfsIdent::Item(name.into()), VfsNode::Item(item));
  }

  pub fn add_by_full_path(&mut self, full_path: impl AsRef<str>, separator: &str, item: T) {
    let mut path = full_path.as_ref().split(separator);

    let count = path.clone().count();

    let (path, Some(name)) = (if count == 0 {
      (None, path.next())
    } else {
      (Some(path.clone().take(count - 1)), path.nth(count - 1))
    }) else {
      return;
    };

    let dir = if let Some(path) = path {
      self.get_dir(count, path)
    } else {
      self
    };

    dir
      .entries
      .insert(VfsIdent::Item(String::from(name)), VfsNode::Item(item));
  }

  fn get_dir<S>(&mut self, count: usize, path: impl Iterator<Item = S>) -> &mut Self
  where
    S: AsRef<str>,
  {
    let mut dir = self;

    let mut path_builder = Vec::with_capacity(count.max(1) - 1);

    for p in path {
      let part = String::from(p.as_ref());
      path_builder.push(part.clone());

      let entry = dir
        .entries
        .entry(VfsIdent::Dir(part))
        .or_insert_with(|| VfsNode::Directory(default()));

      if let VfsNode::Directory(d) = entry {
        dir = d
      } else {
        unsafe {
          unreachable_unchecked();
        }
      }
    }

    dir
  }
}

impl<T> VfsDir<T>
where
  T: VirtualItem,
{
  pub fn insert(&mut self, item: T) {
    self.add_by_full_path(String::from(item.path()), T::SEPARATOR, item);
  }
}

pub trait VirtualItem {
  const SEPARATOR: &str;

  fn path(&self) -> &str;
}

impl<T> VirtualItem for T
where
  T: TypePath,
{
  const SEPARATOR: &str = "::";

  fn path(&self) -> &str {
    T::type_path()
  }
}
