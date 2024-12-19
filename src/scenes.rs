use crate::ValueCache;
use bevy::{
  asset::ReflectHandle,
  ecs::system::SystemId,
  prelude::*,
  reflect::{GetTypeRegistration, TypeRegistryArc},
  tasks::IoTaskPool,
  utils::HashMap,
};
use std::{
  cell::RefCell,
  ops::{Deref, DerefMut},
  path::PathBuf,
  sync::Mutex,
};

#[derive(Event)]
pub struct SaveEvent(PathBuf);

impl SaveEvent {
  pub fn new(path: PathBuf) -> Self {
    Self(path)
  }

  pub fn file(&self) -> &PathBuf {
    &self.0
  }

  pub fn handler(&self, world: &mut World) {
    let world_type_registry = world.resource::<AppTypeRegistry>().clone();

    let mut scene_world = World::new();
    scene_world.insert_resource(world_type_registry.clone());

    let world_type_registry = world_type_registry.read();

    let scene_type_registry = world.resource::<SceneTypeRegistry>().clone();
    let scene_type_registry = scene_type_registry.read();

    let scene_marker_id = world.component_id::<SceneMarker>().unwrap();
    let components = world.components();

    for archetype in world
      .archetypes()
      .iter()
      .filter(|a| a.components().any(|c| c == scene_marker_id))
    {
      for entity in archetype.entities() {
        let new_entity_id = scene_world.spawn_empty().id();

        for comp_id in archetype.components() {
          let Some(comp_info) = components.get_info(comp_id) else {
            error!("failed to get component info for {}", comp_id.index());
            return;
          };

          let Some(comp_type_id) = comp_info.type_id() else {
            error!("failed to get comp type id of {}", comp_info.name());
            return;
          };

          if !scene_type_registry.contains(comp_type_id) {
            // assume if the type is not present in the type registry it is not meant to be saved
            continue;
          }

          let comp_type_reg = world_type_registry.get(comp_type_id).unwrap();

          info!("serializing {}", comp_type_reg.type_info().type_path());

          let Some(ref_comp) = comp_type_reg.data::<ReflectComponent>() else {
            error!("failed to get reflect component of {}", comp_info.name());
            return;
          };

          if let Some(ref_handle) = comp_type_reg.data::<ReflectHandle>() {
            let entity_ref = world.get_entity(entity.id()).unwrap();
            let dyn_ref = ref_comp.reflect(&entity_ref).unwrap();
            let asset_handle = ref_handle
              .downcast_handle_untyped(dyn_ref.as_any())
              .unwrap();
            if let Some(path) = asset_handle.path() {
              info!("asset path => {:?}", path);
            } else {
              continue;
            }
          } else {
            ref_comp.copy(
              world,
              &mut scene_world,
              entity.id(),
              new_entity_id,
              &world_type_registry,
            );
          }
        }
      }
    }

    let scene = DynamicScene::from_world(&scene_world);

    let serialization = scene.serialize(&scene_type_registry).unwrap();
    let filename = self.file().clone();
    IoTaskPool::get()
      .spawn(async move {
        let printable_filename = filename.display().to_string();

        info!("saving scene to {}...", printable_filename);
        if let Some(parent) = filename.parent() {
          if let Err(err) = async_std::fs::create_dir_all(parent).await {
            error!("failed to create directory '{}': {err}", parent.display());
          }
        }

        if let Err(err) = async_std::fs::write(filename, serialization).await {
          error!("failed to save scene to '{}': {err}", printable_filename);
          return;
        }

        info!("finished saving");
      })
      .detach();
  }
}

#[derive(Event)]
pub struct LoadEvent(PathBuf);

impl LoadEvent {
  pub fn new(path: PathBuf) -> Self {
    Self(path)
  }

  pub fn file(&self) -> &PathBuf {
    &self.0
  }
}

pub fn load_map() -> Entity {
  Entity::PLACEHOLDER
}

#[derive(Default, Resource)]
pub struct MapEntityRegistrar {
  mapping: Mutex<HashMap<String, Box<dyn FnOnce(String, &mut World, &mut MapEntities) + Send>>>,
}

impl MapEntityRegistrar {
  pub fn register<T>(&mut self, name: String, sys: SystemId<(), T>)
  where
    T: Bundle + GetTypeRegistration + Clone,
  {
    self.mapping.lock().unwrap().insert(
      name,
      Box::new(move |name, world, entities| {
        let bundle: T = world.run_system(sys).unwrap();
        entities.register(name, bundle);
      }),
    );
  }
}

#[derive(Default, Resource)]
pub struct MapEntities {
  mapping: Mutex<HashMap<String, Box<dyn Fn(&mut World) + Send>>>,
  key_cache: Mutex<RefCell<ValueCache<Vec<String>>>>,
}

impl MapEntities {
  pub fn new_from(world: &mut World, registrar: MapEntityRegistrar) -> Self {
    let mut entities = Self::default();
    let mapping = registrar.mapping.into_inner().unwrap();

    for (k, v) in mapping.into_iter() {
      (v)(k, world, &mut entities);
    }

    entities
  }

  pub fn register<T>(&mut self, id: impl Into<String>, value: T)
  where
    T: Bundle + Clone,
  {
    self.key_cache.lock().unwrap().borrow_mut().dirty();
    self.mapping.lock().unwrap().insert(
      id.into(),
      Box::new(move |world| {
        world.spawn((SceneMarker, value.clone()));
      }),
    );
  }

  pub fn ids(&self) -> Vec<String> {
    let key_cache = self.key_cache.lock().unwrap();
    let mut key_cache = key_cache.borrow_mut();

    if key_cache.is_dirty() {
      let values = self.mapping.lock().unwrap().keys().cloned().collect();
      key_cache.emplace(values);
    }

    key_cache.value().clone()
  }

  pub fn spawn(&self, id: impl AsRef<str>, world: &mut World) {
    let Ok(mapping) = self.mapping.lock() else {
      return;
    };

    let Some(spawn_fn) = mapping.get(id.as_ref()) else {
      return;
    };

    spawn_fn(world);
  }
}

#[derive(Component)]
pub struct SceneMarker;

#[derive(Default, Clone, Resource)]
pub struct SceneTypeRegistry {
  type_registry: TypeRegistryArc,
}

impl Deref for SceneTypeRegistry {
  type Target = TypeRegistryArc;
  fn deref(&self) -> &Self::Target {
    &self.type_registry
  }
}

impl DerefMut for SceneTypeRegistry {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.type_registry
  }
}
