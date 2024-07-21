mod input;
mod ui;
mod util;
mod view;

use backends::raycast::RaycastPickable;
use bevy::prelude::*;
use bevy::state::state::FreelyMutableState;
use bevy::transform::TransformSystem;
use bevy::{render::camera::Viewport, window::PrimaryWindow};
use bevy_egui::{EguiContext, EguiSet};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_mod_picking::backends::egui::EguiPointer;
use bevy_mod_picking::prelude::*;
use std::marker::PhantomData;
use transform_gizmo_egui::GizmoMode;

pub use input::Hotkeys;
pub use util::*;
pub use view::EditorCameraBundle;

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

pub struct EditorPlugin<C, A>
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
      ))
      .insert_resource(self.hotkeys.clone())
      .insert_resource(ui::State::<C>::new())
      .insert_resource(self.config.clone())
      .insert_state(EditorState::Editing)
      .insert_state(self.config.editor_state)
      .add_systems(OnEnter(self.config.editor_state), Self::enable_mouse)
      .add_systems(
        OnExit(self.config.editor_state),
        Self::remove_raycast_targets,
      )
      .add_systems(
        Update,
        (
          Self::handle_input,
          (
            (
              Self::auto_add_raycast_target,
              Self::set_gizmo_mode,
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
      )
      .register_type::<Option<Handle<Image>>>()
      .register_type::<AlphaMode>();
  }
}

impl<C, S> EditorPlugin<C, S>
where
  C: Component + Clone,
  S: FreelyMutableState + Copy,
{
  pub fn new(config: EditorConfig<C, S>) -> Self {
    Self {
      config,
      hotkeys: default(),
      cam_component: default(),
    }
  }

  pub fn with_hotkeys(mut self, hotkeys: Hotkeys) -> Self {
    self.hotkeys = hotkeys;
    self
  }

  fn set_gizmo_mode(
    hotkeys: Res<Hotkeys>,
    input: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<ui::State<C>>,
  ) where
    C: Component,
  {
    for (key, mode) in [
      (hotkeys.scale_gizmo, GizmoMode::ScaleUniform),
      (hotkeys.rotate_gizmo, GizmoMode::RotateView),
      (hotkeys.translate_gizmo, GizmoMode::TranslateView),
    ] {
      if input.just_pressed(key) {
        ui_state.gizmo_mode = mode;
      }
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

  fn auto_add_raycast_target(
    mut commands: Commands,
    query: Query<Entity, (Without<RaycastPickable>, With<Handle<Mesh>>)>,
  ) {
    for entity in &query {
      commands
        .entity(entity)
        .insert((RaycastPickable::default(), PickableBundle::default()));
    }
  }

  fn remove_raycast_targets(
    mut commands: Commands,
    q_targets: Query<Entity, With<RaycastPickable>>,
  ) {
    for target in q_targets.iter() {
      commands.entity(target).remove::<RaycastPickable>();
    }
  }

  fn handle_pick_events(
    mut ui_state: ResMut<ui::State<C>>,
    mut click_events: EventReader<Pointer<Click>>,
    mut q_egui: Query<&mut EguiContext>,
    q_egui_entity: Query<&EguiPointer>,
  ) {
    let mut egui = q_egui.single_mut();
    let egui_context = egui.get_mut();

    for click in click_events.read() {
      if q_egui_entity.get(click.target()).is_ok() {
        continue;
      };

      let modifiers = egui_context.input(|i| i.modifiers);
      let add = modifiers.ctrl || modifiers.shift;

      ui_state
        .selected_entities
        .select_maybe_add(click.target(), add);
    }
  }

  fn enable_mouse(mut q_windows: Query<&mut Window>) {
    for mut window in q_windows.iter_mut() {
      show_cursor(&mut window);
    }
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

  fn in_editor_state(config: Res<EditorConfig<C, S>>, state: Res<State<S>>) -> bool {
    config.editor_state == **state
  }
}
