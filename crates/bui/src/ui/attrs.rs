use crate::{BuiPlugin, reflection};

use super::Attribute;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub fn register_all(plugin: &mut BuiPlugin) {
  plugin
    .register_attr::<Style>()
    .register_attr::<TextColor>()
    .register_attr::<BackgroundColor>();
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

  pub color: Option<Color>,
  pub background_color: Option<Color>,
}

impl Attribute for Style {
  fn insert_into(&self, mut entity: EntityWorldMut) {
    let mut node = Node::default();
    let patches = reflection::patch_reflect(self, &mut node);
    if patches > 0 {
      entity.insert(node);
    }

    if let Some(fg) = self.color {
      entity.insert(TextColor(fg));
    }

    if let Some(bg) = self.background_color {
      entity.insert(BackgroundColor(bg));
    }
  }
}
