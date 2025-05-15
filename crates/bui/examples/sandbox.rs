use std::fmt::Display;

use bevy::{
  prelude::*,
  reflect::{
    GetTypeRegistration, Reflect, Reflectable, TypePath, TypeRegistry,
    serde::{ReflectDeserializer, ReflectSerializer},
  },
};
use serde::de::DeserializeSeed;
use std::fmt::Debug;

fn main() {
  reflect_test();
}

fn write_xml() {
  const DUMMY_DATA: &str = include_str!("../test/dummy_data.xml");

  let nodes = bui::xml::parse(DUMMY_DATA).unwrap();

  let first = &nodes[0];

  let events: Vec<xml::writer::XmlEvent> = first.into();

  let mut writer = xml::EmitterConfig::new()
    .write_document_declaration(false)
    .perform_indent(true)
    .indent_string("\t")
    .create_writer(std::io::stdout());

  for event in events {
    writer.write(event).unwrap();
  }
}

fn reflect_test() {
  // let input = Node {
  //   width: Val::Px(150.0),
  //   height: Val::Px(150.0),
  //   ..default()
  // };

  let input = BackgroundColor(Color::linear_rgb(1.0, 0.0, 0.0));

  make_reflect(input);
}

fn make_reflect<T: Reflectable + Debug>(value: T) {
  let mut registry = TypeRegistry::default();
  registry.register::<T>();

  let ser = ReflectSerializer::new(&value, &registry);
  let out = ron::to_string(&ser).unwrap();

  println!("{out}");

  print_val::<T>(&out, &registry);
}

fn print_val<T: Reflectable + Debug>(ron_str: &str, registry: &TypeRegistry) {
  let de = ReflectDeserializer::new(registry);
  let mut rd = ron::Deserializer::from_str(ron_str).unwrap();

  let out = de.deserialize(&mut rd).unwrap();
  println!("partial ref is type => {}", out.represents::<T>());

  let out = out.try_as_reflect().unwrap();
  println!("full ref is type => {}", out.is::<T>());

  let out = out.downcast_ref::<T>().unwrap();
  println!("val => {out:?}");
}
