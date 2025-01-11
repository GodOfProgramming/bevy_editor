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
use ui::prebuilt::game_view::GameView;
pub use uuid;

use assets::{Prefab, PrefabPlugin, PrefabRegistrar, Prefabs, StaticPrefab};
use bevy::color::palettes::tailwind::{self, PINK_100, RED_500};
use bevy::log::{LogPlugin, DEFAULT_FILTER};
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use bevy::window::{EnabledButtons, WindowCloseRequested, WindowMode};
use bevy_egui::EguiContext;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use cache::Cache;
use input::InputPlugin;
use parking_lot::Mutex;
use scenes::{LoadEvent, SaveEvent, SceneTypeRegistry};
use std::cell::RefCell;
use ui::{managers::UiManager, UiPlugin};
pub use ui::{RawUi, Ui};
pub use util::*;
use view::{EditorCamera, EditorCamera2d, EditorCamera3d, ViewPlugin, ViewState};

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

impl Editor {
  const COLOR: Srgba = tailwind::GREEN_700;

  pub fn new() -> Self {
    Self::new_with_default_modifications(|p| p)
  }

  pub fn new_with_default_modifications<P>(f: impl FnOnce(DefaultPlugins) -> P) -> Self
  where
    P: PluginGroup,
  {
    let mut app = App::new();

    let defaults = DefaultPlugins;
    let defaults = f(defaults);

    let cache = Cache::load_or_default();
    let log_info = cache.get::<LogInfo>().unwrap_or_default();

    app
      .add_plugins(
        defaults
          .set(WindowPlugin {
            primary_window: Some(Window {
              title: String::from("Bevy Editor"),
              mode: WindowMode::Windowed,
              visible: false,
              enabled_buttons: EnabledButtons {
                close: true,
                maximize: true,
                minimize: false, // minimize causes a crash
              },
              ..default()
            }),
            close_when_requested: false,
            ..default()
          })
          .set(LogPlugin {
            level: log_info.level.into(),
            filter: DEFAULT_FILTER.to_string(),
            ..default()
          }),
      )
      .insert_resource(log_info)
      .add_plugins(EditorPlugin);

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
    self
      .app
      .register_type::<GameView<C>>()
      .add_systems(PostStartup, view::disable_camera::<C>)
      .add_systems(
        Update,
        (
          Self::render_2d_cameras::<C>.run_if(in_state(ViewState::Camera2D)),
          Self::render_3d_cameras::<C>.run_if(in_state(ViewState::Camera3D)),
        )
          .run_if(in_state(EditorState::Editing)),
      );

    self.register_ui::<GameView<C>>()
  }

  pub fn launch(self) -> AppExit {
    let Self {
      mut app,
      scene_type_registry,
      prefab_registrar,
      layout,
      cache,
    } = self;

    app
      .add_plugins(UiPlugin(Mutex::new(RefCell::new(Some(layout)))))
      .insert_resource(cache)
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

  fn render_2d_cameras<C: Component>(
    mut gizmos: Gizmos,
    q_cam: Query<(&Transform, &OrthographicProjection), (With<Camera2d>, With<C>)>,
  ) {
    for (transform, projection) in &q_cam {
      let rect_pos = transform.translation;
      gizmos.rect(
        rect_pos,
        projection.area.max - projection.area.min,
        Self::COLOR,
      );
    }
  }

  fn render_3d_cameras<C: Component>(
    mut gizmos: Gizmos,
    q_cam: Query<(&Transform, &Projection), (With<Camera3d>, With<C>)>,
  ) {
    for (transform, projection) in &q_cam {
      match projection {
        Projection::Perspective(perspective) => {
          Self::show_camera(transform, perspective.aspect_ratio, &mut gizmos);
        }
        Projection::Orthographic(orthographic) => {
          Self::show_camera(transform, orthographic.scale, &mut gizmos);
        }
      }
    }
  }

  fn show_camera(transform: &Transform, scaler: f32, gizmos: &mut Gizmos) {
    gizmos.cuboid(transform.clone(), Self::COLOR);

    let forward = transform.forward().as_vec3();

    let rect_pos = transform.translation + forward;
    let rect_iso = Isometry3d::new(rect_pos, transform.rotation);
    let rect_dim = Vec2::new(scaler, 1.0);

    gizmos.rect(rect_iso, rect_dim, Self::COLOR);

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
      gizmos.line(start, corner, Self::COLOR);
    }
  }
}

struct EditorPlugin;

impl Plugin for EditorPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins((
        ViewPlugin,
        MeshPickingPlugin,
        DefaultInspectorConfigPlugin,
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
        OnEnter(EditorState::Exiting),
        (
          (
            EditorCamera::on_app_exit,
            EditorCamera2d::on_app_exit,
            EditorCamera3d::on_app_exit,
            UiPlugin::on_app_exit,
          ),
          Self::on_app_exit,
        )
          .chain(),
      )
      .add_systems(
        FixedUpdate,
        (
          Self::on_close_requested,
          (
            input::global_input_actions,
            (
              scenes::check_for_saves,
              scenes::check_for_loads,
              Self::auto_register_targets,
              Self::handle_pick_events,
            )
              .run_if(in_state(EditorState::Editing)),
          )
            .chain(),
        ),
      )
      .add_systems(
        Update,
        Self::draw_mesh_intersections.run_if(in_state(EditorState::Editing)),
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
}
