use bevy::prelude::*;
use bui::{BuiPlugin, BuiPrime};

fn main() -> Result {
  let mut app = App::new();

  app
    .add_plugins((DefaultPlugins, BuiPlugin::default()))
    .add_systems(Startup, (setup, serialize).chain());

  app.run();

  Ok(())
}

fn setup(mut commands: Commands) {
  let entity = commands
    .spawn((
      BuiPrime::new(Button),
      Node {
        width: Val::Px(150.0),
        height: Val::Px(100.0),
        ..default()
      },
      children![(
        BuiPrime::new(Text::new("Hello World")),
        TextColor(Color::WHITE)
      )],
    ))
    .id();

  commands.spawn(Serialized(entity));
}

fn serialize(world: &mut World) -> Result {
  let mut query = world.query::<&Serialized>();
  let ser = query.single(world)?;
  let entity = ser.0;
  let ui = bui::Bui::serialize(entity, world)?;

  let str: String = (&ui).try_into()?;

  world.despawn(entity);

  println!("{str}");

  Ok(())
}

#[derive(Component)]
struct Serialized(Entity);
