use bevy::{prelude::*, state::state::FreelyMutableState};

use crate::{hide_cursor, show_cursor, EditorConfig, EditorState};

#[derive(Resource, Clone)]
pub struct Hotkeys {
  pub play: KeyCode,

  pub move_cam: KeyCode,

  pub translate_gizmo: KeyCode,
  pub rotate_gizmo: KeyCode,
  pub scale_gizmo: KeyCode,
}

impl Default for Hotkeys {
  fn default() -> Self {
    Self {
      play: KeyCode::Escape,
      move_cam: KeyCode::ShiftLeft,
      translate_gizmo: KeyCode::KeyT,
      rotate_gizmo: KeyCode::KeyR,
      scale_gizmo: KeyCode::KeyS,
    }
  }
}

pub fn special_input<C, S>(
  config: Res<EditorConfig<C, S>>,
  hotkeys: Res<Hotkeys>,
  input: Res<ButtonInput<KeyCode>>,
  current_state: Res<State<S>>,
  mut next_game_state: ResMut<NextState<S>>,
) where
  C: Component + Clone,
  S: FreelyMutableState + Copy,
{
  if input.just_pressed(hotkeys.play) {
    if *current_state.get() == config.gameplay_state {
      next_game_state.set(config.editor_state);
    } else {
      next_game_state.set(config.gameplay_state);
    }
  }
}

pub fn handle_input(
  hotkeys: Res<Hotkeys>,
  input: Res<ButtonInput<KeyCode>>,
  mut windows: Query<&mut Window>,
  mut next_editor_state: ResMut<NextState<EditorState>>,
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
}
