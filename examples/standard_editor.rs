use bevy::prelude::*;
use bevy_editor::{Editor, EditorCamera, EditorConfig};

#[derive(States, Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum GameState {
  Editor,
  Gameplay,
}

fn main() {
  let mut app = App::new();
  app
    .add_plugins(DefaultPlugins)
    .add_systems(Startup, startup);

  let config = EditorConfig::<MainCamera, GameState>::new(GameState::Editor, GameState::Gameplay);

  let editor = Editor::new(app, config);

  editor.run();
}

#[derive(Component, Clone)]
#[require(EditorCamera)]
struct MainCamera;

fn startup(mut commands: Commands) {
  commands.spawn((Name::new("Main Camera"), MainCamera));
}
