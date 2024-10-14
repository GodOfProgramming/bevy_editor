pub mod assets;
mod cache;
mod input;
mod scenes;
mod ui;
mod util;
mod view;

use assets::{LoadPrefabEvent, Manifest, Prefab, PrefabFolder};
use backends::egui::EguiPointer;
use backends::raycast::{RaycastBackendSettings, RaycastPickable};
use bevy::asset::LoadedFolder;
use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use bevy::state::state::FreelyMutableState;
use bevy::transform::TransformSystem;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContext, EguiSet};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_mod_picking::prelude::*;
use bevy_transform_gizmo::{GizmoTransformable, TransformGizmoPlugin};
use cache::Cache;
use scenes::{LoadEvent, MapEntities, MapEntityRegistrar, SaveEvent, SceneTypeRegistry};
use std::marker::PhantomData;
use ui::UiPlugin;

pub use bevy;
pub use input::Hotkeys;
pub use serde;
pub use util::*;
pub use view::EditorCameraBundle;

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

  pub fn register_static_prefab_default<T>(&mut self) -> &mut Self
  where
    T: Bundle + GetTypeRegistration + Clone + Default,
  {
    self.register_static_prefab_internal(None, T::default)
  }

  pub fn register_static_prefab<F, T, M>(&mut self, variant: impl Into<String>, sys: F) -> &mut Self
  where
    F: IntoSystem<(), T, M> + 'static,
    T: Bundle + GetTypeRegistration + Clone,
  {
    self.register_static_prefab_internal(Some(variant.into()), sys)
  }

  pub fn register_prefab<T>(&mut self) -> &mut Self
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
      .add_plugins((
        bevy_egui::EguiPlugin,
        DefaultInspectorConfigPlugin,
        bevy_mod_picking::DefaultPickingPlugins,
        TransformGizmoPlugin::new(Quat::default()),
        UiPlugin::<C>::new(),
      ))
      .add_event::<SaveEvent>()
      .add_event::<LoadEvent>()
      .insert_resource(self.hotkeys.clone())
      .insert_resource(self.config.clone())
      .insert_state(EditorState::Editing)
      .insert_state(self.config.editor_state)
      .add_systems(Startup, (Self::startup, Self::initialize_types))
      .add_systems(OnEnter(self.config.editor_state), Self::on_enter)
      .add_systems(OnExit(self.config.editor_state), Self::on_exit)
      .add_systems(
        Update,
        (
          Self::handle_input,
          Self::check_for_saves,
          Self::check_for_loads,
          (
            (
              view::auto_register_camera::<C>,
              Self::auto_register_targets,
              Self::handle_pick_events,
            ),
            ((view::movement_system, view::orbit), view::cam_free_fly)
              .chain()
              .run_if(in_state(EditorState::Inspecting)),
          ),
        )
          .chain()
          .run_if(Self::in_editor_state),
      )
      .add_systems(
        PostUpdate,
        (
          Self::show_ui_system
            .before(EguiSet::ProcessOutput)
            .before(TransformSystem::TransformPropagate),
          view::set_camera_viewport::<C>,
        )
          .chain(),
      );
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

  fn startup(mut raycast_settings: ResMut<RaycastBackendSettings>) {
    raycast_settings.require_markers = true;
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

  fn on_exit(
    mut commands: Commands,
    q_targets: Query<Entity, (With<RaycastPickable>, Without<Camera>)>,
  ) {
    for target in q_targets.iter() {
      commands
        .entity(target)
        .remove::<RaycastPickable>()
        .remove::<GizmoTransformable>()
        .remove::<PickableBundle>();
    }
  }

  fn show_ui_system(world: &mut World)
  where
    C: Component,
  {
    let Ok(egui_context) = world
      .query_filtered::<&mut EguiContext, With<PrimaryWindow>>()
      .get_single(world)
    else {
      return;
    };
    let mut egui_context = egui_context.clone();

    world.resource_scope::<ui::State<C>, _>(|world, mut ui_state| {
      ui_state.ui(world, egui_context.get_mut())
    });
  }

  fn auto_register_targets(
    mut commands: Commands,
    query: Query<Entity, (Without<RaycastPickable>, With<Handle<Mesh>>)>,
  ) {
    for entity in &query {
      debug!("added raycast to target {}", entity);
      commands.entity(entity).insert((
        RaycastPickable,
        PickableBundle::default(),
        GizmoTransformable,
      ));
    }
  }

  fn check_for_saves(world: &mut World) {
    world.resource_scope(|world, save_events: Mut<Events<SaveEvent>>| {
      save_events.get_reader().read(&save_events).for_each(|e| {
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
      commands.spawn(DynamicSceneBundle {
        scene: asset_server.load(e.file().clone()),
        ..default()
      });
    });
  }

  fn handle_input(
    config: Res<EditorConfig<C, S>>,
    hotkeys: Res<Hotkeys>,
    input: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window>,
    mut next_editor_state: ResMut<NextState<EditorState>>,
    mut next_game_state: ResMut<NextState<S>>,
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

    if input.just_pressed(hotkeys.play_current_level) {
      next_game_state.set(config.gameplay_state);
    }
  }

  fn handle_pick_events(
    mut ui_state: ResMut<ui::State<C>>,
    mut click_events: EventReader<Pointer<Click>>,
    mut q_egui: Query<&mut EguiContext>,
    q_egui_entity: Query<&EguiPointer>,
    q_raycast_pickables: Query<&RaycastPickable>,
  ) {
    let mut egui = q_egui.single_mut();
    let egui_context = egui.get_mut();

    for click in click_events.read() {
      let target = click.target();

      if q_egui_entity.get(target).is_ok() {
        continue;
      };

      let modifiers = egui_context.input(|i| i.modifiers);

      if q_raycast_pickables.get(target).is_ok() {
        ui_state.add_selected(target, modifiers.ctrl);
      }
    }
  }

  fn in_editor_state(config: Res<EditorConfig<C, S>>, state: Res<State<S>>) -> bool {
    config.editor_state == **state
  }
}
