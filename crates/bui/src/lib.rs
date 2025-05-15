pub mod ui;
pub mod xml;

use bevy::{
  ecs::system::SystemId,
  prelude::*,
  reflect::{
    GetTypeRegistration, TypeInfo, TypeRegistration, TypeRegistry, serde::ReflectDeserializer,
  },
  utils::TypeIdMap,
};
use serde::de::DeserializeSeed;
use std::{any::TypeId, str::FromStr};

macro_rules! get_parser {
    ($value:ident, $($ty:ident),+) => {
      match $value {
        $(
          stringify!($ty) => parse_to_reflect::<$ty>,
        )*
        _ => return None,
      }
    };
}

type ParserFn = fn(&str) -> Option<Box<dyn Reflect>>;

#[derive(Default)]
pub struct BuiPlugin {
  event_map: TypeIdMap<EventInfo>,
}

impl BuiPlugin {
  pub fn add_ui_event<E: UiEvent>(mut self) -> Self {
    self.event_map.insert(
      TypeId::of::<E>(),
      EventInfo {
        registration_fn: |app, events| {
          app.add_event::<E>();

          let world = app.world_mut();
          let sys_id =
            world.register_system(|data: In<String>, mut writer: EventWriter<E>| -> Result {
              let input = E::In::from_attr(&data).ok_or_else(|| {
                let registration = E::In::get_type_registration();
                let tp = registration.type_info().type_path();
                format!("Could not parse {tp} from {}", *data)
              })?;

              let event = E::new(input);

              writer.write(event);

              Ok(())
            });

          events.add::<E>(sys_id);
        },
      },
    );
    self
  }
}

impl Plugin for BuiPlugin {
  fn build(&self, app: &mut App) {
    let mut events = UiEvents::default();

    for (_, info) in &self.event_map {
      (info.registration_fn)(app, &mut events);
    }
  }
}

pub trait UiEvent: Event {
  type In: GetTypeRegistration + FromAttr;

  fn new(input: Self::In) -> Self;
}

pub trait FromAttr
where
  Self: Sized,
{
  fn from_attr(data: &str) -> Option<Self>;
}

pub trait FromStrFromAttr: FromStr {}

impl<T> FromStrFromAttr for T where T: FromStr {}

impl<T> FromAttr for T
where
  T: FromStrFromAttr,
{
  fn from_attr(data: &str) -> Option<Self> {
    data.parse().ok()
  }
}

struct EventInfo {
  registration_fn: fn(&mut App, &mut UiEvents),
}

#[derive(Default)]
struct UiEvents {
  inner: TypeIdMap<SystemId<In<String>, Result>>,
}

impl UiEvents {
  fn add<E: UiEvent>(&mut self, id: SystemId<In<String>, Result>) {
    self.inner.insert(TypeId::of::<E>(), id);
  }
}

pub struct Ui {
  node: xml::Node,
}

impl Ui {
  pub fn parse_all(ui_xml: &str) -> Result<Vec<Ui>, xml::ParseError> {
    xml::parse(ui_xml).map(|nodes| nodes.into_iter().map(|node| Ui { node }).collect())
  }

  pub fn spawn(&self, world: &mut World) -> Result<Entity> {
    Self::spawn_node(world, &self.node)
  }

  fn spawn_node(world: &mut World, node: &xml::Node) -> Result<Entity> {
    match node {
      xml::Node::Tag(tag) => {
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let type_registry = type_registry.read();
        create_entity_from_node(tag, world, &type_registry)
      }
      xml::Node::Text(text) => Ok(create_entity_from_text(text, world)),
    }
  }
}

