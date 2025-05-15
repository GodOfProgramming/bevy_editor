use bevy::{color::palettes::css::RED, prelude::*};
use bui::{BuiPlugin, UiEvent};

const UI: &str = include_str!("./ui/simple_button.xml");

fn main() {
  App::new()
    .add_plugins((
      DefaultPlugins,
      BuiPlugin::default().add_ui_event::<ButtonEvent>(),
    ))
    .register_type::<CenteredArea>()
    .add_systems(Startup, startup)
    .add_systems(Update, (button_system, button_event_system))
    .run();
}

fn startup(world: &mut World) {
  world.spawn(Camera2d);

  let nodes = bui::Ui::parse_all(UI).unwrap();
  let node = nodes.first().unwrap();

  if let Err(err) = node.spawn(world) {
    error!("failed to create ui: {err}");
  }
}

#[derive(Event)]
struct ButtonEvent;

impl UiEvent for ButtonEvent {
  type In = String;

  fn new(input: Self::In) -> Self {
    Self
  }
}

fn button_event_system(mut reader: EventReader<ButtonEvent>) {
  for event in reader.read() {
    println!("Got event");
  }
}

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);

#[derive(Default, Component, Reflect)]
#[require(Node = screen_node())]
#[reflect(Default)]
#[reflect(Component)]
pub struct CenteredArea;

#[derive(Component)]
struct ButtonText(&'static str);

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
  (
    Button,
    ButtonText("Sim. Button"),
    children![(Text::new("Sim. Button"),)],
  )
}

fn advanced_button() -> impl Bundle {
  (
    Button,
    ButtonText("Adv. Button"),
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

fn button_system(
  mut interaction_query: Query<
    (
      &ButtonText,
      &Interaction,
      &mut BackgroundColor,
      &mut BorderColor,
      &Children,
    ),
    (Changed<Interaction>, With<Button>),
  >,
  mut text_query: Query<&mut Text>,
  mut event_writer: EventWriter<ButtonEvent>,
) {
  for (bt, interaction, mut color, mut border_color, children) in &mut interaction_query {
    let mut text = text_query.get_mut(children[0]).unwrap();
    match *interaction {
      Interaction::Pressed => {
        **text = "Press".to_string();
        *color = PRESSED_BUTTON.into();
        border_color.0 = RED.into();
        event_writer.write(ButtonEvent);
      }
      Interaction::Hovered => {
        **text = "Hover".to_string();
        *color = HOVERED_BUTTON.into();
        border_color.0 = Color::WHITE;
      }
      Interaction::None => {
        **text = bt.0.to_string();
        *color = NORMAL_BUTTON.into();
        border_color.0 = Color::BLACK;
      }
    }
  }
}
