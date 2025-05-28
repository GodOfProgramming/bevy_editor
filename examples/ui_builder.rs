use beditor::Editor;
use bevy::prelude::*;
use bui::BuiPlugin;

fn main() {
  let mut editor = Editor::default();

  editor
    .register_component::<Name>()
    .register_component::<Node>()
    .register_component::<Text>()
    .register_component::<Button>()
    .add_plugins(BuiPlugin::default());

  editor.run();
}
