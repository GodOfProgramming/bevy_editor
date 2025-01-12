pub mod assets;
mod cache;
mod input;
mod scenes;
mod ui;
mod util;
mod view;

pub use bevy_egui;
pub use bevy_egui::egui;
pub use serde;
pub use ui::{RawUi, Ui};
pub use util::*;
pub use uuid;

use assets::{Prefab, PrefabPlugin, PrefabRegistrar, Prefabs, StaticPrefab};
use bevy::{
  color::palettes::tailwind::{PINK_100, RED_500},
  log::{LogPlugin, DEFAULT_FILTER},
  picking::pointer::PointerInteraction,
  prelude::*,
  reflect::GetTypeRegistration,
  window::{WindowCloseRequested, WindowMode},
};
use bevy_egui::EguiContext;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use cache::Cache;
use input::InputPlugin;
use parking_lot::Mutex;
use scenes::{LoadEvent, SaveEvent, SceneTypeRegistry};
use std::cell::RefCell;
use ui::{managers::UiManager, prebuilt::game_view::GameView, UiPlugin};
use view::EditorViewPlugin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum EditorState {
  Editing,
  Testing,
  Exiting,
}

#[derive(Deref, DerefMut)]
pub struct Editor {
  #[deref]
  app: App,
  cache: Cache,
  scene_type_registry: SceneTypeRegistry,
  prefab_registrar: PrefabRegistrar,
  layout: UiManager,
}

impl Default for Editor {
  fn default() -> Self {
    Self::new(App::new())
  }
}

impl Editor {
  pub fn new(app: App) -> Self {
    Self::new_with_defaults(app, DefaultPlugins)
  }

  pub fn new_with_defaults(mut app: App, plugins: impl PluginGroup) -> Self {
    let cache = Cache::load_or_default();
    let log_info = cache.get::<LogInfo>().unwrap_or_default();

    let defaults = plugins.build();

    app
      .add_plugins(
        defaults
          .add(WindowPlugin {
            primary_window: Some(Window {
              title: String::from("Bevy Editor"),
              mode: WindowMode::Windowed,
              visible: false,
              ..default()
            }),
            close_when_requested: false,
            ..default()
          })
          .add(LogPlugin {
            level: log_info.level.into(),
            filter: DEFAULT_FILTER.to_string(),
            ..default()
          }),
      )
      .insert_resource(log_info);

    Self {
      app,
      cache,
      scene_type_registry: default(),
      prefab_registrar: default(),
      layout: default(),
    }
  }

  pub fn register_ui<U: RawUi>(&mut self) -> &mut Self {
    self.layout.register::<U>();
    self
  }

  pub fn register_static_prefab<T>(&mut self) -> &mut Self
  where
    T: StaticPrefab,
  {
    self.register_type::<T>();

    self.prefab_registrar.register::<T>();

    self
  }

  pub fn load_prefabs<T>(&mut self) -> &mut Self
  where
    T: Prefab,
  {
    self.register_type::<T>();
    self.app.add_plugins(PrefabPlugin::<T>::default());
    self
  }

  pub fn add_game_camera<C>(&mut self) -> &mut Self
  where
    C: Component + Reflect + TypePath,
  {
    view::add_game_camera::<C>(&mut self.app);
    self.register_ui::<GameView<C>>()
  }

  fn register_type<T>(&mut self)
  where
    T: GetTypeRegistration,
  {
    self.scene_type_registry.write().register::<T>();
    self.app.register_type::<T>();
  }

  // systems

  fn set_picking_settings(mut picking_settings: ResMut<MeshPickingSettings>) {
    picking_settings.require_markers = true;
  }

  fn show_window(mut q_windows: Query<&mut Window>) {
    for mut window in &mut q_windows {
      window.visible = true;
    }
  }

  fn show_window_cursor(mut q_windows: Query<&mut Window>) {
    for mut window in q_windows.iter_mut() {
      show_cursor(&mut window);
    }
  }

