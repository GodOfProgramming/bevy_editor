use bevy::{
  prelude::*,
  reflect::{TypeRegistry, serde::TypedReflectSerializer},
};

fn main() -> Result {
  let tr = TypeRegistry::new();
  let i = String::from("foo bar");

  let ser = TypedReflectSerializer::new(&i, &tr);
  let str = ron::to_string(&ser)?;

  println!("i => {str}");

  Ok(())
}
