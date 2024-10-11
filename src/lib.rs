mod cache;
mod input;
mod ui;
mod util;
mod view;

use backends::egui::EguiPointer;
use backends::raycast::{RaycastBackendSettings, RaycastPickable};
use bevy::prelude::*;
use bevy::reflect::{GetTypeRegistration, TypeRegistryArc};
use bevy::state::state::FreelyMutableState;
use bevy::tasks::IoTaskPool;
use bevy::transform::TransformSystem;
use bevy::utils::HashMap;
use bevy::{render::camera::Viewport, window::PrimaryWindow};
use bevy_egui::{EguiContext, EguiSet};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_mod_picking::prelude::*;
use bevy_transform_gizmo::{GizmoPickSource, GizmoTransformable, TransformGizmoPlugin};
use cache::Cache;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Mutex;
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

  pub fn register_type_default<T>(&mut self) -> &mut Self
  where
    T: Bundle + GetTypeRegistration + Clone + Default,
  {
    self.register_type_internal(None, || T::default())
  }

  pub fn register_type<F, T, M>(&mut self, variant: impl Into<String>, sys: F) -> &mut Self
  where
    F: IntoSystem<(), T, M>,
    T: Bundle + GetTypeRegistration + Clone,
  {
    self.register_type_internal(Some(variant.into()), sys)
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

  fn register_type_internal<F, T, M>(&mut self, variant: Option<String>, sys: F) -> &mut Self
  where
    F: IntoSystem<(), T, M>,
    T: Bundle + GetTypeRegistration + Clone,
  {
    let registration = T::get_type_registration();
    let path = registration.type_info().type_path();
    let id = variant
      .map(|v| format!("{path}#{v}"))
      .unwrap_or_else(|| path.into());

    self.scene_type_registry.write().register::<T>();
    self.entity_types.register(id, sys);

    self.app.register_type::<T>();

    self
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
              Self::auto_register_camera,
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
          Self::set_camera_viewport,
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

  // make camera only render to view not obstructed by UI
  fn set_camera_viewport(
    ui_state: Res<ui::State<C>>,
    primary_window: Query<&mut Window, With<PrimaryWindow>>,
    egui_settings: Res<bevy_egui::EguiSettings>,
    mut cameras: Query<&mut Camera, With<C>>,
  ) {
    let mut cam = cameras.single_mut();

    let Ok(window) = primary_window.get_single() else {
      return;
    };

    let scale_factor = window.scale_factor() * egui_settings.scale_factor;

    let viewport_pos = ui_state.viewport_rect.left_top().to_vec2() * scale_factor;
    let viewport_size = ui_state.viewport_rect.size() * scale_factor;

    let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
    let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

    // The desired viewport rectangle at its offset in "physical pixel space"
    let rect = physical_position + physical_size;

    let window_size = window.physical_size();
    if rect.x <= window_size.x && rect.y <= window_size.y {
      cam.viewport = Some(Viewport {
        physical_position,
        physical_size,
        depth: 0.0..1.0,
      });
    }
  }

  fn auto_register_camera(
    mut commands: Commands,
    q_cam: Query<Entity, (Without<RaycastPickable>, With<C>)>,
  ) {
    for cam in &q_cam {
      debug!("added raycast to camera");
      commands
        .entity(cam)
        .insert((RaycastPickable, GizmoPickSource::default()));
    }
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

#[derive(Event)]
struct SaveEvent(PathBuf);

impl SaveEvent {
  pub fn file(&self) -> &PathBuf {
    &self.0
  }

  pub fn handler(&self, world: &mut World) {
    let mut scene_world = World::new();
    scene_world.insert_resource(world.resource::<AppTypeRegistry>().clone());
    let scene_type_registry = world.resource::<SceneTypeRegistry>().clone();
    let type_registry = scene_type_registry.read();

    let mut entities_to_copy = world.query_filtered::<Entity, With<SceneMarker>>();

    for entity in entities_to_copy.iter(world) {
      let new_entity = scene_world.spawn_empty().id();

      for architype in world.archetypes().iter() {
        if architype
          .entities()
          .into_iter()
          .map(|ae| ae.id())
          .any(|e| e == entity)
        {
          for comp_id in architype.components() {
            let components = world.components();
            let Some(comp_info) = components.get_info(comp_id) else {
              error!("failed to get component info for {}", comp_id.index());
              return;
            };

            let Some(comp_type_id) = comp_info.type_id() else {
              error!("failed to get comp type id of {}", comp_info.name());
              return;
            };

            let Some(comp_type) = type_registry.get(comp_type_id) else {
              // assume if the type is not present in the type registry it is not meant to be saved
              continue;
            };

            let Some(comp_ref) = comp_type.data::<ReflectComponent>() else {
              error!("failed to get reflect component of {}", comp_info.name());
              return;
            };

            comp_ref.copy(world, &mut scene_world, entity, new_entity, &type_registry);
          }
        }
      }
    }

    let scene = DynamicScene::from_world(&scene_world);

    let serialization = scene.serialize(&type_registry).unwrap();
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
struct LoadEvent(PathBuf);

impl LoadEvent {
  pub fn file(&self) -> &PathBuf {
    &self.0
  }
}

pub fn load_map() -> Entity {
  Entity::PLACEHOLDER
}

#[derive(Default, Resource)]
struct MapEntityRegistrar {
  mapping: Mutex<HashMap<String, Box<dyn FnOnce(String, &mut World, &mut MapEntities) + Send>>>,
}

impl MapEntityRegistrar {
  pub fn register<F, T, M>(&mut self, name: String, sys: F)
  where
    F: IntoSystem<(), T, M>,
    T: Bundle + GetTypeRegistration + Clone,
  {
    let mut sys = IntoSystem::into_system(sys);
    self.mapping.lock().unwrap().insert(
      name,
      Box::new(move |name, world, entities| {
        sys.initialize(world);
        let bundle: T = sys.run((), world);
        entities.register(name, bundle);
      }),
    );
  }
}

#[derive(Default, Resource)]
struct MapEntities {
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
struct SceneMarker;

#[derive(Default, Clone, Resource)]
struct SceneTypeRegistry {
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
