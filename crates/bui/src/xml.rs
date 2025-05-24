use crate::{
  EVENT_PREFIX, Overrides, PrimaryType, SELF_PREFIX,
  reflection::{self, TypeRegistryExt},
  result_string, serialize_name,
};
use bevy::{
  ecs::component::ComponentId, platform::collections::HashSet, prelude::*, reflect::TypeRegistry,
  utils::TypeIdMap,
};
use core::cmp::Ordering;
use std::{
  any::TypeId,
  borrow::Cow,
  collections::BTreeMap,
  fmt::Display,
  hash::Hash,
  io::{BufReader, BufWriter},
  iter,
  ops::Index,
};
use thiserror::Error;
use xml::{
  EventReader,
  attribute::{Attribute, OwnedAttribute},
  name::Name,
  namespace::Namespace,
  reader::XmlEvent as RXmlEvent,
  writer::XmlEvent as WXmlEvent,
};

#[derive(Debug, Error)]
pub enum ParseError {
  #[error("Expected tag to exist")]
  ExpectedTag,

  #[error("Expected node to exist")]
  ExpectedNode,

  #[error("{0}")]
  General(Box<dyn std::error::Error>),
}

impl From<Box<dyn std::error::Error>> for ParseError {
  fn from(value: Box<dyn std::error::Error>) -> Self {
    Self::General(value)
  }
}

#[derive(Deref, DerefMut)]
pub struct Nodes(Vec<Node>);

impl TryFrom<&str> for Nodes {
  type Error = ParseError;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    parse(value).map(Self)
  }
}

#[derive(Debug)]
pub enum Node {
  Tag(Tag),
  Text(String),
}

impl Node {
  pub(crate) fn serialize(
    entity: Entity,
    world: &World,
    blacklist: Option<&HashSet<TypeId>>,
    overrides: Option<&TypeIdMap<Overrides>>,
  ) -> Result<Self> {
    let app_type_registry = world
      .get_resource::<AppTypeRegistry>()
      .ok_or("AppTypeRegistry not found")?;
    let type_registry = app_type_registry.read();

    let entity_ref = world.entity(entity);

    let primary_type = entity_ref
      .get_components::<&PrimaryType>()
      .ok_or_else(|| format!("Entity {entity:?} does not have a PrimaryType component"))?;

    let primary_type_id = primary_type.type_id();

    let mut reflect = reflection::reflect_component(&entity_ref, primary_type_id, &type_registry)?;

    let reflect_override = overrides
      .and_then(|overrides| {
        overrides
          .get(&primary_type_id)
          .map(|f| (f.el)(reflect, world))
      })
      .transpose()?;

    if let Some(reflect_override) = &reflect_override {
      reflect = &**reflect_override;
    }

    let num_components = entity_ref.archetype().component_count();

    let node = match (reflect.downcast_ref::<Text>(), num_components) {
      (Some(text), 1) => Node::Text(text.0.clone()),
      _ => {
        let tag = Tag::from_entity(
          reflect,
          entity_ref,
          &type_registry,
          world,
          blacklist,
          overrides,
        )?;
        Node::Tag(tag)
      }
    };

    Ok(node)
  }
}

impl TryInto<String> for &Node {
  type Error = BevyError;
  fn try_into(self) -> Result<String> {
    let events: Vec<WXmlEvent> = self.into();
    let writer = BufWriter::new(Vec::new());
    let config = xml::EmitterConfig::default().write_document_declaration(false);
    let mut writer = xml::EventWriter::new_with_config(writer, config);

    for event in events {
      writer.write(event)?;
    }

    let inner = writer.into_inner().into_inner()?;

    let out = String::from_utf8(inner)?;

    Ok(out)
  }
}

impl<'n> From<&'n Node> for Vec<WXmlEvent<'n>> {
  fn from(val: &'n Node) -> Self {
    let mut events = Vec::new();

    match val {
      Node::Tag(tag) => {
        let child_events: Vec<WXmlEvent> = tag.into();
        events.extend(child_events);
      }
      Node::Text(text) => {
        events.push(WXmlEvent::Characters(text));
      }
    }

    events
  }
}

