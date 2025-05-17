use bevy::{
  prelude::*,
  reflect::{
    GetTypeRegistration, TypeRegistry,
    serde::{TypedReflectDeserializer, TypedReflectSerializer},
  },
};
use serde::{Serialize, de::DeserializeSeed};

fn main() {
  let text = Text(String::from("test"));

  let registry = TypeRegistry::new();
  let reg = Text::get_type_registration();

  let se = TypedReflectSerializer::new(&text, &registry);

  let mut writer = std::io::Cursor::new(Vec::new());
  let mut rs = ron::Serializer::new(&mut writer, None).unwrap();
  se.serialize(&mut rs).unwrap();

  let data = writer.into_inner();
  let ron = String::from_utf8(data).unwrap();

  println!("{ron}");

  let de = TypedReflectDeserializer::new(&reg, &registry);
  let mut rd = ron::Deserializer::from_str(ron.as_ref()).unwrap();
  let value = de.deserialize(&mut rd).unwrap();

  let value = Text::from_reflect(&*value);

  println!("{value:?}");
}
