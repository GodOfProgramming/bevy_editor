use bevy::prelude::*;
use bui::{BuiPlugin, UiEvent};

const UI: &str = include_str!("./ui/simple_button.xml");

fn main() {
  App::new()
    .add_plugins((
      DefaultPlugins,
      BuiPlugin::default().add_ui_event::<ButtonEvent>(),
    ))
    .add_systems(Startup, startup)
    .run();
}

fn startup(world: &mut World) {
  let nodes = bui::Ui::parse_all(UI).unwrap();
  let node = nodes.first().unwrap();

  node.create(world).ok();
}

#[derive(Event)]
struct ButtonEvent;

impl UiEvent for ButtonEvent {
  type In = String;

  fn new(input: Self::In) -> Self {
    Self
  }
}
