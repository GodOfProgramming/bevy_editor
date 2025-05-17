pub mod ui;
pub mod xml;

use bevy::{
  ecs::system::SystemId,
  prelude::*,
  reflect::{
    GetTypeRegistration, ReflectMut, ReflectRef, TypeInfo, TypeRegistration, TypeRegistry,
    serde::ReflectDeserializer,
  },
  utils::TypeIdMap,
};
use itertools::{Either, Itertools};
use serde::de::DeserializeSeed;
use std::{any::TypeId, str::FromStr};
use ui::{Attribute, Element, attrs::Style, elements::UiButton};

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

pub struct BuiPlugin {
  vtables: UiVTables,
}

impl Default for BuiPlugin {
  fn default() -> Self {
    Self { vtables: default() }
      .register_element::<UiButton>()
      .register_attr::<Style>()
  }
}

impl BuiPlugin {
  pub fn register_element<E: Element>(mut self) -> Self {
    self.vtables.elements.insert(
      TypeId::of::<E>(),
      ElementVTable {
        register: |app| {
          app.register_type::<E>();
        },
      },
    );

    self
  }

  pub fn register_attr<A: Attribute>(mut self) -> Self {
    self.vtables.attrs.insert(
      TypeId::of::<A>(),
      AttrVTable {
        register: |app| {
          app.register_type::<A>();
        },
        create: |world, entity, value| {
          let Some(value) = value.downcast_ref::<A>() else {
            return;
          };

          let entity = world.entity_mut(entity);

          value.insert_into(entity);
        },
      },
    );

    self
  }

  pub fn register_event<E: UiEvent>(mut self) -> Self {
    self.vtables.events.insert(
      TypeId::of::<E>(),
      EventVTable {
        register: |app, events| {
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

    for vtable in self.vtables.elements.values() {
      (vtable.register)(app);
    }

    for vtable in self.vtables.attrs.values() {
      (vtable.register)(app);
    }

    for vtable in self.vtables.events.values() {
      (vtable.register)(app, &mut events);
    }
  }
}

#[derive(Default, Resource, Clone)]
struct UiVTables {
  elements: TypeIdMap<ElementVTable>,
  attrs: TypeIdMap<AttrVTable>,
  events: TypeIdMap<EventVTable>,
}

#[derive(Clone)]
struct ElementVTable {
  register: fn(&mut App),
}

#[derive(Clone)]
struct AttrVTable {
  register: fn(&mut App),
  create: fn(world: &mut World, entity: Entity, value: &dyn Reflect),
}

#[derive(Clone)]
struct EventVTable {
  register: fn(&mut App, &mut UiEvents),
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
  // replace . with :: so full path lookup works
  let name = tag.name.replace(".", "::");

  let registration = get_type_registration(&name, type_registry)?;
  let reflect_component = registration
    .data::<ReflectComponent>()
    .ok_or_else(|| format!("Type {name} does not have ReflectComponent"))?;
  let mut reflect_val = registration
    .data::<ReflectDefault>()
    .ok_or_else(|| format!("Type {name} does not have ReflectDefault"))?
    .default();

  let struct_ref = reflect_val.reflect_mut().as_struct()?;
  let (fields, components): (Vec<_>, Vec<_>) = tag.attrs.iter().partition_map(|(k, v)| {
    k.strip_prefix("self.")
      .map(|n| Either::Left((n, v)))
      .unwrap_or(itertools::Either::Right((k, v)))
  });
  patch_struct_with_map(fields, struct_ref);

  // create children first, as they need the world
  let children = create_child_entities(&tag.children, world)?;

  // then the actual entity for this element
  let mut entity = world.spawn_empty();

  // add the reflected value to this entity
  reflect_component.insert(&mut entity, &*reflect_val, type_registry);

  // then add all children
  entity.add_children(&children);

  let entity = entity.id();

  // the world is free again and now the attributes can be created
  for (name, value) in components {
    insert_attribute(name, value, world, entity, type_registry)?;
  }

  Ok(entity)
}

fn create_child_entities<'c>(
  child_elements: impl IntoIterator<Item = &'c xml::Node>,
  world: &mut World,
) -> Result<Vec<Entity>> {
  let mut children = Vec::new();

  for child in child_elements {
    let child = Ui::spawn_node(world, child)?;
    children.push(child);
  }

  Ok(children)
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

fn insert_attribute(
  name: &str,
  value: &str,
  world: &mut World,
  entity: Entity,
  type_registry: &TypeRegistry,
) -> Result {
  let reg = get_type_registration(name, type_registry)?;

  let full_name = reg.type_info().type_path();
  let ref_ron = format!("{{ \"{full_name}\": {value} }}");

  let partial_value = deserialize_reflect(ref_ron, type_registry)?;
  let ref_value = partial_value
    .try_as_reflect()
    .ok_or_else(|| format!("Type {full_name} does not implement Reflect"))?;

  world.resource_scope(|world, vtables: Mut<UiVTables>| {
    let Some(fns) = vtables.attrs.get(&reg.type_id()) else {
      error!("Type {name} was not registered as an attribute");
      return;
    };

    (fns.create)(world, entity, ref_value);
  });

  Ok(())
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

fn patch_struct_with_map<I, K, V>(iter: I, dyn_struct: &mut dyn Struct)
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

fn patch_reflect<A: Reflect, B: Reflect>(patch: &A, target: &mut B) {
  if let (ReflectRef::Struct(patch_struct), ReflectMut::Struct(target_struct)) =
    (patch.reflect_ref(), target.reflect_mut())
  {
    for i in 0..patch_struct.field_len() {
      let field_name = patch_struct.name_at(i).unwrap();
      let patch_field = patch_struct.field_at(i).unwrap();

      if let Some(inner) = patch_field.try_as_reflect() {
        if let Some(target_field) = target_struct.field_mut(field_name) {
          target_field.apply(inner);
        }
      }
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

fn char_filter<I, K, V>(iter: I, filter_fn: fn(&char) -> bool) -> impl Iterator<Item = (K, V)>
where
  K: AsRef<str>,
  V: AsRef<str>,
  I: IntoIterator<Item = (K, V)>,
{
  iter.into_iter().filter(move |(k, _)| {
    k.as_ref()
      .chars()
      .next()
      .as_ref()
      .map(filter_fn)
      .unwrap_or(false)
  })
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
