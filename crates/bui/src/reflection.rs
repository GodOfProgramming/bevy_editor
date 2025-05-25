use std::any::TypeId;

use crate::{UiVTables, result_string};
use bevy::{
  prelude::*,
  reflect::{
    ReflectMut, ReflectRef, TypeRegistration, TypeRegistry,
    serde::{TypedReflectDeserializer, TypedReflectSerializer},
  },
};
use serde::de::DeserializeSeed;

#[derive(thiserror::Error, Debug)]
pub enum ReflectionError {
  #[error("Type {0} was not registered in the TypeRegistry")]
  UnregisteredType(String),

  #[error("Type {0} does not have {1}")]
  MissingTypeData(String, String),

  #[error("Could not downcast {0} to {1}")]
  InvalidCast(String, String),
}

impl ReflectionError {
  pub fn unregistered_type(t: impl Into<String>) -> Self {
    Self::UnregisteredType(t.into())
  }

  pub fn missing_type_data(t: impl Into<String>, data: impl Into<String>) -> Self {
    Self::MissingTypeData(t.into(), data.into())
  }

  pub fn invalid_cast(from: impl Into<String>, to: impl Into<String>) -> Self {
    Self::InvalidCast(from.into(), to.into())
  }
}

pub trait TypeRegistryExt {
  fn type_name_of(&self, type_id: TypeId) -> Result<&'static str, String>;
}

impl TypeRegistryExt for TypeRegistry {
  fn type_name_of(&self, type_id: TypeId) -> Result<&'static str, String> {
    self
      .get(type_id)
      .map(|r| r.type_info().type_path())
      .ok_or_else(|| format!("{type_id:?}"))
  }
}

pub fn get_type_registration_from_name<'t>(
  name: &str,
  type_registry: &'t TypeRegistry,
) -> Result<&'t TypeRegistration> {
  let registration = type_registry
    .get_with_short_type_path(name)
    .or_else(|| type_registry.get_with_type_path(name))
    .ok_or_else(|| ReflectionError::unregistered_type(name))?;

  Ok(registration)
}

pub fn serialize_reflect(reflect: &dyn PartialReflect, registry: &TypeRegistry) -> Result<String> {
  let ser = TypedReflectSerializer::new(reflect, registry);
  let out = ron::to_string(&ser)?;
  Ok(out)
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

pub fn patch_struct_with_map<I, K, V>(
  iter: I,
  reflect: &mut dyn Reflect,
  type_registry: &TypeRegistry,
) -> Result
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

        patch_field(field, value, type_registry)?;
      }
    }
    bevy::reflect::ReflectKind::TupleStruct => {
      let dyn_struct = reflect.reflect_mut().as_tuple_struct()?;

      for (key, value) in iter {
        let key = key.as_ref().parse::<usize>()?;

        let Some(field) = dyn_struct.field_mut(key) else {
          return Err(format!("Unknown field name in struct {key}"))?;
        };

        patch_field(field, value, type_registry)?;
      }
    }
    k => Err(format!("Patching unsupported for type {k:?}"))?,
  }

  Ok(())
}

fn patch_field(
  field: &mut dyn PartialReflect,
  value: impl AsRef<str>,
  type_registry: &TypeRegistry,
) -> Result {
  let Some(type_info) = field.get_represented_type_info() else {
    return Err("Unable to get type info of field")?;
  };

  let Some(registration) = type_registry.get(type_info.type_id()) else {
    return Err("Unable to acquire type info of field")?;
  };

  let de = TypedReflectDeserializer::new(registration, type_registry);
  let mut rd = ron::Deserializer::from_str(value.as_ref())?;
  let reflect = de.deserialize(&mut rd)?;

  field.apply(&*reflect);

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

pub fn reflect_component<'r>(
  entity: &'r EntityRef,
  type_id: TypeId,
  type_registry: &TypeRegistry,
) -> Result<&'r dyn Reflect> {
  let ref_comp = type_registry
    .get_type_data::<ReflectComponent>(type_id)
    .ok_or_else(|| {
      let tp = type_registry.type_name_of(type_id);
      let tp = result_string(&tp);
      format!("Type {tp} does not have ReflectComponent")
    })?;

  let reflect = ref_comp.reflect(entity).ok_or_else(|| {
    let tp = type_registry.type_name_of(type_id);
    let tp = result_string(&tp);
    format!("Type {tp} was not reflectable")
  })?;

  Ok(reflect)
}
