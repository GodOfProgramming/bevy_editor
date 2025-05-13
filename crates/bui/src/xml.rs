use std::{collections::BTreeMap, io::BufReader};
use thiserror::Error;
use xml::{EventReader, reader::XmlEvent};

#[derive(Debug, Error)]
pub enum XmlParseError {
  #[error("Expected tag to exist")]
  ExpectedTag,

  #[error("Expected node to exist")]
  ExpectedNode,

  #[error("{0}")]
  General(xml::reader::Error),
}

#[derive(Debug)]
pub enum XmlNode {
  Tag(Tag),
  Text(String),
}

#[derive(Default, Debug)]
pub struct Tag {
  name: String,
  attrs: BTreeMap<String, String>,
  children: Vec<XmlNode>,
}

impl Tag {}

pub fn parse(data: &str) -> Result<Vec<XmlNode>, XmlParseError> {
  let reader = BufReader::new(data.as_bytes());
  let parser = EventReader::new(reader);

  let mut root_nodes = Vec::new();
  let mut stack = Vec::new();

  for event in parser {
    match event {
      Ok(XmlEvent::StartElement {
        name, attributes, ..
      }) => {
        let mut attrs = BTreeMap::default();

        for attr in attributes {
          let name = attr.name.to_string();
          let value = attr.value;
          attrs.insert(name, value);
        }

        let tag = Tag {
          name: name.to_string(),
          attrs,
          children: Vec::new(),
        };

        stack.push(XmlNode::Tag(tag));
      }
      Ok(XmlEvent::EndElement { .. }) => {
        if stack.is_empty() {
          return Err(XmlParseError::ExpectedTag);
        }

        if stack.len() == 1 {
          let Some(node) = stack.pop() else {
            return Err(XmlParseError::ExpectedTag);
          };

          root_nodes.push(node);
        } else {
          let Some(node) = stack.pop() else {
            return Err(XmlParseError::ExpectedTag);
          };

          let Some(XmlNode::Tag(tag)) = stack.last_mut() else {
            return Err(XmlParseError::ExpectedTag);
          };

          tag.children.push(node);
        }
      }
      Ok(XmlEvent::Characters(text) | XmlEvent::CData(text)) => {
        let Some(XmlNode::Tag(tag)) = stack.last_mut() else {
          return Err(XmlParseError::ExpectedTag);
        };

        tag.children.push(XmlNode::Text(text.trim().to_string()));
      }
      Err(e) => return Err(XmlParseError::General(e)),
      _ => (),
    }
  }

  Ok(root_nodes)
}

#[cfg(test)]
mod tests {
  use super::{Tag, XmlNode};
  use speculoos::prelude::*;

  const DUMMY_DATA: &str = include_str!("../test/dummy_data.xml");

  #[test]
  fn parse_dummy_data() {
    let nodes = super::parse(DUMMY_DATA).unwrap();

    assert_that(&nodes.len()).is_equal_to(1);

    let XmlNode::Tag(pane) = nodes.first().unwrap() else {
      panic!("Expected pane tag to be first")
    };

    assert_that(&pane.name.as_str()).is_equal_to("Pane");

    let (XmlNode::Text(text1), XmlNode::Tag(button), XmlNode::Tag(rich_text), XmlNode::Text(text2)) = (
      &pane.children[0],
      &pane.children[1],
      &pane.children[2],
      &pane.children[3],
    ) else {
      panic!(
        "Unexpected number of child elements: {}",
        pane.children.len()
      );
    };

    assert_that(&text1.as_str()).is_equal_to("Example Text 1");

    let button_on_click = &button.attrs["OnClick"];
    let XmlNode::Text(button_text) = &button.children[0] else {
      panic!("Expected button text");
    };
    assert_that(&button.name.as_str()).is_equal_to("Button");
    assert_that(&button_on_click.as_str()).is_equal_to("SomeEvent");
    assert_that(&button_text.as_str()).is_equal_to("Button Example");

    let XmlNode::Text(rich_text_text) = &rich_text.children[0] else {
      panic!("Expected nested text");
    };
    assert_that(&rich_text.name.as_str()).is_equal_to("RichText");
    assert_that(&rich_text_text.as_str()).is_equal_to("Some Unicode = \u{E596}");

    assert_that(&text2.as_str()).is_equal_to("Example Text 2");
  }
}
