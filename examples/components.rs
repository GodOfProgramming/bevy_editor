use bevy::prelude::*;
use bevy_editor::Editor;

fn main() {
  let mut editor = Editor::default();

  editor
    .register_components::<(SpinComponent, GrowthComponent)>()
    .add_systems(Startup, startup)
    .add_systems(Update, (spin, grow));

  editor.launch();
}

fn startup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  // circular base
  commands.spawn((
    Name::new("Base"),
    Mesh3d(meshes.add(Circle::new(4.0))),
    MeshMaterial3d(materials.add(Color::WHITE)),
    Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
  ));
  // cube
  commands.spawn((
    Name::new("Cube"),
    Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
    MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
    Transform::from_xyz(0.0, 0.5, 0.0),
  ));
  // light
  commands.spawn((
    Name::new("Light"),
    PointLight {
      shadows_enabled: true,
      ..default()
    },
    Transform::from_xyz(4.0, 8.0, 4.0),
  ));
}

fn spin(mut q_spins: Query<(&mut Transform, &SpinComponent)>) {
  for (mut transform, spin) in &mut q_spins {
    if spin.velocity != 0.0 {
      transform.rotation *= Quat::from_axis_angle(spin.angle, spin.velocity / 100.0);
    }
  }
}

fn grow(mut q_growths: Query<(&mut Transform, &GrowthComponent)>) {
  for (mut transform, growth) in &mut q_growths {
    if growth.rate != 0.0 {
      transform.scale += growth.dims * growth.rate / 100.0;
    }
  }
}

#[derive(Component, Reflect, Default)]
struct SpinComponent {
  velocity: f32,
  angle: Vec3,
}

#[derive(Component, Reflect, Default)]
struct GrowthComponent {
  rate: f32,
  dims: Vec3,
}
