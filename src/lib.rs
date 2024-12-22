pub mod assets;
mod cache;
mod input;
mod scenes;
mod ui;
mod util;
mod view;

use assets::{LoadPrefabEvent, Manifest, Prefab, PrefabFolder};
use bevy::asset::LoadedFolder;
use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use bevy::state::state::FreelyMutableState;
use bevy::transform::TransformSystem;
use bevy_egui::EguiSet;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use cache::Cache;
use scenes::{LoadEvent, MapEntities, MapEntityRegistrar, SaveEvent, SceneTypeRegistry};
use std::marker::PhantomData;
use ui::UiPlugin;

pub use bevy;
pub use input::Hotkeys;
pub use serde;
pub use util::*;
pub use view::EditorCamera;

pub struct Editor {
  app: App,
  cache: Cache,
  scene_type_registry: SceneTypeRegistry,
  entity_types: MapEntityRegistrar,
}

impl Editor {
  pub fn new<C, S>(mut app: App, config: EditorConfig<C, S>) -> Self
  where
    C: Component + Clone,
    S: FreelyMutableState + Copy,
  {
    app.add_plugins(EditorPlugin::new(config));

    let mut cache_path = std::env::current_exe()
      .unwrap()
      .parent()
      .unwrap()
      .to_path_buf();

    cache_path.push("cache.sqlite");

    let cache = Cache::connect(cache_path).unwrap();

    Self {
      app,
      cache,
      scene_type_registry: default(),
      entity_types: default(),
    }
  }

  pub fn register_prefab_default<T>(&mut self) -> &mut Self
  where
    T: Bundle + GetTypeRegistration + Clone + Default,
  {
    self.register_static_prefab_internal(None, T::default)
  }

  pub fn register_prefab<F, T, M>(&mut self, variant: impl Into<String>, sys: F) -> &mut Self
  where
    F: IntoSystem<(), T, M> + 'static,
    T: Bundle + GetTypeRegistration + Clone,
  {
    self.register_static_prefab_internal(Some(variant.into()), sys)
  }

