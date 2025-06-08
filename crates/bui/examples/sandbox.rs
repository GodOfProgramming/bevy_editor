use bevy::prelude::*;
use bui::BuiPlugin;

fn main() -> Result {
  App::new()
    .add_plugins((DefaultPlugins, BuiPlugin::default()))
    .add_systems(Startup, startup)
    .run();
  Ok(())
}

#[derive(Component)]
struct HandleHolder(Handle<bui::Bui>);

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
  let bui_handle = asset_server.load::<bui::Bui>("sample.bui.xml");
  commands.spawn(HandleHolder(bui_handle));
}
