use std::fmt::Display;

use bevy::reflect::{
  Reflect, TypePath, TypeRegistry,
  serde::{ReflectDeserializer, ReflectSerializer},
};
use serde::de::DeserializeSeed;

fn main() {
  write_xml();
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
  let mut registry = TypeRegistry::default();

  registry.register::<i32>();

  let input: String = String::from("foobar");

  let ser = ReflectSerializer::new(&input, &registry);
  let out = ron::to_string(&ser).unwrap();
  println!("{out}");

  print_val::<String>(&out, &registry);
}

fn print_val<T: Reflect + TypePath + Display>(r: &str, registry: &TypeRegistry) {
  let de = ReflectDeserializer::new(registry);
  let mut rd = ron::Deserializer::from_str(r).unwrap();
  let out = de.deserialize(&mut rd).unwrap();
  println!("is i32 => {}", out.represents::<T>());
  let output = out.try_downcast::<T>().unwrap();
  println!("val => {output}");
}
