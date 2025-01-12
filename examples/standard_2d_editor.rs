use bevy::{color::palettes::css::PURPLE, prelude::*};
use bevy_editor::Editor;

fn main() {
  let mut editor = Editor::default();

  editor
    .add_game_camera::<GameCamera>()
    .add_systems(Startup, startup);

  editor.launch();
}

#[derive(Component, Reflect)]
struct GameCamera;

fn startup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<ColorMaterial>>,
) {
  commands.spawn((Name::new("Game Camera"), GameCamera, Camera2d));

  commands.spawn((
    Name::new("Purple Square Sprite"),
    Sprite {
      color: Color::from(PURPLE),
      ..default()
    },
    Transform::default()
      .with_translation(Vec3::new(-64.0, 0.0, 0.0))
      .with_scale(Vec3::new(32.0, 32.0, 1.0)),
  ));

  commands.spawn((
    Name::new("Purple Square"),
    Mesh2d(meshes.add(Rectangle::default())),
    MeshMaterial2d(materials.add(Color::from(PURPLE))),
    Transform::default()
      .with_translation(Vec3::new(64.0, 0.0, 0.0))
      .with_scale(Vec3::new(32.0, 32.0, 1.0)),
  ));
}
