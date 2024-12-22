use bevy::{
  asset::{io::Reader, AssetLoader, LoadContext, LoadedFolder},
  prelude::*,
  reflect::GetTypeRegistration,
  utils::hashbrown::{hash_map, HashMap},
};
use serde::Deserialize;
use std::marker::PhantomData;

use crate::scenes::MapEntities;

pub struct PrefabPlugin<T> {
  _pd: PhantomData<T>,
}

impl<T> Default for PrefabPlugin<T> {
  fn default() -> Self {
    Self { _pd: default() }
  }
}

impl<T> Plugin for PrefabPlugin<T>
where
  T: Prefab,
{
  fn build(&self, app: &mut App) {
    app
      .init_asset::<T::Descriptor>()
      .insert_resource(Manifest::<T>::default())
      .add_event::<LoadPrefabEvent<T>>()
      .register_asset_loader(Loader::<T>::default())
      .add_systems(
        Startup,
        |assets: ResMut<AssetServer>, mut commands: Commands| {
          let folders = assets.load_folder(T::DIR);
          commands.insert_resource(PrefabFolder::<T>::new(folders));
          info!(
            "Started folder load for {}",
            T::get_type_registration().type_info().type_path()
          );
        },
      )
      .add_systems(
        Update,
        (
          |mut event_reader: EventReader<AssetEvent<LoadedFolder>>,
           folders: Res<PrefabFolder<T>>,
           loaded_folders: Res<Assets<LoadedFolder>>,
           mut event_writer: EventWriter<LoadPrefabEvent<T>>| {
            for event in event_reader.read() {
              info!(
                "Loaded folder for {}",
                T::get_type_registration().type_info().type_path()
              );
              if event.is_loaded_with_dependencies(folders.handle()) {
                let folders = loaded_folders.get(folders.handle()).unwrap();
                for handle in folders.handles.iter() {
                  let id = handle.id().typed_unchecked::<T::Descriptor>();
                  event_writer.send(LoadPrefabEvent::<T>::new(id));
                }
              }
            }
          },
          |mut event_reader: EventReader<LoadPrefabEvent<T>>,
           descriptors: Res<Assets<T::Descriptor>>,
           mut manifest: ResMut<Manifest<T>>,
           mut map_entities: ResMut<MapEntities>,
           assets: Res<AssetServer>| {
            for event in event_reader.read() {
              info!(
                "Received prefab load event for {}",
                T::get_type_registration().type_info().type_path()
              );
              let Some(desc) = descriptors.get(event.id) else {
                warn!("asset id did not resolve to a descriptor asset");
                return;
              };
              let prefab = T::transform(desc, &assets);
              map_entities.register(prefab.name().to_string(), prefab.clone());
              manifest.register(prefab);
            }
          },
        ),
      );
  }
}

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
  const EXTENSIONS: &[&str];

  type Descriptor: Asset + for<'a> Deserialize<'a>;

  fn name(&self) -> &str {
    Self::get_type_registration()
      .type_info()
      .type_path_table()
      .short_path()
  }

  fn transform(desc: &Self::Descriptor, assets: &AssetServer) -> Self;
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

  async fn load(
    &self,
    reader: &mut dyn Reader,
    _settings: &Self::Settings,
    _load_context: &mut LoadContext<'_>,
  ) -> Result<Self::Asset, Self::Error> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    let ron: <T as Prefab>::Descriptor = ron::de::from_bytes(&bytes).unwrap();
    Ok(ron)
  }

  fn extensions(&self) -> &[&str] {
    T::EXTENSIONS
  }
}

#[derive(Resource)]
pub struct PrefabFolder<T>
where
  T: Prefab,
{
  handle: Handle<LoadedFolder>,
  _phantom_data: PhantomData<T>,
}

impl<T> PrefabFolder<T>
where
  T: Prefab,
{
  pub fn new(handle: Handle<LoadedFolder>) -> Self {
    Self {
      handle,
      _phantom_data: default(),
    }
  }

  pub fn handle(&self) -> &Handle<LoadedFolder> {
    &self.handle
  }
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
