use bevy::prelude::*;
use bevy_editor::{EditorConfig, EditorPlugin};

#[derive(States, Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum GameState {
  Editor,
  Gameplay,
}

fn main() {
  App::new()
    .add_plugins((
      DefaultPlugins,
      EditorPlugin::<MainCamera, GameState>::new(EditorConfig::new(
        GameState::Editor,
        GameState::Gameplay,
      ))
      .with_spawner(spawn_plane)
      .with_spawner(spawn_cube_raw),
    ))
    .add_systems(Startup, startup)
    .run();
}

#[derive(Component, Clone)]
struct MainCamera;

fn startup(mut commands: Commands) {
  commands.spawn((
    Name::new("Main Camera"),
    MainCamera,
    bevy_editor::EditorCameraBundle {
      camera_bundle: Camera3dBundle::default(),
      state: default(),
      settings: default(),
    },
  ));
}

fn spawn_plane(
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) -> impl Bundle {
  (
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

fn spawn_cube_raw(world: &mut World) {
  world.resource_scope::<Assets<Mesh>, ()>(|world, mut meshes| {
    world.resource_scope::<Assets<StandardMaterial>, ()>(|world, mut materials| {
      world.spawn((
        Name::new("cube"),
        PbrBundle {
          mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
          material: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(1.0, 1.0, 1.0),
            ..default()
          }),
          transform: Transform::default().with_translation(Vec3::Z / 2.0),
          ..default()
        },
      ));
    });
  });
}
