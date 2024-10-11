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

  editor.register_type("basic", Plane::basic);

  editor.run();
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

#[derive(Reflect, Clone, Bundle)]
struct Plane {
  name: Name,
  mesh_bundle: StandardMaterialMeshBundle,
}

impl Plane {
  fn basic(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
  ) -> Self {
    Self {
      name: Name::new("ground"),
      mesh_bundle: StandardMaterialMeshBundle {
        mesh: meshes.add(Plane3d::default()),
        transform: Transform::default().with_scale(Vec3::splat(100.0)),
        material: materials.add(StandardMaterial {
          base_color: Color::linear_rgb(0.3, 0.2, 0.7),
          ..default()
        }),
        ..default()
      },
    }
  }
}

#[derive(Default, Reflect, Clone, Bundle)]
struct StandardMaterialMeshBundle {
  pub mesh: Handle<Mesh>,
  pub material: Handle<StandardMaterial>,
  pub transform: Transform,
  pub global_transform: GlobalTransform,
  /// User indication of whether an entity is visible
  pub visibility: Visibility,
  /// Inherited visibility of an entity.
  pub inherited_visibility: InheritedVisibility,
  /// Algorithmically-computed indication of whether an entity is visible and should be extracted for rendering
  pub view_visibility: ViewVisibility,
}
