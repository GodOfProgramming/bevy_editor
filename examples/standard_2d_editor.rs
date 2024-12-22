use bevy::{color::palettes::css::PURPLE, prelude::*};
use bevy_editor::Editor;

fn main() {
  let mut app = App::new();
  app
    .add_plugins(DefaultPlugins)
    .add_systems(Startup, startup);

  let editor = Editor::new(app);

  editor.run();
}

fn startup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<ColorMaterial>>,
) {
  commands.spawn((
    Mesh2d(meshes.add(Rectangle::default())),
    MeshMaterial2d(materials.add(Color::from(PURPLE))),
    Transform::default().with_scale(Vec3::splat(128.0)),
  ));
}