impl From<&str> for Node {
  fn from(value: &str) -> Self {
    value.to_string().into()
  }
}

impl From<String> for Node {
  fn from(value: String) -> Self {
    Self::Text(value)
  }
}

#[derive(Default, Debug)]
pub struct Tag {
  name: String,
  attrs: BTreeMap<Attr, String>,
  children: Vec<Node>,
}

impl Tag {
  fn new(name: impl ToString, attrs: impl IntoIterator<Item = OwnedAttribute>) -> Self {
    Self {
      name: name.to_string(),
      attrs: attrs
        .into_iter()
        .map(|attr| {
          (
            Attr {
              prefix: attr.name.prefix,
              name: attr.name.local_name,
            },
            attr.value,
          )
        })
        .collect(),
      children: Vec::new(),
    }
  }

  fn from_entity(
    base: &dyn Reflect,
    entity: EntityRef,
    type_registry: &TypeRegistry,
    world: &World,
    blacklist: Option<&HashSet<TypeId>>,
    overrides: Option<&TypeIdMap<Overrides>>,
  ) -> Result<Self> {
    let base_type_info = base
      .get_represented_type_info()
      .ok_or("Base Component has no TypeInfo")?;
    let base_type_id = base_type_info.type_id();
    let base_type_name = base_type_info.type_path();

    let base_comp_id = world.components().get_id(base_type_id).ok_or_else(|| {
      let tp = type_registry.type_name_of(base_type_id);
      let tp = result_string(&tp);
      format!("Type {tp} has no component id")
    })?;

    let mut attrs = Self::serialize_attrs(
      &entity,
      world,
      type_registry,
      base_comp_id,
      blacklist,
      overrides,
    )?;
    attrs.insert(
      Attr::named("self"),
      reflection::serialize_reflect(base, type_registry)?,
    );

    let children = Self::serialize_children(&entity, world, blacklist, overrides)
      .transpose()?
      .unwrap_or_default();

    Ok(Self {
      name: serialize_name(base_type_name),
      attrs,
      children,
    })
  }

  fn serialize_attrs(
    entity: &EntityRef,
    world: &World,
    type_registry: &TypeRegistry,
    base_component_id: ComponentId,
    blacklist: Option<&HashSet<TypeId>>,
    overrides: Option<&TypeIdMap<Overrides>>,
  ) -> Result<BTreeMap<Attr, String>> {
    let components = world.components();

    entity
      .archetype()
      .components()
      .filter(|comp| *comp != base_component_id)
      .filter_map(|comp| {
        let component_info = components.get_info(comp)?;
        component_info
          .type_id()
          .map(|type_id| (component_info, type_id))
      })
      .filter_map(|(component_info, type_id)| {
        blacklist
          .map(|blacklist| (!blacklist.contains(&type_id)).then_some((component_info, type_id)))
          .unwrap_or(Some((component_info, type_id)))
      })
      .map(|(component_info, component_type_id)| {
        let mut reflect = reflection::reflect_component(entity, component_type_id, type_registry)
          .map_err(|err| format!("{err}: {}", component_info.name()))?;

        let (attr_override, reflect_override) = overrides
          .and_then(|overrides| {
            overrides
              .get(&component_type_id)
              .map(|f| (f.attr)(reflect, world))
          })
          .transpose()?
          .unzip();

        if let Some(reflect_override) = &reflect_override {
          reflect = &**reflect_override;
        }

        let attr = attr_override.flatten().map(Ok).unwrap_or_else(|| {
          let ti = reflect.reflect_type_info();
          let type_id = ti.type_id();
          Attr::from_type(type_id, type_registry).map_err(|err| {
            let tp = ti.type_path();
            format!("{err}: {}", tp)
          })
        })?;

        let value = reflection::serialize_reflect(reflect, type_registry)?;

        Ok((attr, value))
      })
      .collect()
  }

