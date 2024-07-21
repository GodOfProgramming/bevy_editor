use bevy::{prelude::*, window::CursorGrabMode};

pub fn show_cursor(window: &mut Window) {
  window.cursor.visible = true;
  window.cursor.grab_mode = CursorGrabMode::None;
}

pub fn hide_cursor(window: &mut Window) {
  window.cursor.visible = false;
  window.cursor.grab_mode = CursorGrabMode::Locked;
}
