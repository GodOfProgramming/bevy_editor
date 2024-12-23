use bevy::{color::palettes::css::PURPLE, prelude::*};
use bevy_editor::Editor;

fn main() {
  let mut app = App::new();
  app.add_systems(Startup, startup);

  let mut editor = Editor::new(app);

  editor.add_game_camera::<GameCamera>();

  editor.run();
}

#[derive(Component)]
struct GameCamera;

fn startup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut color_materials: ResMut<Assets<ColorMaterial>>,
) {
  commands.spawn((Name::new("Game Camera"), GameCamera, Camera2d));

  commands.spawn((
    Name::new("Purple Square Sprite"),
    Sprite {
      color: Color::from(PURPLE),
      custom_size: Some(Vec2::splat(32.0)),
      ..default()
    },
    Transform::default().with_translation(Vec3::new(-64.0, 0.0, 0.0)),
  ));

  commands.spawn((
    Name::new("Purple Square"),
    Mesh2d(meshes.add(Rectangle::default())),
    MeshMaterial2d(color_materials.add(Color::from(PURPLE))),
    Transform::default()
      .with_translation(Vec3::new(64.0, 0.0, 0.0))
      .with_scale(Vec3::new(32.0, 32.0, 0.0)),
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