fn create_entity_from_node(
  tag: &xml::Tag,
  world: &mut World,
  type_registry: &TypeRegistry,
) -> Result<Entity> {
  let name = tag.name.replace(".", "::");

  let registration = get_type_registration(&name, type_registry)?;
  let reflect_component = get_reflect_component(&name, registration)?;
  let mut reflect_val = registration
    .data::<ReflectDefault>()
    .ok_or_else(|| format!("Type {name} does not have ReflectDefault"))?
    .default();
  let struct_ref = reflect_val.reflect_mut().as_struct()?;

  let (fields, components): (Vec<_>, Vec<_>) =
    tag.attrs.iter().partition(|(k, _)| k.starts_with("self."));

  apply_map_to_struct(fields, struct_ref);

  let mut children = Vec::with_capacity(tag.children.len());

  for child in &tag.children {
    let child = Ui::spawn_node(world, child)?;
    children.push(child);
  }

  let mut entity = world.spawn_empty();

  reflect_component.insert(&mut entity, &*reflect_val, type_registry);
  for (name, value) in components {
    println!("creating extra component: {name}");
    let reg = get_type_registration(name, type_registry)?;
    println!("have registration");
    let ref_comp = get_reflect_component(name, reg)?;
    println!("have ref comp");
    let full_name = reg.type_info().type_path();
    let ref_ron = format!("{{ \"{full_name}\": {value} }}");
    println!("ref ron made");
    let value = deserialize_reflect(ref_ron, type_registry)?;
    println!("deserialized partial");
    let value = value
      .try_as_reflect()
      .ok_or_else(|| format!("Type {name} does not implement Reflect"))?;
    println!("deserialized full");
    ref_comp.insert(&mut entity, value, type_registry);
  }

  entity.add_children(&children);

  Ok(entity.id())
}

fn get_type_registration<'t>(
  name: &str,
  type_registry: &'t TypeRegistry,
) -> Result<&'t TypeRegistration> {
  let registration = type_registry
    .get_with_short_type_path(name)
    .or_else(|| type_registry.get_with_type_path(name))
    .ok_or_else(|| format!("Type {name} not registered"))?;

  Ok(registration)
}

fn get_reflect_component<'t>(
  name: &str,
  registration: &'t TypeRegistration,
) -> Result<&'t ReflectComponent> {
  let reflect_component = registration
    .data::<ReflectComponent>()
    .ok_or_else(|| format!("Type {name} does not have ReflectComponent"))?;

  Ok(reflect_component)
}

fn deserialize_reflect(
  ron: impl AsRef<str>,
  registry: &TypeRegistry,
) -> Result<Box<dyn PartialReflect>> {
  let de = ReflectDeserializer::new(registry);
  let mut rd = ron::Deserializer::from_str(ron.as_ref())?;
  let value = de.deserialize(&mut rd)?;

  Ok(value)
}

fn create_entity_from_text(text: &str, world: &mut World) -> Entity {
  world.spawn(Text::new(text.to_string())).id()
}

fn apply_map_to_struct<I, K, V>(iter: I, dyn_struct: &mut dyn Struct)
where
  K: AsRef<str>,
  V: AsRef<str>,
  I: IntoIterator<Item = (K, V)>,
{
  for (key, value_str) in iter {
    let key = key.as_ref();
    let value_str = value_str.as_ref();

    let Some(field) = dyn_struct.field_mut(key) else {
      continue;
    };

    let Some(type_info) = field.get_represented_type_info() else {
      continue;
    };

    let Some(parser_fn) = get_parser_fn(type_info) else {
      continue;
    };

    let new_val = (parser_fn)(value_str);

    if let Some(new_val) = new_val {
      field.apply(&*new_val);
    }
  }
}

fn get_parser_fn(type_info: &TypeInfo) -> Option<ParserFn> {
  let ty = type_info.ty();
  let type_name = ty.ident()?;

  #[rustfmt::skip]
  let f = get_parser!(type_name,
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
    f32, f64, bool, char, String
  );

  Some(f)
}

fn parse_to_reflect<T>(value: &str) -> Option<Box<dyn Reflect>>
where
  T: Reflect + FromStr,
{
  Some(Box::new(value.parse::<T>().ok()?) as Box<dyn Reflect>)
}

#[cfg(test)]
mod tests {
  use crate::Ui;
  use bevy::prelude::*;
  use speculoos::prelude::*;

  #[test]
  fn construct_type_by_xml() {
    const EXAMPLE_UI: &str = include_str!("../test/example_ui.xml");

    #[derive(Default, Component, Reflect)]
    #[reflect(Component)]
    #[reflect(Default)]
    struct Example {
      field1: i32,
      field2: String,
    }

    let mut world = World::default();
    {
      let app_type_registry = AppTypeRegistry::default();
      {
        let mut type_registry = app_type_registry.write();
        type_registry.register::<Example>();
      }
      world.insert_resource(app_type_registry);
    }

    let uis = Ui::parse_all(EXAMPLE_UI).unwrap();
    let ui = uis.first().unwrap();

    let entity = ui.spawn(&mut world).unwrap();

    let example_component = world.get::<Example>(entity).unwrap();

    assert_that(&example_component.field1).is_equal_to(123);
    assert_that(&example_component.field2.as_str()).is_equal_to("some text");
  }
}
