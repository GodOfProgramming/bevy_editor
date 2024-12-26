pub mod assets;
mod cache;
mod input;
mod scenes;
mod ui;
mod util;
mod view;

use assets::{Prefab, PrefabPlugin, PrefabRegistrar, Prefabs, StaticPrefab};
pub use bevy;
use bevy::color::palettes::tailwind::{self, PINK_100, RED_500};
use bevy::log::{Level, LogPlugin, DEFAULT_FILTER};
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use bevy::utils::tracing::level_filters::LevelFilter;
use bevy::window::{EnabledButtons, WindowCloseRequested, WindowMode};
pub use bevy_egui;
pub use bevy_egui::egui;
use bevy_egui::EguiContext;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use cache::{Cache, Saveable};
use input::InputPlugin;
use scenes::{LoadEvent, SaveEvent, SceneTypeRegistry};
pub use serde;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use ui::{CustomTab, UiPlugin};
pub use util::*;
use view::{
  ActiveEditorCamera, EditorCamera, EditorCamera2d, EditorCamera3d, ViewPlugin, ViewState,
};

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
  pub fn new() -> Self {
    let mut app = App::new();

    app.add_plugins(EditorPlugin);

    Self {
      app,
      scene_type_registry: default(),
      prefab_registrar: default(),
    }
  }

  pub fn add_custom_tab(&mut self, f: fn(&mut World, &mut egui::Ui)) -> &mut Self {
    self.app.insert_resource(CustomTab(f));
    self
  }

  pub fn add_game_camera<C>(&mut self) -> &mut Self
  where
    C: Component,
  {
    const COLOR: Srgba = tailwind::GREEN_700;

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

    fn render_2d_cameras<C: Component>(
      mut gizmos: Gizmos,
      q_cam: Query<(&Transform, &OrthographicProjection), (With<Camera2d>, With<C>)>,
    ) {
      for (transform, projection) in &q_cam {
        let rect_pos = transform.translation;
        gizmos.rect(rect_pos, projection.area.max - projection.area.min, COLOR);
      }
    }

    fn render_3d_cameras<C: Component>(
      mut gizmos: Gizmos,
      q_cam: Query<(&Transform, &Projection), (With<Camera3d>, With<C>)>,
    ) {
      fn show_camera(transform: &Transform, scaler: f32, gizmos: &mut Gizmos) {
        gizmos.cuboid(transform.clone(), COLOR);

        let forward = transform.forward().as_vec3();

        let rect_pos = transform.translation + forward;
        let rect_iso = Isometry3d::new(rect_pos, transform.rotation);
        let rect_dim = Vec2::new(1.0 * scaler, 1.0);

        gizmos.rect(rect_iso, rect_dim, COLOR);

        let start = transform.translation + forward * transform.scale / 2.0;

        let rect_corners = [
          rect_dim,
          -rect_dim,
          rect_dim.with_x(-rect_dim.x),
          rect_dim.with_y(-rect_dim.y),
        ]
        .map(|corner| Vec3::from((corner / 2.0, 0.0)))
        .map(|corner| rect_iso * corner);

        for corner in rect_corners {
          gizmos.line(start, corner, COLOR);
        }
      }

      for (transform, projection) in &q_cam {
        match projection {
          Projection::Perspective(perspective) => {
            show_camera(transform, perspective.aspect_ratio, &mut gizmos);
          }
          Projection::Orthographic(orthographic) => {
            show_camera(transform, orthographic.scale, &mut gizmos);
          }
        }
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
      )
      .add_systems(
        Update,
        (
          render_2d_cameras::<C>.run_if(in_state(ViewState::Camera2D)),
          render_3d_cameras::<C>.run_if(in_state(ViewState::Camera3D)),
        )
          .run_if(in_state(EditorState::Editing)),
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

  pub fn launch(self) -> AppExit {
    let Self {
      mut app,
      scene_type_registry,
      prefab_registrar,
    } = self;

    app
      .insert_resource(scene_type_registry)
      .insert_resource(prefab_registrar);

    debug!("Launching Editor");

    app.run()
  }

  fn register_type<T>(&mut self)
  where
    T: GetTypeRegistration,
  {
    self.scene_type_registry.write().register::<T>();
    self.app.register_type::<T>();
  }
}

impl Deref for Editor {
  type Target = App;
  fn deref(&self) -> &Self::Target {
    &self.app
  }
}

impl DerefMut for Editor {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.app
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

    let cache = Cache::load_or_default();

    let log_info = cache.get::<LogInfo>().unwrap_or_default();

    app
      .add_plugins((
        DefaultPlugins
          .set(WindowPlugin {
            primary_window: Some(window),
            close_when_requested: false,
            ..default()
          })
          .set(LogPlugin {
            level: log_info.level.into(),
            filter: DEFAULT_FILTER.to_string(),
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
      .insert_resource(cache)
      .insert_resource(log_info)
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
              scenes::check_for_saves,
              scenes::check_for_loads,
              Self::auto_register_targets,
              Self::handle_pick_events,
              Self::draw_mesh_intersections,
            )
              .run_if(in_state(EditorState::Editing)),
          )
            .chain(),
          (
            Self::on_close_requested,
            (
              EditorCamera::on_app_exit,
              EditorCamera2d::on_app_exit,
              EditorCamera3d::on_app_exit,
            ),
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

  fn on_exit(
    mut commands: Commands,
    q_targets: Query<Entity, (With<RayCastPickable>, Without<Camera>)>,
  ) {
    for target in q_targets.iter() {
      commands
        .entity(target)
        .remove::<(RayCastPickable, PickingBehavior)>();
    }
  }

  fn initialize_types(world: &mut World) {
    let Some(registrar) = world.remove_resource::<PrefabRegistrar>() else {
      return;
    };

    let prefabs = Prefabs::new(world, registrar);

    world.insert_resource(prefabs);
  }

  fn on_enter(mut q_windows: Query<&mut Window>) {
    for mut window in q_windows.iter_mut() {
      show_cursor(&mut window);
    }
  }

  fn auto_register_targets(
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
    mut ui_state: ResMut<ui::State>,
    mut click_events: EventReader<Pointer<Click>>,
    mut q_egui: Query<&mut EguiContext>,
    q_raycast_pickables: Query<&RayCastPickable>,
  ) {
    let mut egui = q_egui.single_mut();
    let egui_context = egui.get_mut();
    let modifiers = egui_context.input(|i| i.modifiers);

    for click in click_events
      .read()
      .filter(|evt| evt.button == PointerButton::Primary)
    {
      let target = click.target;

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

  fn on_app_exit(app_exit: EventReader<AppExit>, log_info: Res<LogInfo>, mut cache: ResMut<Cache>) {
    if !app_exit.is_empty() {
      cache.store(log_info.clone());
      cache.save();
    }
  }
}

#[derive(Default, Clone, Resource, Serialize, Deserialize)]
struct LogInfo {
  level: LogLevel,
}

impl Saveable for LogInfo {
  const KEY: &str = "logging";
}

#[derive(Reflect, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
enum LogLevel {
  Trace,
  Debug,
  #[default]
  Info,
  Warn,
  Error,
}

impl Into<Level> for LogLevel {
  fn into(self) -> Level {
    match self {
      LogLevel::Trace => Level::TRACE,
      LogLevel::Debug => Level::DEBUG,
      LogLevel::Info => Level::INFO,
      LogLevel::Warn => Level::WARN,
      LogLevel::Error => Level::ERROR,
    }
  }
}

impl Into<LevelFilter> for LogLevel {
  fn into(self) -> LevelFilter {
    match self {
      LogLevel::Trace => LevelFilter::TRACE,
      LogLevel::Debug => LevelFilter::DEBUG,
      LogLevel::Info => LevelFilter::INFO,
      LogLevel::Warn => LevelFilter::WARN,
      LogLevel::Error => LevelFilter::ERROR,
    }
  }
}
