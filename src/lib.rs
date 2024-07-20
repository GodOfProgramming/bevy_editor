mod input;
mod ui;

use backends::raycast::RaycastPickable;
pub use input::Hotkeys;

use bevy::prelude::*;
use bevy::transform::TransformSystem;
use bevy::{render::camera::Viewport, window::PrimaryWindow};
use bevy_egui::{EguiContext, EguiSet};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_mod_picking::backends::egui::EguiPointer;
use bevy_mod_picking::prelude::*;
use std::marker::PhantomData;
use transform_gizmo_egui::GizmoMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum EditorState {
  Active,
  Inactive,
}

pub struct EditorPlugin<C>
where
  C: Component,
{
  hotkeys: Hotkeys,
  cam_component: PhantomData<C>,
}

impl<C> Default for EditorPlugin<C>
where
  C: Component,
{
  fn default() -> Self {
    Self {
      hotkeys: default(),
      cam_component: default(),
    }
  }
}

impl<C> EditorPlugin<C>
where
  C: Component,
{
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
}

impl<C> Plugin for EditorPlugin<C>
where
  C: Component,
{
  fn build(&self, app: &mut App) {
    app
      .add_plugins((
        bevy_egui::EguiPlugin,
        DefaultInspectorConfigPlugin,
        bevy_mod_picking::DefaultPickingPlugins,
      ))
      .insert_state(EditorState::Active)
      .insert_resource(Hotkeys::default())
      .insert_resource(ui::State::<C>::new())
      .add_systems(OnEnter(EditorState::Inactive), Self::remove_raycast_targets)
      .add_systems(
        Update,
        (
          Self::set_gizmo_mode,
          Self::auto_add_raycast_target,
          Self::handle_pick_events,
        )
          .run_if(in_state(EditorState::Active)),
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
