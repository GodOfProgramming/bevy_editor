use std::marker::PhantomData;

use bevy::{
  asset::{AssetLoader, AsyncReadExt, LoadedFolder},
  prelude::*,
  reflect::GetTypeRegistration,
  utils::hashbrown::{hash_map, HashMap},
};
use serde::Deserialize;

#[derive(Resource)]
pub struct PrefabManifest {
  table: HashMap<String, DynamicPrefab>,
}

struct DynamicPrefab {}

#[derive(Resource)]
pub struct Manifest<T>
where
  T: Prefab,
{
  table: HashMap<String, T>,
}

impl<T> Default for Manifest<T>
where
  T: Prefab,
{
  fn default() -> Self {
    Self { table: default() }
  }
}

impl<T> Manifest<T>
where
  T: Prefab,
{
  pub fn ids(&self) -> hash_map::Keys<String, T> {
    self.table.keys()
  }

  pub fn register(&mut self, prefab: T) {
    self.table.insert(prefab.name().to_string(), prefab);
  }

  pub fn lookup(&self, name: impl AsRef<str>) -> Option<T> {
    self.table.get(name.as_ref()).cloned()
  }
}

pub trait Prefab: GetTypeRegistration + Bundle + Clone {
  const DIR: &str;

  type Descriptor: Asset + for<'a> Deserialize<'a>;

  fn name(&self) -> &str;

  fn transform(desc: &Self::Descriptor, assets: &AssetServer) -> Self;
}

#[derive(Event)]

pub struct LoadPrefabEvent<T>
where
  T: Prefab,
{
  pub id: AssetId<T::Descriptor>,
}

impl<T> LoadPrefabEvent<T>
where
  T: Prefab,
{
  pub fn new(id: AssetId<T::Descriptor>) -> Self {
    Self { id }
  }
}

#[derive(Resource)]
pub struct PrefabFolder<T>
where
  T: Prefab,
{
  folder: Handle<LoadedFolder>,
  _phantom_data: PhantomData<T>,
}

impl<T> PrefabFolder<T>
where
  T: Prefab,
{
  pub fn new(folder: Handle<LoadedFolder>) -> Self {
    Self {
      folder,
      _phantom_data: default(),
    }
  }

  pub fn folder(&self) -> &Handle<LoadedFolder> {
    &self.folder
  }
}

pub struct Loader<T>
where
  T: Prefab,
{
  _phantom_data: PhantomData<T>,
}

impl<T> Default for Loader<T>
where
  T: Prefab,
{
  fn default() -> Self {
    Self {
      _phantom_data: default(),
    }
  }
}

impl<T> AssetLoader for Loader<T>
where
  T: Prefab,
{
  type Asset = <T as Prefab>::Descriptor;

  type Settings = ();

  type Error = std::io::Error;

  async fn load<'a>(
    &'a self,
    reader: &'a mut bevy::asset::io::Reader<'_>,
    _settings: &'a Self::Settings,
    _load_context: &'a mut bevy::asset::LoadContext<'_>,
  ) -> Result<Self::Asset, Self::Error> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    let ron: <T as Prefab>::Descriptor = ron::de::from_bytes(&bytes).unwrap();
    Ok(ron)
  }

  fn extensions(&self) -> &[&str] {
    &["weapon.ron"]
  }
}