  fn serialize_children(
    entity: &EntityRef,
    world: &World,
    blacklist: Option<&HashSet<TypeId>>,
    overrides: Option<&TypeIdMap<Overrides>>,
  ) -> Option<Result<Vec<Node>>> {
    entity.get::<Children>().map(|children| {
      children
        .iter()
        .map(|child| Node::serialize(child, world, blacklist, overrides))
        .collect()
    })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn attr(&self, attr: impl Into<Attr>) -> Option<&str> {
    self.attrs.get(&attr.into()).map(String::as_str)
  }

  pub fn children(&self) -> &Vec<Node> {
    &self.children
  }

  pub fn attr_iter(&self) -> impl Iterator<Item = (&Attr, &str)> {
    self.attrs.iter().map(|(k, v)| (k, v.as_str()))
  }
}

impl<'n> From<&'n Tag> for Vec<WXmlEvent<'n>> {
  fn from(val: &'n Tag) -> Self {
    let attrs = val
      .attrs
      .iter()
      .map(|(k, v)| {
        Attribute::new(
          k.prefix
            .as_ref()
            .map(|prefix| Name::prefixed(&k.name, prefix))
            .unwrap_or_else(|| Name::local(&k.name)),
          v,
        )
      })
      .collect::<Vec<Attribute>>();

    let attrs = Cow::Owned(attrs);
    let ns = Cow::Owned(Namespace::empty());

    let start = WXmlEvent::StartElement {
      name: Name::local(&val.name),
      attributes: attrs,
      namespace: ns,
    };

    let num_children = val.children.len();

    let child_events = val
      .children
      .iter()
      .flat_map(<&Node as Into<Vec<WXmlEvent>>>::into);

    let end = WXmlEvent::EndElement {
      name: (num_children > 0).then_some(Name::local(&val.name)),
    };

    iter::once(start)
      .chain(child_events)
      .chain(iter::once(end))
      .collect()
  }
}

impl Index<usize> for Tag {
  type Output = Node;
  fn index(&self, index: usize) -> &Self::Output {
    &self.children[index]
  }
}

impl Index<&Attr> for Tag {
  type Output = str;
  fn index(&self, index: &Attr) -> &Self::Output {
    &self.attrs[index]
  }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Attr {
  prefix: Option<String>,
  name: String,
}

impl Attr {
  fn from_type(type_id: TypeId, type_registry: &TypeRegistry) -> Result<Self> {
    let type_info = type_registry
      .get_type_info(type_id)
      .ok_or("Attempted to make Attr from unregistered type")?;

    Ok(Self {
      prefix: None,
      name: serialize_name(type_info.type_path()),
    })
  }

  pub fn named(name: impl ToString) -> Self {
    Self {
      prefix: None,
      name: name.to_string(),
    }
  }

  pub fn prefixed(prefix: impl ToString, name: impl ToString) -> Self {
    Self {
      prefix: Some(prefix.to_string()),
      name: name.to_string(),
    }
  }

  pub fn with_prefix(mut self, prefix: Option<impl ToString>) -> Self {
    self.prefix = prefix.map(|ts| ts.to_string());
    self
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn prefix(&self) -> Option<&str> {
    self.prefix.as_deref()
  }
}

impl Display for Attr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(prefix) = &self.prefix {
      write!(f, "{}:", prefix)?;
    }
    write!(f, "{}", self.name)
  }
}

impl PartialOrd for Attr {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Attr {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self.name.as_ref(), other.name.as_ref()) {
      ("self", "self") => self.prefix.cmp(&other.prefix),
      ("self", _) => Ordering::Less,
      (_, "self") => Ordering::Greater,
      _ => match self.prefix.cmp(&other.prefix) {
        Ordering::Equal => self.name.cmp(&other.name),
        ord => ord,
      },
    }
  }
}

impl Hash for Attr {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.prefix.hash(state);
    self.name.hash(state);
  }
}

