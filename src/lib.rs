pub mod assets;
mod cache;
mod input;
mod scenes;
mod ui;
mod util;
mod view;

use assets::{Prefab, PrefabPlugin, PrefabRegistrar, Prefabs, StaticPrefab};
use bevy::color::palettes::tailwind::{PINK_100, RED_500};
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use bevy::window::{EnabledButtons, WindowCloseRequested, WindowMode};
use bevy_egui::EguiContext;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use cache::Cache;
use input::InputPlugin;
use scenes::{LoadEvent, SaveEvent, SceneTypeRegistry};
use ui::UiPlugin;

pub use bevy;
pub use serde;
pub use util::*;
use view::{ActiveEditorCamera, EditorCamera, EditorCamera3d, ViewPlugin};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum EditorState {
  Editing,
  Testing,
}

pub struct Editor {
  app: App,
  scene_type_registry: SceneTypeRegistry,
  prefab_registrar: PrefabRegistrar,
}

impl Editor {
  pub fn new(app: App) -> Self {
    Self {
      app,
      scene_type_registry: default(),
      prefab_registrar: default(),
    }
  }

  pub fn on_enter_editor_hook<System, M>(&mut self, system: System) -> &mut Self
  where
    System: IntoSystem<(), (), M>,
  {
    self.app.add_systems(OnEnter(EditorState::Editing), system);

    self
  }

  pub fn swap_to_camera<C>(&mut self) -> &mut Self
  where
    C: Component,
  {
    fn swap_cameras<Enabled, Disabled>(
      mut q_enabled_cameras: Query<&mut Camera, (With<Enabled>, Without<Disabled>)>,
      mut q_disabled_cameras: Query<&mut Camera, (With<Disabled>, Without<Enabled>)>,
    ) where
      Enabled: Component,
      Disabled: Component,
    {
      for mut cam in &mut q_enabled_cameras {
        cam.is_active = true;
      }

      for mut cam in &mut q_disabled_cameras {
        cam.is_active = false;
      }
    }

    self
      .app
      .add_systems(PostStartup, swap_cameras::<ActiveEditorCamera, C>)
      .add_systems(
        OnEnter(EditorState::Testing),
        swap_cameras::<C, ActiveEditorCamera>,
      )
      .add_systems(
        OnEnter(EditorState::Editing),
        swap_cameras::<ActiveEditorCamera, C>,
      );

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

  pub fn run(self) -> AppExit {
    let Self {
      mut app,
      scene_type_registry,
      prefab_registrar,
    } = self;

    app
      .add_plugins(EditorPlugin)
      .insert_resource(Cache::load_or_default())
      .insert_resource(scene_type_registry)
      .insert_resource(prefab_registrar)
      .run()
  }

  fn register_type<T>(&mut self)
  where
    T: GetTypeRegistration,
  {
    self.scene_type_registry.write().register::<T>();
    self.app.register_type::<T>();
  }
}

struct EditorPlugin;

impl Plugin for EditorPlugin {
  fn build(&self, app: &mut App) {
    let mut window = Window::default();

    window.title = String::from("Bevy Editor");
    window.mode = WindowMode::Windowed;
    window.visible = false;
    window.enabled_buttons = EnabledButtons {
      close: true,
      maximize: true,
      minimize: false, // minimize causes a crash
    };

    app
      .add_plugins((
        DefaultPlugins.set(WindowPlugin {
          primary_window: Some(window),
          close_when_requested: false,
          ..default()
        }),
        ViewPlugin,
        MeshPickingPlugin,
        DefaultInspectorConfigPlugin,
        UiPlugin,
        InputPlugin,
      ))
      .add_event::<SaveEvent>()
      .add_event::<LoadEvent>()
      .insert_state(EditorState::Editing)
      .add_systems(Startup, (Self::startup, Self::initialize_types))
      .add_systems(PostStartup, Self::post_startup)
      .add_systems(OnEnter(EditorState::Editing), Self::on_enter)
      .add_systems(OnExit(EditorState::Editing), Self::on_exit)
      .add_systems(
        Update,
        (
          (
            input::global_input_actions,
            (
              input::handle_input,
              scenes::check_for_saves,
              scenes::check_for_loads,
              Self::auto_register_targets,
              Self::handle_pick_events,
              Self::draw_mesh_intersections,
            )
              .run_if(in_state(EditorState::Editing)),
            ui::render,
          )
            .chain(),
          (
            Self::on_close_requested,
            (EditorCamera::on_app_exit, EditorCamera3d::on_app_exit),
            Self::on_app_exit,
          )
            .chain(),
        ),
      );
  }
}

impl EditorPlugin {
  fn startup(mut picking_settings: ResMut<MeshPickingSettings>) {
    picking_settings.require_markers = true;
  }

  fn post_startup(mut q_windows: Query<&mut Window>) {
    for mut window in &mut q_windows {
      window.visible = true;
    }
  }

  fn initialize_types(world: &mut World) {
    let Some(registrar) = world.remove_resource::<PrefabRegistrar>() else {
      return;
    };

    let prefabs = Prefabs::new(world, registrar);

    world.insert_resource(prefabs);
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

    for click in click_events
      .read()
      .filter(|evt| evt.button == PointerButton::Primary)
    {
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

  fn on_close_requested(
    close_requests: EventReader<WindowCloseRequested>,
    mut app_exit: EventWriter<AppExit>,
  ) {
    if !close_requests.is_empty() {
      app_exit.send(AppExit::Success);
    }
  }

  fn on_app_exit(app_exit: EventReader<AppExit>, cache: Res<Cache>) {
    if !app_exit.is_empty() {
      cache.save();
    }
  }
}
