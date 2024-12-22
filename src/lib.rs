pub mod assets;
mod input;
mod scenes;
mod ui;
mod util;
mod view;

use assets::{LoadPrefabEvent, Manifest, Prefab, PrefabFolder};
use bevy::color::palettes::tailwind::{PINK_100, RED_500};
use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use bevy::state::state::FreelyMutableState;
use bevy::{asset::LoadedFolder, picking::pointer::PointerInteraction};
use bevy_egui::EguiContext;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use scenes::{LoadEvent, MapEntities, MapEntityRegistrar, SaveEvent, SceneTypeRegistry};
use ui::UiPlugin;
use view::{CameraSettings, CameraState};

pub use bevy;
pub use input::Hotkeys;
pub use serde;
pub use util::*;
pub use view::EditorCamera;

pub struct Editor {
  app: App,
  scene_type_registry: SceneTypeRegistry,
  entity_types: MapEntityRegistrar,
}

impl Editor {
  pub fn new<S>(mut app: App, active_state: S, gameplay_state: S) -> Self
  where
    S: FreelyMutableState + Copy,
  {
    let config = EditorConfig::new(active_state, gameplay_state);

    app.add_plugins(EditorPlugin::new(config));

    Self {
      app,
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
      scene_type_registry,
      entity_types,
    } = self;

    app.insert_resource(scene_type_registry);
    app.insert_resource(entity_types);

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
struct EditorConfig<S>
where
  S: FreelyMutableState + Copy,
{
  editor_state: S,
  gameplay_state: S,
}

impl<S> EditorConfig<S>
where
  S: FreelyMutableState + Copy,
{
  pub fn new(active_editor_state: S, gameplay_state: S) -> Self {
    Self {
      editor_state: active_editor_state,
      gameplay_state,
    }
  }
}

struct EditorPlugin<S>
where
  S: FreelyMutableState + Copy,
{
  config: EditorConfig<S>,
  hotkeys: Hotkeys,
}

impl<S> Plugin for EditorPlugin<S>
where
  S: FreelyMutableState + Copy,
{
  fn build(&self, app: &mut App) {
    app
      .add_plugins((MeshPickingPlugin, DefaultInspectorConfigPlugin, UiPlugin))
      .register_type::<CameraState>()
      .register_type::<CameraSettings>()
      .add_event::<SaveEvent>()
      .add_event::<LoadEvent>()
      .insert_resource(self.hotkeys.clone())
      .insert_resource(self.config.clone())
      .insert_state(EditorState::Editing)
      .insert_state(self.config.gameplay_state)
      .add_systems(
        Startup,
        (Self::startup, Self::spawn_camera, Self::initialize_types),
      )
      .add_systems(OnEnter(self.config.editor_state), Self::on_enter)
      .add_systems(OnExit(self.config.editor_state), Self::on_exit)
      .add_systems(
        Update,
        (
          input::special_input::<S>,
          (
            input::handle_input,
            scenes::check_for_saves,
            scenes::check_for_loads,
            (
              view::auto_register_camera,
              Self::auto_register_targets,
              Self::handle_pick_events,
              Self::draw_mesh_intersections,
            ),
            ((view::movement_system, view::look), view::cam_free_fly)
              .chain()
              .run_if(in_state(EditorState::Inspecting)),
          )
            .chain()
            .run_if(in_state(self.config.editor_state)),
          ui::render,
        )
          .chain(),
      )
      .add_systems(PostUpdate, view::set_camera_viewport);
  }
}

impl<S> EditorPlugin<S>
where
  S: FreelyMutableState + Copy,
{
  fn new(config: EditorConfig<S>) -> Self {
    Self {
      config,
      hotkeys: default(),
    }
  }

  fn startup(mut picking_settings: ResMut<MeshPickingSettings>) {
    picking_settings.require_markers = true;
  }

  fn spawn_camera(mut commands: Commands) {
    commands.spawn(EditorCamera);
  }

  fn initialize_types(world: &mut World) {
    let Some(registrar) = world.remove_resource::<MapEntityRegistrar>() else {
      return;
    };
    let entities = MapEntities::new_from(world, registrar);
    world.insert_resource(entities);
  }

  fn on_exit(
    mut commands: Commands,
    q_targets: Query<Entity, (With<RayCastPickable>, Without<Camera>)>,
  ) {
    for target in q_targets.iter() {
      commands.entity(target).remove::<RayCastPickable>();
    }
  }

  fn on_enter(mut q_windows: Query<&mut Window>) {
    for mut window in q_windows.iter_mut() {
      show_cursor(&mut window);
    }
  }

  fn auto_register_targets(
    mut commands: Commands,
    query: Query<Entity, (Without<RayCastPickable>, With<Mesh3d>)>,
  ) {
    for entity in &query {
      debug!("added raycast to target {}", entity);
      commands.entity(entity).insert((RayCastPickable,));
    }
  }

  fn handle_pick_events(
    mut ui_state: ResMut<ui::State>,
    mut click_events: EventReader<Pointer<Click>>,
    mut q_egui: Query<&mut EguiContext>,
    q_raycast_pickables: Query<&RayCastPickable>,
  ) {
    let mut egui = q_egui.single_mut();
    let egui_context = egui.get_mut();

    for click in click_events.read() {
      let target = click.target;

      let modifiers = egui_context.input(|i| i.modifiers);

      if q_raycast_pickables.get(target).is_ok() {
        ui_state.add_selected(target, modifiers.ctrl);
      }
    }
  }

  fn draw_mesh_intersections(pointers: Query<&PointerInteraction>, mut gizmos: Gizmos) {
    for (point, normal) in pointers
      .iter()
      .filter_map(|interaction| interaction.get_nearest_hit())
      .filter_map(|(_entity, hit)| hit.position.zip(hit.normal))
    {
      gizmos.sphere(point, 0.05, RED_500);
      gizmos.arrow(point, point + normal.normalize() * 0.5, PINK_100);
    }
  }
}
