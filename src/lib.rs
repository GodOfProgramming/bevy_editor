pub mod assets;
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
use bevy_egui::EguiContext;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use scenes::{LoadEvent, SaveEvent, SceneTypeRegistry};
use ui::UiPlugin;

pub use bevy;
pub use input::Hotkeys;
pub use serde;
pub use util::*;
use view::{EditorCamera, View3dPlugin};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum EditorState {
  Editing,
  Testing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
enum InternalState {
  Editing,
  Inspecting,
}

pub struct Editor {
  app: App,
  settings: EditorSettings,
  scene_type_registry: SceneTypeRegistry,
  prefab_registrar: PrefabRegistrar,
}

impl Editor {
  pub fn new(app: App) -> Self {
    Self {
      app,
      settings: default(),
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
      .add_systems(PostStartup, swap_cameras::<EditorCamera, C>)
      .add_systems(
        OnEnter(EditorState::Testing),
        swap_cameras::<C, EditorCamera>,
      )
      .add_systems(
        OnEnter(EditorState::Editing),
        swap_cameras::<EditorCamera, C>,
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
      settings,
      scene_type_registry,
      prefab_registrar,
    } = self;

    app
      .add_plugins(EditorPlugin::new(settings))
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

#[derive(Resource, Default, Clone)]
struct EditorSettings {
  hotkeys: Hotkeys,
}

struct EditorPlugin {
  config: EditorSettings,
}

impl Plugin for EditorPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins((
        View3dPlugin,
        MeshPickingPlugin,
        DefaultInspectorConfigPlugin,
        UiPlugin,
      ))
      .add_event::<SaveEvent>()
      .add_event::<LoadEvent>()
      .insert_resource(self.config.clone())
      .insert_state(EditorState::Editing)
      .insert_state(InternalState::Editing)
      .add_systems(Startup, (Self::startup, Self::initialize_types))
      .add_systems(OnEnter(EditorState::Editing), Self::on_enter)
      .add_systems(OnExit(EditorState::Editing), Self::on_exit)
      .add_systems(
        Update,
        (
          input::special_input,
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
      );
  }
}

impl EditorPlugin {
  fn new(config: EditorSettings) -> Self {
    Self { config }
  }

  fn startup(mut picking_settings: ResMut<MeshPickingSettings>) {
    picking_settings.require_markers = true;
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
