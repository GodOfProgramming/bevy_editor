use bevy::prelude::{Deref, DerefMut};
use std::{borrow::Cow, collections::BTreeMap, io::BufReader, iter, ops::Index};
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
  General(xml::reader::Error),
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
  pub name: String,
  pub attrs: BTreeMap<String, String>,
  pub children: Vec<Node>,
}

impl Tag {
  fn new(name: impl ToString, attrs: impl IntoIterator<Item = OwnedAttribute>) -> Self {
    Self {
      name: name.to_string(),
      attrs: attrs
        .into_iter()
        .map(|attr| (attr.name.to_string(), attr.value))
        .collect(),
      children: Vec::new(),
    }
  }
}

impl<'n> From<&'n Tag> for Vec<WXmlEvent<'n>> {
  fn from(val: &'n Tag) -> Self {
    let attrs = val
      .attrs
      .iter()
      .map(|(k, v)| Attribute::new(Name::local(k), v))
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

impl Index<&str> for Tag {
  type Output = str;
  fn index(&self, index: &str) -> &Self::Output {
    &self.attrs[index]
  }
}

pub fn parse(data: &str) -> Result<Vec<Node>, ParseError> {
  let reader = BufReader::new(data.as_bytes());
  let parser = EventReader::new(reader);

  let mut roots = Vec::new();
  let mut stack = Vec::new();

  for event in parser {
    match event {
      Ok(RXmlEvent::StartElement {
        name, attributes, ..
      }) => {
        let tag = Tag::new(name, attributes);
        stack.push(Node::Tag(tag));
      }
      Ok(RXmlEvent::EndElement { .. }) => {
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
      Err(e) => return Err(ParseError::General(e)),
      _ => (),
    }
  }

  Ok(roots)
}

#[cfg(test)]
mod tests {
  use super::Node;
  use speculoos::prelude::*;

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
