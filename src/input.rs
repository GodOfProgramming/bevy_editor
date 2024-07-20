use bevy::prelude::*;

#[derive(Resource, Clone)]
pub struct Hotkeys {
  pub play_current_level: KeyCode,

  pub translate_gizmo: KeyCode,
  pub rotate_gizmo: KeyCode,
  pub scale_gizmo: KeyCode,
}

impl Default for Hotkeys {
  fn default() -> Self {
    Self {
      play_current_level: KeyCode::Escape,
      translate_gizmo: KeyCode::KeyT,
      rotate_gizmo: KeyCode::KeyR,
      scale_gizmo: KeyCode::KeyS,
    }
  }
}
