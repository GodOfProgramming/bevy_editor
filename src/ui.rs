pub mod assets;
pub mod control_panel;
pub mod game_view;
pub mod hierarchy;
pub mod inspector;
pub mod prefabs;
pub mod resources;

use assets::Assets;
use bevy::asset::UntypedAssetId;
use bevy::ecs::system::{SystemParam, SystemState};
use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use bevy::utils::HashMap;
use bevy_egui::{egui, EguiPlugin};
use bevy_inspector_egui::bevy_inspector;
use control_panel::ControlPanelUi;
use egui_dock::{DockArea, DockState, NodeIndex};
use game_view::GameView;
use hierarchy::Hierarchy;
use inspector::Inspector;
use prefabs::Prefabs;
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

pub trait Ui: Resource + GetTypeRegistration + Send + Sync + Sized {
  fn title(&mut self) -> egui::WidgetText;

  fn can_clear(&self) -> bool {
    true
  }

  fn closeable(&mut self) -> bool {
    false
  }

  #[allow(unused_variables)]
  fn on_close(&mut self, world: &mut World) {}

  fn render(&mut self, ui: &mut egui::Ui, world: &mut World);

  #[allow(unused_variables)]
  fn context_menu(&mut self, ui: &mut egui::Ui, world: &mut World) {}
}

trait RegisterParams: ParameterizedUi {
  fn register_params(world: &mut World) {
    if !world.is_resource_added::<ComponentState<Self>>() {
      let state = SystemState::<<Self as ParameterizedUi>::Params<'_, '_>>::new(world);
      world.insert_resource(UiComponentState(state));
    }
  }
}

impl<T> RegisterParams for T where T: ParameterizedUi {}

pub trait ParameterizedUi: Ui {
  type Params<'w, 's>: for<'world, 'system> SystemParam<
    Item<'world, 'system> = Self::Params<'world, 'system>,
  >;

  fn title(&mut self) -> egui::WidgetText;

  fn can_clear(&self) -> bool {
    true
  }

  fn closeable(&mut self) -> bool {
    false
  }

  #[allow(unused_variables)]
  fn on_close(&mut self, params: Self::Params<'_, '_>) {}

  fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>);

  #[allow(unused_variables)]
  fn context_menu(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {}
}

type ComponentState<'w, 's, T> = UiComponentState<<T as ParameterizedUi>::Params<'w, 's>>;

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

  fn closeable(&mut self) -> bool {
    ParameterizedUi::closeable(self)
  }

  fn on_close(&mut self, world: &mut World) {
    T::register_params(world);
    world.resource_scope(|world, mut params: Mut<ComponentState<Self>>| {
      let params = params.get_mut(world);
      ParameterizedUi::on_close(self, params);
    });
  }

  fn render(&mut self, ui: &mut egui::Ui, world: &mut World) {
    T::register_params(world);
    world.resource_scope(|world, mut params: Mut<ComponentState<Self>>| {
      let params = params.get_mut(world);
      ParameterizedUi::render(self, ui, params);
    });
  }

  fn context_menu(&mut self, ui: &mut egui::Ui, world: &mut World) {
    T::register_params(world);
    world.resource_scope(|world, mut params: Mut<ComponentState<Self>>| {
      let params = params.get_mut(world);
      ParameterizedUi::context_menu(self, ui, params);
    });
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
  closable: fn(&mut World) -> bool,
  on_close: fn(&mut World),
  render: fn(&mut egui::Ui, &mut World),
  context_menu: fn(&mut egui::Ui, &mut World),
}

impl VTable {
  pub fn new<T>() -> Self
  where
    T: Ui,
  {
    Self {
      title: |world| world.resource_mut::<T>().title(),
      can_clear: |world| world.resource::<T>().can_clear(),
      closable: |world| world.resource_mut::<T>().closeable(),
      on_close: |world| {
        world.resource_scope(|world, mut res: Mut<T>| {
          res.on_close(world);
        });
      },
      render: |ui, world| {
        world.resource_scope(|world, mut res: Mut<T>| {
          res.render(ui, world);
        });
      },
      context_menu: |ui, world| {
        world.resource_scope(|world, mut res: Mut<T>| {
          res.context_menu(ui, world);
        });
      },
    }
  }
}

#[derive(Resource)]
struct Layout {
  dock: DockState<TypeId>,
  panels: HashMap<TypeId, VTable>,
  id: egui::Id,
}

impl Layout {
  pub fn new(w: &mut World) -> Self {
    let mut panels = HashMap::new();
    let p = &mut panels;

    let mut state = DockState::new(vec![ui::<GameView>(p, w)]);

    let tree = state.main_surface_mut();

    let root = NodeIndex::root();

    let tabs = vec![ui::<Hierarchy>(p, w), ui::<ControlPanelUi>(p, w)];
    let [central_panel, _left_panel] = tree.split_left(root, 1.0 / 6.0, tabs);

    let tabs = vec![ui::<Inspector>(p, w)];
    let [central_panel, _right_panel] = tree.split_right(central_panel, 4.0 / 5.0, tabs);

    let tabs = vec![
      ui::<Prefabs>(p, w),
      ui::<Resources>(p, w),
      ui::<Assets>(p, w),
    ];
    tree.split_below(central_panel, 0.7, tabs);

    Self {
      dock: state,
      panels,
      id: egui::Id::new(TypeId::of::<Self>()),
    }
  }

  fn render(&mut self, world: &mut World) {
    let Ok(ctx) = world
      .query::<&mut bevy_egui::EguiContext>()
      .get_single_mut(world)
      .map(|ctx| ctx.get().clone())
    else {
      return;
    };

    egui::CentralPanel::default()
      .frame(
        egui::Frame::central_panel(&ctx.style())
          .inner_margin(0.0)
          .fill(egui::Color32::TRANSPARENT),
      )
      .show(&ctx, |ui| {
        egui::menu::bar(ui, |ui| {
          self.menu_bar_ui(ui);
        });

        let mut tab_viewer = TabViewer {
          panels: &mut self.panels,
          world,
        };
        DockArea::new(&mut self.dock)
          .id(self.id)
          .show_inside(ui, &mut tab_viewer);
      });
  }

  fn menu_bar_ui(&mut self, ui: &mut egui::Ui) {
    ui.menu_button("File", |ui| {
      if ui.button("Open Scene").clicked() {
        debug!("Do scene open dialog");
      }
    });
  }
}

struct TabViewer<'a> {
  world: &'a mut World,
  panels: &'a mut HashMap<TypeId, VTable>,
}

impl egui_dock::TabViewer for TabViewer<'_> {
  type Tab = TypeId;

  fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
    (self.panels.get_mut(tab).unwrap().title)(self.world)
  }

  fn clear_background(&self, tab: &Self::Tab) -> bool {
    (self.panels.get(tab).unwrap().can_clear)(self.world)
  }

  fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
    (self.panels.get(tab).unwrap().closable)(self.world)
  }

  fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
    (self.panels.get(tab).unwrap().on_close)(self.world);
    true
  }

  fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
    (self.panels.get_mut(tab).unwrap().render)(ui, self.world);
  }

  fn context_menu(
    &mut self,
    ui: &mut egui::Ui,
    tab: &mut Self::Tab,
    _surface: egui_dock::SurfaceIndex,
    _node: NodeIndex,
  ) {
    (self.panels.get_mut(tab).unwrap().context_menu)(ui, self.world);
  }

  fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
    egui::Id::new(tab)
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
