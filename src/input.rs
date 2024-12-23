use crate::{hide_cursor, show_cursor, EditorState};
use bevy::prelude::*;
use leafwing_input_manager::{
  plugin::InputManagerPlugin,
  prelude::{ActionState, Buttonlike, InputMap, MouseScrollAxis},
  Actionlike, InputManagerBundle,
};

#[derive(Component)]
pub struct EditorInput;

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum EditorActions {
  Play,
  PanCamera,
  OrbitCamera,
  #[actionlike(Axis)]
  Zoom,
  MoveNorth,
  MoveSouth,
  MoveWest,
  MoveEast,
}

pub struct InputPlugin;

impl InputPlugin {
  fn init_input(mut commands: Commands) {
    let inputs = InputMap::default()
      .with(EditorActions::Play, KeyCode::F5)
      .with(EditorActions::OrbitCamera, MouseButton::Right)
      .with(EditorActions::PanCamera, MouseButton::Middle)
      .with_axis(EditorActions::Zoom, MouseScrollAxis::Y)
      .with(EditorActions::MoveNorth, KeyCode::KeyW)
      .with(EditorActions::MoveSouth, KeyCode::KeyS)
      .with(EditorActions::MoveWest, KeyCode::KeyA)
      .with(EditorActions::MoveEast, KeyCode::KeyD);

    commands.spawn((
      Name::new("Editor Input"),
      EditorInput,
      InputManagerBundle::with_map(inputs),
    ));
  }
}

impl Plugin for InputPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins(InputManagerPlugin::<EditorActions>::default())
      .register_type::<Box<dyn Buttonlike>>()
      .add_systems(Startup, Self::init_input);
  }
}

pub fn global_input_actions(
  q_action_states: Query<&ActionState<EditorActions>>,
  current_state: Res<State<EditorState>>,
  mut next_editor_state: ResMut<NextState<EditorState>>,
) {
  for action_state in &q_action_states {
    if action_state.just_pressed(&EditorActions::Play) {
      if *current_state.get() == EditorState::Editing {
        next_editor_state.set(EditorState::Testing);
      } else {
        next_editor_state.set(EditorState::Editing);
      }
    }
  }
}

pub fn handle_input(
  q_action_states: Query<&ActionState<EditorActions>>,
  mut windows: Query<&mut Window>,
) {
  for action_state in &q_action_states {
    if action_state.just_pressed(&EditorActions::OrbitCamera)
      || action_state.just_pressed(&EditorActions::PanCamera)
    {
      let Ok(mut window) = windows.get_single_mut() else {
        return;
      };

      hide_cursor(&mut window);
      continue;
    }

    if (action_state.just_released(&EditorActions::OrbitCamera)
      && action_state.released(&EditorActions::PanCamera))
      || (action_state.just_released(&EditorActions::PanCamera)
        && action_state.released(&EditorActions::OrbitCamera))
    {
      let Ok(mut window) = windows.get_single_mut() else {
        return;
      };

      show_cursor(&mut window);
    }
  }
}
