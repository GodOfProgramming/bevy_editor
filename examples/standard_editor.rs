use bevy::prelude::*;
use bevy_editor::{Editor, EditorConfig};
use serde::Serialize;

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

  let mut editor = Editor::new(app, config);

  editor.register_type::<SpawnableComponent>();

  editor.run();
}

#[derive(Component, Clone)]
struct MainCamera;

fn startup(
  mut commands: Commands,
  meshes: ResMut<Assets<Mesh>>,
  materials: ResMut<Assets<StandardMaterial>>,
) {
  commands.spawn((
    Name::new("Main Camera"),
    MainCamera,
    bevy_editor::EditorCameraBundle {
      camera_bundle: Camera3dBundle::default(),
      state: default(),
      settings: default(),
    },
  ));

  commands.spawn(spawn_plane(meshes, materials));
}

fn spawn_plane(
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) -> impl Bundle {
  return (
    Name::new("ground"),
    PbrBundle {
      mesh: meshes.add(Plane3d::default()),
      transform: Transform::default().with_scale(Vec3::splat(100.0)),
      material: materials.add(StandardMaterial {
        base_color: Color::linear_rgb(0.3, 0.2, 0.7),
        ..default()
      }),
      ..default()
    },
  );
}

#[derive(Default, Component, Reflect, Serialize)]
struct SpawnableComponent {
  life: i32,
}