pub fn parse(data: &str) -> Result<Vec<Node>, ParseError> {
  const NAMESPACE_ELEMENT: &str = "BUI...NAMESPACE";

  // inject namespaces into the overall doc
  // xml-rs doesn't allow for ignoring namespace decls
  let data = format!(
    "<{NAMESPACE_ELEMENT}
      xmlns:{EVENT_PREFIX}='{EVENT_PREFIX}'
      xmlns:{SELF_PREFIX}='{SELF_PREFIX}'>
        {data}
      </{NAMESPACE_ELEMENT}>"
  );

  let reader = BufReader::new(data.as_bytes());
  let parser = EventReader::new(reader);

  let mut roots = Vec::new();
  let mut stack = Vec::new();

  for event in parser {
    match event {
      Ok(RXmlEvent::StartElement {
        name, attributes, ..
      }) => {
        // ignore injected namespace
        if name.local_name == NAMESPACE_ELEMENT {
          continue;
        }

        let tag = Tag::new(name, attributes);
        stack.push(Node::Tag(tag));
      }
      Ok(RXmlEvent::EndElement { name }) => {
        // ignore injected namespace
        if name.local_name == NAMESPACE_ELEMENT {
          continue;
        }

        if stack.is_empty() {
          return Err(ParseError::ExpectedTag);
        }

        if stack.len() == 1 {
          let Some(node) = stack.pop() else {
            return Err(ParseError::ExpectedTag);
          };

          roots.push(node);
        } else {
          let Some(node) = stack.pop() else {
            return Err(ParseError::ExpectedTag);
          };

          let Some(Node::Tag(tag)) = stack.last_mut() else {
            return Err(ParseError::ExpectedTag);
          };

          tag.children.push(node);
        }
      }
      Ok(RXmlEvent::Characters(text) | RXmlEvent::CData(text)) => {
        let Some(Node::Tag(tag)) = stack.last_mut() else {
          return Err(ParseError::ExpectedTag);
        };

        tag.children.push(text.trim().into());
      }
      Err(e) => return Err(ParseError::General(Box::new(e))),
      _ => (),
    }
  }

  Ok(roots)
}

#[cfg(test)]
mod tests {
  use std::ops::Index;

  use super::{Attr, Node, Tag};
  use speculoos::prelude::*;

  impl Index<&str> for Tag {
    type Output = str;
    fn index(&self, index: &str) -> &Self::Output {
      let mut split = index.split(':');
      let prefix_or_name = split.next();
      let maybe_name = split.next();

      match (prefix_or_name, maybe_name) {
        (Some(name), None) => {
          let attr = Attr::named(name);
          &self[&attr]
        }
        (Some(prefix), Some(name)) => {
          let attr = Attr::prefixed(prefix, name);
          &self[&attr]
        }
        _ => unimplemented!(),
      }
    }
  }

  #[test]
  fn parse_dummy_data() {
    const DUMMY_DATA: &str = include_str!("../test/dummy_data.xml");

    let nodes = super::parse(DUMMY_DATA).unwrap();

    assert_that(&nodes.len()).is_equal_to(1);

    let Node::Tag(pane) = nodes.first().unwrap() else {
      panic!("Expected pane tag to be first")
    };

    assert_that(&pane.name.as_str()).is_equal_to("Pane");

    let (Node::Text(text1), Node::Tag(button), Node::Tag(rich_text), Node::Text(text2)) =
      (&pane[0], &pane[1], &pane[2], &pane[3])
    else {
      panic!(
        "Unexpected number of child elements: {}",
        pane.children.len()
      );
    };

    assert_that(&text1.as_str()).is_equal_to("Example Text 1");

    let button_on_click = &button["OnClick"];
    let Node::Text(button_text) = &button[0] else {
      panic!("Expected button text");
    };
    assert_that(&button.name.as_str()).is_equal_to("Button");
    assert_that(&button_on_click).is_equal_to("SomeEvent");
    assert_that(&button_text.as_str()).is_equal_to("Button Example");

    let Node::Text(rich_text_text) = &rich_text[0] else {
      panic!("Expected nested text");
    };
    assert_that(&rich_text.name.as_str()).is_equal_to("RichText");
    assert_that(&rich_text_text.as_str()).is_equal_to("Some Unicode = \u{E596}");

    assert_that(&text2.as_str()).is_equal_to("Example Text 2");
  }
}
