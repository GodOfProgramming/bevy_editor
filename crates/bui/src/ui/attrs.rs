use super::{Attribute, NoParams};
use crate::{BuiPlugin, PrimaryType, reflection};
use bevy::{
  ecs::system::SystemParam,
  prelude::*,
  text::{self, ComputedTextBlock, FontSmoothing},
  ui::widget::TextNodeFlags,
};
use serde::{Deserialize, Serialize};
use std::{marker::PhantomData, path::PathBuf};

pub fn register_all(plugin: &mut BuiPlugin) {
  super::generated::attrs::register_all(plugin);
  plugin.register_attr::<Style>();
  plugin
    .blacklist::<PrimaryType>()
    .blacklist::<ComputedNode>()
    .blacklist::<ComputedNodeTarget>()
    .blacklist::<ComputedTextBlock>()
    .blacklist::<TextNodeFlags>()
    .blacklist::<ScrollPosition>()
    .blacklist::<TransformTreeChanged>()
    .blacklist::<Children>()
    .blacklist::<ChildOf>();
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
}

impl Attribute for Style {
  type Params<'w, 's> = NoParams;
  fn construct(&self, _params: Self::Params<'_, '_>) -> impl Bundle {
    let mut node = Node::default();
    reflection::patch_reflect(self, &mut node);
    node
  }
}

#[derive(Serialize, Deserialize, Default, Reflect, Clone)]
#[reflect(Serialize, Deserialize)]
#[serde(default)]
pub struct Font {
  pub font: PathBuf,
  pub size: f32,
  pub line_height: LineHeight,
  pub smoothing: FontSmoothing,
}

#[derive(SystemParam)]
pub struct FontParams<'w, 's> {
  assets: Res<'w, AssetServer>,
  _pd: PhantomData<&'s ()>,
}

impl Attribute for Font {
  type Params<'w, 's> = FontParams<'w, 's>;
  fn construct(&self, params: Self::Params<'_, '_>) -> impl Bundle {
    let font: Handle<text::Font> = params.assets.load(self.font.clone());
    TextFont::from_font(font)
      .with_font_size(self.size)
      .with_line_height(self.line_height.into())
      .with_font_smoothing(self.smoothing)
  }
}

#[derive(Debug, Clone, Copy, Reflect, Serialize, Deserialize)]
#[reflect(Debug, Clone, Serialize, Deserialize)]
pub enum LineHeight {
  /// Set line height to a specific number of pixels
  Px(f32),
  /// Set line height to a multiple of the font size
  RelativeToFont(f32),
}

impl Default for LineHeight {
  fn default() -> Self {
    LineHeight::RelativeToFont(1.2)
  }
}

impl From<LineHeight> for text::LineHeight {
  fn from(value: LineHeight) -> Self {
    match value {
      LineHeight::Px(px) => text::LineHeight::Px(px),
      LineHeight::RelativeToFont(rel) => text::LineHeight::RelativeToFont(rel),
    }
  }
}