  pub fn load_prefabs<T>(&mut self) -> &mut Self
  where
    T: Prefab,
  {
    self.register_type::<T>();

    self
      .app
      .init_asset::<T::Descriptor>()
      .insert_resource(Manifest::<T>::default())
      .add_event::<LoadPrefabEvent<T>>()
      .register_asset_loader(assets::Loader::<T>::default())
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
              if event.is_loaded_with_dependencies(folders.folder()) {
                let folders = loaded_folders.get(folders.folder()).unwrap();
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
    self
  }

  pub fn run(self) -> AppExit {
    let Self {
      mut app,
      cache,
      scene_type_registry,
      entity_types,
    } = self;

    app.insert_resource(scene_type_registry);
    app.insert_resource(entity_types);
    app.insert_resource(cache);

    app.run()
  }

  fn register_static_prefab_internal<F, T, M>(
    &mut self,
    variant: Option<String>,
    sys: F,
  ) -> &mut Self
  where
    F: IntoSystem<(), T, M> + 'static,
    T: Bundle + GetTypeRegistration + Clone,
  {
    self.register_type::<T>();

    let registration = T::get_type_registration();
    let path = registration.type_info().type_path();
    let id = variant
      .map(|v| format!("{path}#{v}"))
      .unwrap_or_else(|| path.into());

    let sys_id = self.app.register_system(sys);
    self.entity_types.register(id, sys_id);

    self
  }

  fn register_type<T>(&mut self)
  where
    T: GetTypeRegistration,
  {
    self.scene_type_registry.write().register::<T>();
    self.app.register_type::<T>();
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
enum EditorState {
  Editing,
  Inspecting,
}

#[derive(Resource, Clone)]
pub struct EditorConfig<C, S>
where
  C: Component + Clone,
  S: FreelyMutableState + Copy,
{
  editor_state: S,
  gameplay_state: S,
  _phantom_data: PhantomData<C>,
}

impl<C, S> EditorConfig<C, S>
where
  C: Component + Clone,
  S: FreelyMutableState + Copy,
{
  pub fn new(active_editor_state: S, gameplay_state: S) -> Self {
    Self {
      editor_state: active_editor_state,
      gameplay_state,
      _phantom_data: default(),
    }
  }
}

struct EditorPlugin<C, A>
where
  C: Component + Clone,
  A: FreelyMutableState + Copy,
{
  config: EditorConfig<C, A>,
  hotkeys: Hotkeys,
  cam_component: PhantomData<C>,
}

impl<C, A> Plugin for EditorPlugin<C, A>
where
  C: Component + Clone,
  A: FreelyMutableState + Copy,
{
  fn build(&self, app: &mut App) {
    app
      .add_plugins((DefaultInspectorConfigPlugin, UiPlugin))
      .add_event::<SaveEvent>()
      .add_event::<LoadEvent>()
      .insert_resource(self.hotkeys.clone())
      .insert_resource(self.config.clone())
      .insert_state(EditorState::Editing)
      .insert_state(self.config.editor_state)
      .add_systems(Startup, Self::initialize_types)
      .add_systems(OnEnter(self.config.editor_state), Self::on_enter)
      .add_systems(
        Update,
        (
          Self::special_input,
          (
            Self::handle_input,
            Self::check_for_saves,
            Self::check_for_loads,
            ((view::movement_system, view::orbit), view::cam_free_fly)
              .chain()
              .run_if(in_state(EditorState::Inspecting)),
          )
            .chain()
            .run_if(in_state(self.config.editor_state)),
          Self::show_ui_system,
        )
          .chain(),
      )
      .add_systems(PostUpdate, view::set_camera_viewport::<C>);
  }
}

impl<C, S> EditorPlugin<C, S>
where
  C: Component + Clone,
  S: FreelyMutableState + Copy,
{
  fn new(config: EditorConfig<C, S>) -> Self {
    Self {
      config,
      hotkeys: default(),
      cam_component: default(),
    }
  }

  fn initialize_types(world: &mut World) {
    let Some(registrar) = world.remove_resource::<MapEntityRegistrar>() else {
      return;
    };
    let entities = MapEntities::new_from(world, registrar);
    world.insert_resource(entities);
  }

  fn on_enter(mut q_windows: Query<&mut Window>) {
    for mut window in q_windows.iter_mut() {
      show_cursor(&mut window);
    }
  }

  fn show_ui_system(world: &mut World)
  where
    C: Component,
  {
    world.resource_scope(|world, mut ui_state: Mut<ui::State>| {
      ui_state.ui(world);
    });
  }

  fn check_for_saves(world: &mut World) {
    world.resource_scope(|world, save_events: Mut<Events<SaveEvent>>| {
      save_events.get_cursor().read(&save_events).for_each(|e| {
        e.handler(world);
      });
    });
  }

  fn check_for_loads(
    mut commands: Commands,
    mut load_events: EventReader<LoadEvent>,
    asset_server: Res<AssetServer>,
  ) {
    load_events.read().for_each(|e| {
      commands.spawn(DynamicSceneRoot(asset_server.load(e.file().clone())));
    });
  }

  fn special_input(
    config: Res<EditorConfig<C, S>>,
    hotkeys: Res<Hotkeys>,
    input: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<S>>,
    mut next_game_state: ResMut<NextState<S>>,
  ) {
    if input.just_pressed(hotkeys.play) {
      if *current_state.get() == config.gameplay_state {
        next_game_state.set(config.editor_state);
      } else {
        next_game_state.set(config.gameplay_state);
      }
    }
  }

  fn handle_input(
    hotkeys: Res<Hotkeys>,
    input: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window>,
    mut next_editor_state: ResMut<NextState<EditorState>>,
  ) {
    if input.just_pressed(hotkeys.move_cam) {
      let Ok(mut window) = windows.get_single_mut() else {
        return;
      };

      hide_cursor(&mut window);
      next_editor_state.set(EditorState::Inspecting);
    }

    if input.just_released(hotkeys.move_cam) {
      let Ok(mut window) = windows.get_single_mut() else {
        return;
      };

      show_cursor(&mut window);
      next_editor_state.set(EditorState::Editing);
    }
  }
}
