mod xml;

use bevy::{
  prelude::*,
  reflect::{TypeInfo, TypeRegistry},
};
use std::str::FromStr;

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

pub struct Ui {
  node: xml::Node,
}

impl Ui {
  pub fn parse_all(ui_xml: &str) -> Result<Vec<Ui>, xml::ParseError> {
    xml::parse(ui_xml).map(|nodes| nodes.into_iter().map(|node| Ui { node }).collect())
  }

  pub fn create(&self, world: &mut World) -> Entity {
    match &self.node {
      xml::Node::Tag(tag) => {
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let type_registry = type_registry.read();
        create_entity_from_node(tag, world, &type_registry)
      }
      xml::Node::Text(text) => create_entity_from_text(text, world),
    }
  }
}

fn create_entity_from_node(
  tag: &xml::Tag,
  world: &mut World,
  type_registry: &TypeRegistry,
) -> Entity {
  let registration = type_registry.get_with_short_type_path(&tag.name).unwrap();
  let reflect_component = registration.data::<ReflectComponent>().unwrap();
  let mut reflect_val = registration.data::<ReflectDefault>().unwrap().default();

  let struct_ref = reflect_val.reflect_mut().as_struct().unwrap();

  apply_map_to_struct(&tag.attrs, struct_ref);

  let mut entity = world.spawn_empty();
  reflect_component.insert(&mut entity, &*reflect_val, type_registry);
  entity.id()
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

    let entity = ui.create(&mut world);

    let example_component = world.get::<Example>(entity).unwrap();

    assert_that(&example_component.field1).is_equal_to(123);
    assert_that(&example_component.field2.as_str()).is_equal_to("some text");
  }
}
