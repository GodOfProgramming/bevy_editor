use bevy::{platform::collections::HashMap, prelude::*};
use bui::{BuiPlugin, BuiResource, PrimaryType, ui::events::EntityEvent};
use std::fmt::Display;

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);

fn main() {
  App::new()
    .add_plugins((
      DefaultPlugins,
      BuiPlugin::builder()
        .register_element::<Zone>()
        .register_attr::<Label>()
        .register_event::<ClickEvent>()
        .register_event::<HoverEvent>()
        .register_event::<LeaveEvent>()
        .build(),
    ))
    .init_resource::<UiHandles>()
    .add_systems(Startup, startup)
    .add_systems(FixedUpdate, button_event_system)
    .run();
}

fn startup(
  mut commands: Commands,
  asset_server: Res<AssetServer>,
  mut ui_handles: ResMut<UiHandles>,
) -> Result {
  commands.spawn(Camera2d);

  let handle = asset_server.load::<bui::Bui>("simple_button.bui.xml");

  ui_handles.insert(handle.id(), handle);

  Ok(())
}

#[derive(Resource, Default, Deref, DerefMut)]
struct UiHandles(HashMap<AssetId<bui::Bui>, Handle<bui::Bui>>);

#[derive(Reflect, Default, Component)]
#[reflect(Default, Component)]
struct Label(String);

impl Display for Label {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

#[derive(Reflect, Default)]
struct ClickEvent(i32);

#[derive(Reflect, Default)]
struct HoverEvent;

#[derive(Reflect, Default)]
struct LeaveEvent;

fn button_event_system(
  mut click_reader: EventReader<EntityEvent<ClickEvent>>,
  mut hover_reader: EventReader<EntityEvent<HoverEvent>>,
  mut leave_reader: EventReader<EntityEvent<LeaveEvent>>,
  mut q_bg_colors: Query<&mut BackgroundColor>,
) {
  for event in click_reader.read() {
    let entity = event.entity();
    let Ok(mut bg) = q_bg_colors.get_mut(entity) else {
      continue;
    };

    *bg = BackgroundColor(PRESSED_BUTTON);

    info!("Click on {}", event.0);
  }

  for event in hover_reader.read() {
    let entity = event.entity();
    let Ok(mut bg) = q_bg_colors.get_mut(entity) else {
      continue;
    };

    *bg = BackgroundColor(HOVERED_BUTTON);
  }

  for event in leave_reader.read() {
    let entity = event.entity();
    let Ok(mut bg) = q_bg_colors.get_mut(entity) else {
      continue;
    };

    *bg = BackgroundColor(NORMAL_BUTTON);
  }
}

#[derive(Default, Component, Reflect)]
#[require(Node)]
#[reflect(Default)]
#[reflect(Component)]
pub struct Zone;

fn screen() -> impl Bundle {
  (screen_node(), children![simple_button(), advanced_button()])
}

fn screen_node() -> Node {
  Node {
    width: Val::Percent(100.0),
    height: Val::Percent(100.0),
    align_items: AlignItems::Center,
    justify_content: JustifyContent::Center,
    ..default()
  }
}

fn simple_button() -> impl Bundle {
  (Button, children![(Text::new("Sim. Button"),)])
}

fn advanced_button() -> impl Bundle {
  (
    Button,
    Node {
      width: Val::Px(150.0),
      height: Val::Px(65.0),
      border: UiRect::all(Val::Px(5.0)),
      // horizontally center child text
      justify_content: JustifyContent::Center,
      // vertically center child text
      align_items: AlignItems::Center,
      ..default()
    },
    BorderColor(Color::BLACK),
    BorderRadius::MAX,
    BackgroundColor(NORMAL_BUTTON),
    children![(
      Text::new("Adv. Button"),
      TextFont {
        font_size: 33.0,
        ..default()
      },
      TextColor(Color::srgb(0.9, 0.9, 0.9)),
      TextShadow::default(),
    )],
  )
}
