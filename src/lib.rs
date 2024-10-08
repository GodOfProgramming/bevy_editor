mod input;
mod ui;
mod util;
mod view;

use backends::egui::EguiPointer;
use backends::raycast::{RaycastBackendSettings, RaycastPickable};
use bevy::prelude::*;
use bevy::reflect::{GetTypeRegistration, ReflectKind};
use bevy::state::state::FreelyMutableState;
use bevy::transform::TransformSystem;
use bevy::utils::HashMap;
use bevy::{render::camera::Viewport, window::PrimaryWindow};
use bevy_egui::{EguiContext, EguiSet};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_mod_picking::prelude::*;
use bevy_transform_gizmo::{GizmoPickSource, GizmoTransformable, TransformGizmoPlugin};
use serde::Serialize;
use std::any::Any;
use std::fs;
use std::marker::PhantomData;
use std::path::PathBuf;
use ui::UiPlugin;

pub use bevy;
pub use input::Hotkeys;
pub use serde;
pub use util::*;
pub use view::EditorCameraBundle;

pub struct Editor {
  app: App,
  hashmap: HashMap<String, u64>,
  cache_dir: PathBuf,
}

impl Editor {
  pub fn new<C, S>(mut app: App, config: EditorConfig<C, S>) -> Self
  where
    C: Component + Clone,
    S: FreelyMutableState + Copy,
  {
    app.add_plugins(EditorPlugin::new(config));

    let mut cache_dir = std::env::current_exe()
      .unwrap()
      .parent()
      .unwrap()
      .to_path_buf();

    cache_dir.push("cache");

    Self {
      app,
      hashmap: default(),
      cache_dir,
    }
  }

  pub fn register_type<T>(&mut self) -> &mut Self
  where
    T: GetTypeRegistration + Default + ?Sized + Serialize,
  {
    let registration = T::get_type_registration();
    let path = registration.type_info().type_path();

    let default_value = T::default();
    let ron_value = ron::to_string(&default_value).unwrap();
    let hash_value = ron_value.hash_value();

    let old_hash = self.hashmap.insert(path.to_string(), hash_value);

    if old_hash.map(|oh| hash_value != oh).unwrap_or(true) {
      let file_path = PathBuf::from(&path.replace("::", "/"));

      let mut output_path = self.cache_dir.clone();

      output_path.push(file_path);

      if let Some(dir_path) = output_path.parent() {
        fs::create_dir_all(dir_path).unwrap();
      }

      println!("writing {} to {}", path, output_path.display());
      fs::write(output_path, ron_value).unwrap();
    }

    self.app.register_type::<T>();

    self
  }

  pub fn run(&mut self) -> AppExit {
    self.app.run()
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
      .add_systems(Startup, Self::startup)
      .add_systems(OnEnter(self.config.editor_state), Self::on_enter)
      .add_systems(OnExit(self.config.editor_state), Self::on_exit)
      .add_systems(
        Update,
        (
          Self::handle_input,
          Self::check_for_saves,
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

  fn check_for_loads(world: &mut World) {
    world.resource_scope(|world, load_events: Mut<Events<LoadEvent>>| {
      load_events.get_reader().read(&load_events).for_each(|e| {
        let type_registry = world.resource::<AppTypeRegistry>().0.clone();
        let type_registry = type_registry.read();

        let desc = MapDescriptor::from(e.file().clone());

        for obj in &desc.objects {
          for component in &obj.components {
            if let Some(c) = type_registry.get_with_type_path(&component.name) {
              let Some(reflect_default) = c.data::<ReflectDefault>() else {
                error!("failed to load {}", component.name);
                return;
              };

              let instance: Box<dyn Reflect> = reflect_default.default();

              match instance.reflect_kind() {
                ReflectKind::Struct => todo!(),
                ReflectKind::TupleStruct => todo!(),
                ReflectKind::Tuple => todo!(),
                ReflectKind::List => todo!(),
                ReflectKind::Array => todo!(),
                ReflectKind::Map => todo!(),
                ReflectKind::Enum => todo!(),
                ReflectKind::Value => todo!(),
              }
            }
          }
        }
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
    // let children = world
    //   .get::<Children>(entity)
    //   .map(|children| children.iter().copied().collect::<Vec<_>>());
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

struct MapDescriptor {
  objects: Vec<ObjectDescriptor>,
}

struct ObjectDescriptor {
  components: Vec<ComponentDescriptor>,
}

struct ComponentDescriptor {
  name: String,
  fields: HashMap<String, Box<dyn Any>>,
}

impl From<PathBuf> for MapDescriptor {
  fn from(value: PathBuf) -> Self {
    todo!();
  }
}
