use bevy::{
  asset::{io::Reader, AssetLoader, LoadContext, LoadedFolder},
  ecs::system::{SystemParam, SystemState},
  prelude::*,
  reflect::GetTypeRegistration,
  utils::hashbrown::{hash_map, HashMap},
};
use serde::Deserialize;
use std::{
  marker::PhantomData,
  ops::{Deref, DerefMut},
};

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

pub trait StaticPrefab: GetTypeRegistration + Sized {
  type Params<'pw, 'ps>: for<'w, 's> SystemParam<Item<'w, 's> = Self::Params<'w, 's>>;

  fn spawn(params: Self::Params<'_, '_>) -> impl Bundle;
}

fn short_name_of<T>() -> &'static str
where
  T: GetTypeRegistration,
{
  T::get_type_registration()
    .type_info()
    .type_path_table()
    .short_path()
}

type RegistrationFn = dyn Fn(&mut World) -> Box<SpawnFn> + Send + Sync;

#[derive(Resource, Default)]
pub struct PrefabRegistrar {
  registrations: HashMap<String, Box<RegistrationFn>>,
}

impl PrefabRegistrar {
  pub fn register<T>(&mut self)
  where
    T: StaticPrefab,
  {
    self.register_internal(short_name_of::<T>(), |world| {
      let mut state = SystemState::<T::Params<'_, '_>>::new(world);

      // the below is what's stored in Prefabs
      move |world| {
        let params = state.get_mut(world);
        let bundle = T::spawn(params);
        world.spawn(bundle);
      }
    });
  }

  /// Calls R which produces a closure that is later invoked to return the spawn function
  fn register_internal<'w, 's, R, S>(&mut self, name: impl Into<String>, f: R)
  where
    S: FnMut(&mut World) + Send + Sync + 'static,
    R: Fn(&mut World) -> S + Send + Sync + 'static,
  {
    self
      .registrations
      .insert(name.into(), Box::new(move |world| Box::new((f)(world))));
  }
}

type SpawnFn = dyn FnMut(&mut World) + Send + Sync;
type PrefabSpawnMap = HashMap<String, Box<SpawnFn>>;

#[derive(Resource)]
pub struct Prefabs(PrefabSpawnMap);

impl Prefabs {
  pub fn new(world: &mut World, registrar: PrefabRegistrar) -> Self {
    let prefabs = registrar
      .registrations
      .into_iter()
      .map(|(k, v)| (k, (v)(world)))
      .collect();

    Self(prefabs)
  }

  pub fn spawn(&mut self, id: impl AsRef<str>, world: &mut World) {
    if let Some(spawn_fn) = self.get_mut(id.as_ref()) {
      (spawn_fn)(world);
    }
  }
}

impl Deref for Prefabs {
  type Target = PrefabSpawnMap;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for Prefabs {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

pub trait Prefab: GetTypeRegistration + Bundle + Clone {
  const DIR: &str;
  const EXTENSIONS: &[&str];

  type Descriptor: Asset + for<'a> Deserialize<'a>;

  fn name(&self) -> &str {
    short_name_of::<Self>()
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
