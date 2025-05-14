pub mod xml;
use bevy::prelude::*;

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

    let ty = type_info.ty();
    let Some(type_name) = ty.ident() else {
      continue;
    };

    let new_val: Option<Box<dyn Reflect>> = match type_name {
      "i32" => value_str
        .parse::<i32>()
        .ok()
        .map(|v| Box::new(v) as Box<dyn Reflect>),
      "f32" => value_str
        .parse::<f32>()
        .ok()
        .map(|v| Box::new(v) as Box<dyn Reflect>),
      "bool" => value_str
        .parse::<bool>()
        .ok()
        .map(|v| Box::new(v) as Box<dyn Reflect>),
      "String" => Some(Box::new(value_str.to_string()) as Box<dyn Reflect>),
      _ => None,
    };

    if let Some(new_val) = new_val {
      field.apply(&*new_val);
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::{apply_map_to_struct, xml::XmlNode};
  use bevy::{prelude::*, reflect::TypeRegistry};
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

    let mut tr = TypeRegistry::default();
    let nodes = super::xml::parse(EXAMPLE_UI).unwrap();

    let node = nodes.first().unwrap();

    let XmlNode::Tag(tag) = node else {
      panic!("Expected xml tag");
    };

    tr.register::<Example>();

    let mut world = World::default();

    let registration = tr.get_with_short_type_path(&tag.name).unwrap();
    let reflect_component = registration.data::<ReflectComponent>().unwrap();
    let mut reflect_val = registration.data::<ReflectDefault>().unwrap().default();

    let struct_ref = reflect_val.reflect_mut().as_struct().unwrap();

    apply_map_to_struct(&tag.attrs, struct_ref);

    let entity = {
      let mut entity = world.spawn_empty();

      reflect_component.insert(&mut entity, &*reflect_val, &tr);
      entity.id()
    };

    let example_component = world.get::<Example>(entity).unwrap();

    assert_that(&example_component.field1).is_equal_to(123);
    assert_that(&example_component.field2.as_str()).is_equal_to("some text");
  }
}
