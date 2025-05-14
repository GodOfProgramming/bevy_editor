use std::{collections::BTreeMap, io::BufReader, ops::Index};
use thiserror::Error;
use xml::{EventReader, reader::XmlEvent};

#[derive(Debug, Error)]
pub enum ParseError {
  #[error("Expected tag to exist")]
  ExpectedTag,

  #[error("Expected node to exist")]
  ExpectedNode,

  #[error("{0}")]
  General(xml::reader::Error),
}

#[derive(Debug)]
pub enum Node {
  Tag(Tag),
  Text(String),
}

#[derive(Default, Debug)]
pub struct Tag {
  pub name: String,
  pub attrs: BTreeMap<String, String>,
  pub children: Vec<Node>,
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

        stack.push(Node::Tag(tag));
      }
      Ok(XmlEvent::EndElement { .. }) => {
        if stack.is_empty() {
          return Err(ParseError::ExpectedTag);
        }

        if stack.len() == 1 {
          let Some(node) = stack.pop() else {
            return Err(ParseError::ExpectedTag);
          };

          root_nodes.push(node);
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
      Ok(XmlEvent::Characters(text) | XmlEvent::CData(text)) => {
        let Some(Node::Tag(tag)) = stack.last_mut() else {
          return Err(ParseError::ExpectedTag);
        };

        tag.children.push(Node::Text(text.trim().to_string()));
      }
      Err(e) => return Err(ParseError::General(e)),
      _ => (),
    }
  }

  Ok(root_nodes)
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