  fn remove_picking_from_targets(
    mut commands: Commands,
    q_targets: Query<Entity, (With<RayCastPickable>, Without<Camera>)>,
  ) {
    for target in q_targets.iter() {
      commands
        .entity(target)
        .remove::<(RayCastPickable, PickingBehavior)>();
    }
  }

  fn initialize_prefabs(world: &mut World) {
    let Some(registrar) = world.remove_resource::<PrefabRegistrar>() else {
      return;
    };

    let prefabs = Prefabs::new(world, registrar);

    world.insert_resource(prefabs);
  }

  #[allow(clippy::type_complexity)]
  fn auto_register_picking_targets(
    mut commands: Commands,
    q_entities: Query<
      Entity,
      (
        Without<RayCastPickable>,
        Or<(With<Sprite>, With<Mesh2d>, With<Mesh3d>)>,
      ),
    >,
  ) {
    for entity in &q_entities {
      debug!("Registered Picking: {}", entity);
      commands.entity(entity).insert((
        RayCastPickable,
        PickingBehavior {
          is_hoverable: true,
          should_block_lower: true,
        },
      ));
    }
  }

  fn handle_pick_events(
    mut selection: ResMut<ui::InspectorSelection>,
    mut click_events: EventReader<Pointer<Click>>,
    mut q_egui: Single<&mut EguiContext>,
    q_raycast_pickables: Query<&RayCastPickable>,
  ) {
    let egui_context = q_egui.get_mut();
    let modifiers = egui_context.input(|i| i.modifiers);

    for click in click_events
      .read()
      .filter(|evt| evt.button == PointerButton::Primary)
    {
      let target = click.target;

      if q_raycast_pickables.get(target).is_ok() {
        selection.add_selected(target, modifiers.ctrl);
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

  fn on_close_requested(
    close_requests: EventReader<WindowCloseRequested>,
    mut next_editor_state: ResMut<NextState<EditorState>>,
  ) {
    if !close_requests.is_empty() {
      next_editor_state.set(EditorState::Exiting)
    }
  }

  fn on_app_exit(cache: ResMut<Cache>, mut app_exit: EventWriter<AppExit>) {
    cache.save();
    app_exit.send(AppExit::Success);
  }

  pub fn build(self) -> App {
    let Self {
      mut app,
      scene_type_registry,
      prefab_registrar,
      layout,
      cache,
    } = self;

    app
      .add_plugins((
        EditorViewPlugin,
        MeshPickingPlugin,
        DefaultInspectorConfigPlugin,
        InputPlugin,
        UiPlugin(Mutex::new(RefCell::new(Some(layout)))),
      ))
      .insert_resource(cache)
      .insert_resource(scene_type_registry)
      .insert_resource(prefab_registrar)
      .insert_state(EditorState::Editing)
      .add_event::<SaveEvent>()
      .add_event::<LoadEvent>()
      .configure_sets(
        Update,
        (
          EditorGlobal,
          Editing
            .in_set(EditorGlobal)
            .run_if(in_state(EditorState::Editing)),
        ),
      )
      .add_systems(
        Startup,
        (Self::set_picking_settings, Self::initialize_prefabs),
      )
      .add_systems(PostStartup, Self::show_window)
      .add_systems(OnEnter(EditorState::Editing), Self::show_window_cursor)
      .add_systems(
        OnExit(EditorState::Editing),
        Self::remove_picking_from_targets,
      )
      .add_systems(
        Update,
        (
          scenes::check_for_saves,
          scenes::check_for_loads,
          Self::on_close_requested,
          Self::draw_mesh_intersections,
          Self::auto_register_picking_targets,
          Self::handle_pick_events,
        )
          .in_set(Editing),
      )
      .add_systems(Update, input::global_input_actions.in_set(EditorGlobal))
      .add_systems(
        OnEnter(EditorState::Exiting),
        (
          (
            view::save_view_state,
            view::view2d::save_settings,
            view::view3d::save_settings,
            UiPlugin::on_app_exit,
            LogInfo::on_app_exit,
          ),
          Self::on_app_exit,
        )
          .chain()
          .in_set(EditorGlobal),
      );

    app
  }
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct EditorGlobal;

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct Editing;
