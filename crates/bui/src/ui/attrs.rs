use crate::patch_reflect;

use super::Attribute;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Reflect, Clone)]
#[serde(default)]
pub struct Style {
  pub display: Display,
  pub box_sizing: BoxSizing,
  pub position_type: PositionType,
  pub overflow: Overflow,
  pub overflow_clip_margin: OverflowClipMargin,
  pub left: Val,
  pub right: Val,
  pub top: Val,
  pub bottom: Val,
  pub width: Val,
  pub height: Val,
  pub min_width: Val,
  pub min_height: Val,
  pub max_width: Val,
  pub max_height: Val,
  pub aspect_ratio: Option<f32>,
  pub align_items: AlignItems,
  pub justify_items: JustifyItems,
  pub align_self: AlignSelf,
  pub justify_self: JustifySelf,
  pub align_content: AlignContent,
  pub justify_content: JustifyContent,
  pub margin: UiRect,
  pub padding: UiRect,
  pub border: UiRect,
  pub flex_direction: FlexDirection,
  pub flex_wrap: FlexWrap,
  pub flex_grow: f32,
  pub flex_shrink: f32,
  pub flex_basis: Val,
  pub row_gap: Val,
  pub column_gap: Val,
  pub grid_auto_flow: GridAutoFlow,
  pub grid_template_rows: Vec<RepeatedGridTrack>,
  pub grid_template_columns: Vec<RepeatedGridTrack>,
  pub grid_auto_rows: Vec<GridTrack>,
  pub grid_auto_columns: Vec<GridTrack>,
  pub grid_row: GridPlacement,
  pub grid_column: GridPlacement,

  pub background_color: Option<Color>,
}

impl Attribute for Style {
  fn insert_into(&self, mut entity: EntityWorldMut) {
    let mut node = Node::default();
    patch_reflect(self, &mut node);
    entity.insert(node);

    if let Some(bg) = &self.background_color {
      entity.insert(BackgroundColor(*bg));
    }
  }
}

#[cfg(test)]
mod tests {
  use std::any::Any;

  use super::Style;
  use bevy::reflect::{GetTypeRegistration, Reflect, TypeRegistry, serde::ReflectDeserializer};
  use serde::de::DeserializeSeed;

  #[test]
  fn style_impls_reflect() {
    let tp = Style::get_type_registration().type_info().type_path();
    let ron = format!("{{ \"{tp}\": ( width: Px(150.0) ) }}");

    let mut tr = TypeRegistry::new();
    tr.register::<Style>();

    let de = ReflectDeserializer::new(&tr);
    let mut rd = ron::Deserializer::from_str(&ron).unwrap();
    let value = de.deserialize(&mut rd).unwrap();

    let style = Style::default();
    let style = style.as_reflect();
    let style = style.as_partial_reflect();
    let style = style.try_as_reflect();

    let sanity = style.is_some();
    assert!(sanity);

    let style = style.unwrap();
    let tid = style.get_represented_type_info().unwrap().type_id();
    let ti = tr.get_type_info(tid).unwrap();

    panic!("actual path => {}, first path => {}", ti.type_path(), tp);

    // assert!(value.try_into_reflect().is_ok());
  }
}
