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
#[require(EditorCamera, Name(camera_name))]
struct MainCamera;

fn camera_name() -> Name {
  Name::new("Main Camera")
}

fn startup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  commands.spawn((
    MainCamera,
    Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
  ));

  // circular base
  commands.spawn((
    Mesh3d(meshes.add(Circle::new(4.0))),
    MeshMaterial3d(materials.add(Color::WHITE)),
    Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
  ));
  // cube
  commands.spawn((
    Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
    MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
    Transform::from_xyz(0.0, 0.5, 0.0),
  ));
  // light
  commands.spawn((
    PointLight {
      shadows_enabled: true,
      ..default()
    },
    Transform::from_xyz(4.0, 8.0, 4.0),
  ));
}
