use crate::{BuiPlugin, patch_reflect};

use super::Attribute;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub fn register_all(plugin: &mut BuiPlugin) {
  plugin.register_attr::<Style>();
}

#[derive(Serialize, Deserialize, Default, Reflect, Clone)]
#[reflect(Serialize, Deserialize)]
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

#[derive(Default, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
struct Foo {}

#[cfg(test)]
mod tests {
  use super::Style;
  use crate::ui::attrs::Foo;
  use bevy::reflect::{GetTypeRegistration, TypeRegistry, serde::TypedReflectDeserializer};
  use serde::de::DeserializeSeed;

  #[test]
  fn style_impls_reflect() {
    let mut tr = TypeRegistry::new();
    tr.register::<Style>();
    tr.register::<Foo>();

    fn check_foo(tr: &TypeRegistry) {
      let reg = Foo::get_type_registration();
      let ron = "( )";

      let de = TypedReflectDeserializer::new(&reg, tr);
      let mut rd = ron::Deserializer::from_str(ron).unwrap();
      let value = de.deserialize(&mut rd).unwrap();

      let foo = value.try_as_reflect();

      assert!(foo.is_some());
    }

    fn check_style(tr: &TypeRegistry) {
      let reg = Style::get_type_registration();
      let ron = "( width: Px(150.0) )";

      let de = TypedReflectDeserializer::new(&reg, tr);
      let mut rd = ron::Deserializer::from_str(ron).unwrap();
      let value = de.deserialize(&mut rd).unwrap();

      let style = value.try_as_reflect();

      assert!(style.is_some());
    }

    check_foo(&tr);
    check_style(&tr);
  }
}
