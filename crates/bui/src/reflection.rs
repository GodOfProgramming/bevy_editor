use crate::UiVTables;
use bevy::{
  prelude::*,
  reflect::{
    ReflectMut, ReflectRef, TypeInfo, TypeRegistration, TypeRegistry,
    serde::TypedReflectDeserializer,
  },
};
use serde::de::DeserializeSeed;
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

pub fn get_type_registration<'t>(
  name: &str,
  type_registry: &'t TypeRegistry,
) -> Result<&'t TypeRegistration> {
  let registration = type_registry
    .get_with_short_type_path(name)
    .or_else(|| type_registry.get_with_type_path(name))
    .ok_or_else(|| format!("Type {name} not registered"))?;

  Ok(registration)
}

pub fn deserialize_reflect(
  registry: &TypeRegistry,
  registration: &TypeRegistration,
  ron: impl AsRef<str>,
  vtables: &UiVTables,
) -> Result<Box<dyn Reflect>> {
  let de = TypedReflectDeserializer::new(registration, registry);
  let mut rd = ron::Deserializer::from_str(ron.as_ref())?;
  let value = de.deserialize(&mut rd)?;

  let reflect = value
  // first try base reflect casting
  .try_into_reflect()
  // then try getting the reflect from reflect component
  .or_else(|value: Box<dyn PartialReflect>| -> Result<Box<dyn Reflect>, Box<dyn PartialReflect>> {
      let reflect = registration.data::<ReflectFromReflect>().and_then(|rfr| {
      rfr.from_reflect(&*value)
    }).ok_or(value)?;
    Ok(reflect)
  })
  // if that fails, try the plugin registration
  .map_or_else(|value| {
    let vtable = vtables.reflection.get(&registration.type_id())?;
    let reflect = (vtable.from_reflect)(&*value)?;
    Some(reflect)
  }, Some)
  // if all else fails, this is an error
  .ok_or_else(|| {
    let tp = registration.type_info().type_path();
    format!(
      "Type {tp} does not implement Reflect correctly. Missing #[reflect(Serialize, Deserialize, FromReflect)] or registering with the plugin builder?"
    )
  })?;

  Ok(reflect)
}

pub fn patch_struct_with_map<I, K, V>(iter: I, reflect: &mut dyn Reflect) -> Result
where
  K: AsRef<str>,
  V: AsRef<str>,
  I: IntoIterator<Item = (K, V)>,
{
  let ref_mut = reflect.reflect_mut();

  match ref_mut.kind() {
    bevy::reflect::ReflectKind::Struct => {
      let dyn_struct = reflect.reflect_mut().as_struct()?;

      for (key, value) in iter {
        let key = key.as_ref();

        let Some(field) = dyn_struct.field_mut(key) else {
          return Err(format!("Unknown field name in struct {key}"))?;
        };

        patch_field(field, value)?;
      }
    }
    bevy::reflect::ReflectKind::TupleStruct => {
      let dyn_struct = reflect.reflect_mut().as_tuple_struct()?;

      for (key, value) in iter {
        let key = key.as_ref().parse::<usize>()?;

        let Some(field) = dyn_struct.field_mut(key) else {
          return Err(format!("Unknown field name in struct {key}"))?;
        };

        patch_field(field, value)?;
      }
    }
    k => Err(format!("Patching unsupported for type {k:?}"))?,
  }

  Ok(())
}

fn patch_field(field: &mut dyn PartialReflect, value: impl AsRef<str>) -> Result {
  let Some(type_info) = field.get_represented_type_info() else {
    return Err("Unable to get type info of field")?;
  };

  let Some(parser_fn) = get_parser_fn(type_info) else {
    let tp = type_info.type_path();
    return Err(format!("Unsupported field type in struct {tp}"))?;
  };

  let new_val = (parser_fn)(value.as_ref());

  if let Some(new_val) = new_val {
    field.apply(&*new_val);
  }

  Ok(())
}

pub fn patch_reflect<A: Reflect, B: Reflect>(patch: &A, target: &mut B) -> usize {
  let mut patches = 0;

  match (patch.reflect_ref(), target.reflect_mut()) {
    (ReflectRef::Struct(patch_struct), ReflectMut::Struct(target_struct)) => {
      for i in 0..patch_struct.field_len() {
        let field_name = patch_struct.name_at(i).unwrap();
        let patch_field = patch_struct.field_at(i).unwrap();

        if let Some(inner) = patch_field.try_as_reflect() {
          if let Some(target_field) = target_struct.field_mut(field_name) {
            target_field.apply(inner);
            patches += 1;
          }
        }
      }
    }
    (ReflectRef::TupleStruct(patch_struct), ReflectMut::TupleStruct(target_struct)) => {
      for i in 0..patch_struct.field_len() {
        let patch_field = patch_struct.field(i).unwrap();

        if let Some(inner) = patch_field.try_as_reflect() {
          if let Some(target_field) = target_struct.field_mut(i) {
            target_field.apply(inner);
            patches += 1;
          }
        }
      }
    }
    _ => {}
  }

  patches
}

fn parse_to_reflect<T>(value: &str) -> Option<Box<dyn Reflect>>
where
  T: Reflect + FromStr,
{
  Some(Box::new(value.parse::<T>().ok()?) as Box<dyn Reflect>)
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
