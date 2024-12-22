use crate::{hide_cursor, show_cursor, EditorSettings, EditorState, InternalState};
use bevy::prelude::*;

#[derive(Clone)]
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

pub fn special_input(
  settings: Res<EditorSettings>,
  input: Res<ButtonInput<KeyCode>>,
  current_state: Res<State<EditorState>>,
  mut next_editor_state: ResMut<NextState<EditorState>>,
) {
  if input.just_pressed(settings.hotkeys.play) {
    if *current_state.get() == EditorState::Editing {
      next_editor_state.set(EditorState::Testing);
    } else {
      next_editor_state.set(EditorState::Editing);
    }
  }
}

pub fn handle_input(
  settings: Res<EditorSettings>,
  input: Res<ButtonInput<KeyCode>>,
  mut windows: Query<&mut Window>,
  mut next_internal_state: ResMut<NextState<InternalState>>,
) {
  if input.just_pressed(settings.hotkeys.move_cam) {
    let Ok(mut window) = windows.get_single_mut() else {
      return;
    };

    hide_cursor(&mut window);
    next_internal_state.set(InternalState::Inspecting);
  }

  if input.just_released(settings.hotkeys.move_cam) {
    let Ok(mut window) = windows.get_single_mut() else {
      return;
    };

    show_cursor(&mut window);
    next_internal_state.set(InternalState::Editing);
  }
}
