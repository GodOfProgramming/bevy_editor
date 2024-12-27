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
use bevy::utils::HashMap;
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
      .add_plugins(EguiPlugin)
      .add_systems(PreStartup, init_resources)
      .add_systems(Update, render);
  }
}

fn init_resources(world: &mut World) {
  let layout = Layout::new(world);
  world.insert_resource(layout);
  world.insert_resource(InspectorSelection::Entities(SelectedEntities::default()));
}

pub fn render(world: &mut World) {
  world.resource_scope(|world, mut ui_state: Mut<Layout>| {
    ui_state.render(world);
  });
}

pub trait Ui: Resource + Send + Sync {
  fn title(&mut self) -> egui::WidgetText {
    std::any::type_name::<Self>().into()
  }

  fn can_clear(&self) -> bool {
    true
  }

  fn on_close(&self, _world: &mut World) {}

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

  fn on_close(&self, _world: &mut World) {}

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

  fn on_close(&self, world: &mut World) {
    ParameterizedUi::on_close(self, world);
  }

  fn render(&mut self, ui: &mut egui::Ui, world: &mut World) {
    if !world.is_resource_added::<UiComponentState<<Self as ParameterizedUi>::Params<'_, '_>>>() {
      let state = SystemState::<<Self as ParameterizedUi>::Params<'_, '_>>::new(world);
      world.insert_resource(UiComponentState(state));
    }

    world.resource_scope(
      |world, mut params: Mut<UiComponentState<<Self as ParameterizedUi>::Params<'_, '_>>>| {
        let params = params.get_mut(world);
        ParameterizedUi::render(self, ui, params);
      },
    );
  }
}

#[derive(Resource)]
struct UiComponentState<P>(SystemState<P>)
where
  P: SystemParam + 'static;

impl<P> Deref for UiComponentState<P>
where
  P: SystemParam + 'static,
{
  type Target = SystemState<P>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<P> DerefMut for UiComponentState<P>
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

struct VTable {
  title: fn(&mut World) -> egui::WidgetText,
  can_clear: fn(&World) -> bool,
  on_close: fn(&mut World),
  render: fn(&mut egui::Ui, &mut World),
}

impl VTable {
  pub fn new<T>() -> Self
  where
    T: Ui,
  {
    Self {
      title: |world| world.resource_mut::<T>().title(),
      can_clear: |world| world.resource::<T>().can_clear(),
      on_close: |world| {
        world.resource_scope(|world, res: Mut<T>| {
          res.on_close(world);
        });
        world.remove_resource::<T>();
      },
      render: |ui, world| {
        world.resource_scope(|world, mut res: Mut<T>| {
          res.render(ui, world);
        });
      },
    }
  }
}

#[derive(Resource)]
struct Layout {
  dock: DockState<TypeId>,
  panels: HashMap<TypeId, VTable>,
}

impl Layout {
  pub fn new(w: &mut World) -> Self {
    let mut panels = HashMap::new();
    let p = &mut panels;

    let mut state = DockState::new(vec![ui::<GameView>(p, w)]);

    let tree = state.main_surface_mut();

    let root = NodeIndex::root();

    let tabs = vec![ui::<HierarchyUi>(p, w), ui::<ControlPanelUi>(p, w)];
    let [central_panel, _left_panel] = tree.split_left(root, 1.0 / 6.0, tabs);

    let tabs = vec![ui::<Inspector>(p, w)];
    let [central_panel, _right_panel] = tree.split_right(central_panel, 4.0 / 5.0, tabs);

    let tabs = vec![
      ui::<PrefabsUi>(p, w),
      ui::<Resources>(p, w),
      ui::<AssetsUi>(p, w),
    ];
    tree.split_below(central_panel, 0.7, tabs);

    Self {
      dock: state,
      panels,
    }
  }

  fn render(&mut self, world: &mut World) {
    let Ok(mut ctx) = world
      .query::<&mut bevy_egui::EguiContext>()
      .get_single_mut(world)
    else {
      return;
    };

    let ctx = ctx.get_mut().clone();

    let mut tab_viewer = TabViewer {
      panels: &mut self.panels,
      world,
    };

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
  panels: &'a mut HashMap<TypeId, VTable>,
  world: &'a mut World,
}

impl egui_dock::TabViewer for TabViewer<'_> {
  type Tab = TypeId;

  fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
    (self.panels.get_mut(tab).unwrap().title)(self.world)
  }

  fn clear_background(&self, tab: &Self::Tab) -> bool {
    (self.panels.get(tab).unwrap().can_clear)(self.world)
  }

  fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
    (self.panels.get_mut(tab).unwrap().render)(ui, self.world);
  }

  fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
    false
  }

  fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
    (self.panels.get_mut(tab).unwrap().on_close)(self.world);
    true
  }
}

fn ui<U: Ui>(panels: &mut HashMap<TypeId, VTable>, world: &mut World) -> TypeId
where
  U: Default + 'static,
{
  let type_id = TypeId::of::<U>();
  panels.insert(type_id, VTable::new::<U>());
  world.insert_resource(U::default());
  type_id
}
