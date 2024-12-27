pub mod assets;
pub mod control_panel;
pub mod game_view;
pub mod hierarchy;
pub mod inspector;
pub mod prefabs;
pub mod resources;

use assets::AssetsUi;
use bevy::asset::UntypedAssetId;
use bevy::ecs::system::{SystemParam, SystemState};
use bevy::prelude::*;
use bevy_egui::{egui, EguiPlugin};
use bevy_inspector_egui::bevy_inspector;
use control_panel::ControlPanelUi;
use egui_dock::{DockArea, DockState, NodeIndex};
use game_view::GameView;
use hierarchy::HierarchyUi;
use inspector::Inspector;
use prefabs::PrefabsUi;
use resources::Resources;
use std::any::TypeId;
use std::ops::{Deref, DerefMut};

pub(crate) struct UiPlugin;

impl Plugin for UiPlugin {
  fn build(&self, app: &mut App) {
    debug!("Building UI Plugin");
    app
      .insert_resource(InspectorSelection::Entities(SelectedEntities::default()))
      .add_plugins(EguiPlugin)
      .add_systems(Startup, on_start)
      .add_systems(Update, render);
  }
}

fn on_start(mut commands: Commands) {
  commands.insert_resource(State::new());
}

pub fn render(world: &mut World) {
  world.resource_scope(|world, mut ui_state: Mut<State>| {
    ui_state.render(world);
  });
}

pub trait Ui: Send + Sync {
  fn title(&mut self) -> egui::WidgetText {
    std::any::type_name::<Self>().into()
  }

  fn can_clear(&self) -> bool {
    true
  }

  fn render(&mut self, ui: &mut egui::Ui, world: &mut World);
}

pub trait ParameterizedUi: Ui {
  type Params<'w, 's>: for<'world, 'system> SystemParam<
    Item<'world, 'system> = Self::Params<'world, 'system>,
  >;

  fn title(&mut self) -> egui::WidgetText {
    std::any::type_name::<Self>().into()
  }

  fn can_clear(&self) -> bool {
    true
  }

  fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>);
}

impl<T> Ui for T
where
  T: ParameterizedUi + 'static,
{
  fn title(&mut self) -> egui::WidgetText {
    ParameterizedUi::title(self)
  }

  fn can_clear(&self) -> bool {
    ParameterizedUi::can_clear(self)
  }

  fn render(&mut self, ui: &mut egui::Ui, world: &mut World) {
    if !world.is_resource_added::<UiState<<Self as ParameterizedUi>::Params<'_, '_>>>() {
      let state = SystemState::<<Self as ParameterizedUi>::Params<'_, '_>>::new(world);
      world.insert_resource(UiState(state));
    }

    world.resource_scope(
      |world, mut params: Mut<UiState<<Self as ParameterizedUi>::Params<'_, '_>>>| {
        let params = params.get_mut(world);
        ParameterizedUi::render(self, ui, params);
      },
    );
  }
}

#[derive(Component)]
pub struct UiComponent(Box<dyn Ui>);

impl Deref for UiComponent {
  type Target = Box<dyn Ui>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for UiComponent {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

#[derive(Resource)]
struct UiState<P>(SystemState<P>)
where
  P: SystemParam + 'static;

impl<P> Deref for UiState<P>
where
  P: SystemParam + 'static,
{
  type Target = SystemState<P>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<P> DerefMut for UiState<P>
where
  P: SystemParam + 'static,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

#[derive(Resource)]
pub enum InspectorSelection {
  Entities(SelectedEntities),
  Resource(TypeId, String),
  Asset(TypeId, String, UntypedAssetId),
}

impl InspectorSelection {
  pub fn add_selected(&mut self, entity: Entity, add: bool) {
    if let InspectorSelection::Entities(selected_entities) = self {
      selected_entities.select_maybe_add(entity, add);
    } else {
      let mut selected_entities = SelectedEntities::default();
      selected_entities.select_replace(entity);
      *self = Self::Entities(selected_entities);
    }
  }
}

#[derive(Default, Debug)]
pub struct SelectedEntities(bevy_inspector::hierarchy::SelectedEntities);

impl Deref for SelectedEntities {
  type Target = bevy_inspector::hierarchy::SelectedEntities;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for SelectedEntities {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

#[derive(Resource)]
pub(crate) struct State {
  dock: DockState<Box<dyn Ui>>,
  viewport_rect: egui::Rect,
  game_view_hovered: bool,
}

impl State {
  pub fn new() -> Self {
    let mut state = DockState::new(vec![ui::<GameView>()]);

    let tree = state.main_surface_mut();

    let root = NodeIndex::root();

    let tabs = vec![ui::<HierarchyUi>(), ui::<ControlPanelUi>()];
    let [central_panel, _left_panel] = tree.split_left(root, 1.0 / 6.0, tabs);

    let tabs = vec![ui::<Inspector>()];
    let [central_panel, _right_panel] = tree.split_right(central_panel, 4.0 / 5.0, tabs);

    let tabs = vec![ui::<PrefabsUi>(), ui::<Resources>(), ui::<AssetsUi>()];
    tree.split_below(central_panel, 0.7, tabs);

    Self {
      dock: state,
      viewport_rect: egui::Rect::NOTHING,
      game_view_hovered: false,
    }
  }

  pub fn viewport(&self) -> egui::Rect {
    egui::Rect {
      max: egui::Pos2::new(self.viewport_rect.max.x, self.viewport_rect.max.y),
      min: egui::Pos2::new(self.viewport_rect.min.x, self.viewport_rect.min.y),
    }
  }

  pub fn hovered(&self) -> bool {
    self.game_view_hovered
  }

  fn render(&mut self, world: &mut World) {
    let Ok(mut ctx) = world
      .query::<&mut bevy_egui::EguiContext>()
      .get_single_mut(world)
    else {
      return;
    };

    let ctx = ctx.get_mut().clone();

    let mut tab_viewer = TabViewer { world };

    egui::CentralPanel::default()
      .frame(
        egui::Frame::central_panel(&ctx.style())
          .inner_margin(0.)
          .fill(egui::Color32::TRANSPARENT),
      )
      .show(&ctx, |ui| {
        DockArea::new(&mut self.dock).show_inside(ui, &mut tab_viewer);
      });
  }
}

struct TabViewer<'a> {
  world: &'a mut World,
}

impl egui_dock::TabViewer for TabViewer<'_> {
  type Tab = Box<dyn Ui>;

  fn title(&mut self, window: &mut Self::Tab) -> egui::WidgetText {
    window.title()
  }

  fn clear_background(&self, window: &Self::Tab) -> bool {
    window.can_clear()
  }

  fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
    tab.render(ui, self.world);
  }

  fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
    false
  }

  fn on_close(&mut self, _tab: &mut Self::Tab) -> bool {
    // self.world.despawn(tab)
    false
  }
}

fn ui<U: Ui>() -> Box<dyn Ui>
where
  U: Default + 'static,
{
  Box::new(U::default())
}
