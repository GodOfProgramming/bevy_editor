use bevy::prelude::*;

#[derive(Resource, Clone)]
pub struct Hotkeys {
  pub translate_gizmo: KeyCode,
  pub rotate_gizmo: KeyCode,
  pub scale_gizmo: KeyCode,
}

impl Default for Hotkeys {
  fn default() -> Self {
    Self {
      translate_gizmo: KeyCode::KeyT,
      rotate_gizmo: KeyCode::KeyR,
      scale_gizmo: KeyCode::KeyS,
    }
  }
}
