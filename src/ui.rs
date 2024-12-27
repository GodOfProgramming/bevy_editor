pub mod assets;
pub mod control_panel;
pub mod game_view;
pub mod hierarchy;
pub mod inspector;
pub mod prefabs;
pub mod resources;

use crate::cache::{Cache, Saveable};
use assets::Assets;
use bevy::asset::UntypedAssetId;
use bevy::ecs::system::{SystemParam, SystemState};
use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use bevy::utils::HashMap;
use bevy_egui::{egui, EguiPlugin};
use bevy_inspector_egui::bevy_inspector;
use control_panel::ControlPanel;
use egui_dock::{DockArea, DockState, NodeIndex};
use game_view::GameView;
use hierarchy::Hierarchy;
use inspector::Inspector;
use prefabs::Prefabs;
use resources::Resources;
use serde::{Deserialize, Serialize};
use std::any::TypeId;
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use uuid::Uuid;

pub(crate) struct UiPlugin;

impl Plugin for UiPlugin {
  fn build(&self, app: &mut App) {
    debug!("Building UI Plugin");
    app
      .init_resource::<Layout>()
      .init_resource::<InspectorSelection>()
      .add_plugins(EguiPlugin)
      .add_systems(Startup, init_resources)
      .add_systems(Update, render);
  }
}

fn init_resources(world: &mut World) {
  world.resource_scope(|world, mut layout: Mut<Layout>| {
    layout.restore_or_init(world);
  });
}

pub fn render(world: &mut World) {
  world.resource_scope(|world, mut layout: Mut<Layout>| {
    layout.render(world);
  });
}

pub fn on_app_exit(mut cache: ResMut<Cache>, layout: Res<Layout>, q_uuids: Query<&PersistentId>) {
  let new_state = layout.state.map_tabs(|tab| **q_uuids.get(*tab).unwrap());
  cache.store(&LayoutState(new_state));
}

pub trait UiComponent: Component + GetTypeRegistration + Send + Sync + Sized {
  const COMPONENT_NAME: &str;
  const ID: PersistentId;

  fn spawn(world: &mut World) -> Self;

  #[allow(unused_variables)]
  fn title(entity: Entity, world: &mut World) -> egui::WidgetText {
    Self::COMPONENT_NAME.into()
  }

  #[allow(unused_variables)]
  fn can_clear(entity: Entity, world: &mut World) -> bool {
    true
  }

  #[allow(unused_variables)]
  fn closeable(entity: Entity, world: &mut World) -> bool {
    false
  }

  #[allow(unused_variables)]
  fn on_close(entity: Entity, world: &mut World) {}

  fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World);

  #[allow(unused_variables)]
  fn context_menu(entity: Entity, ui: &mut egui::Ui, world: &mut World) {}
}

trait RegisterParams: Ui {
  fn register_params(entity: Entity, world: &mut World) {
    if !world.entity(entity).contains::<ComponentState<Self>>() {
      let state = SystemState::<<Self as Ui>::Params<'_, '_>>::new(world);
      world.entity_mut(entity).insert(UiComponentState(state));
    }
  }

  fn with_params<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(Self::Params<'_, '_>) -> T,
  ) -> T {
    let world_cell = world.as_unsafe_world_cell();
    let mut entity = unsafe { world_cell.world_mut() }.entity_mut(entity);
    let mut params = entity.get_mut::<ComponentState<Self>>().unwrap();
    let params = params.get_mut(unsafe { world_cell.world_mut() });
    f(params)
  }
}

impl<T> RegisterParams for T where T: Ui {}

trait UiExtensions: Ui {
  fn get_entity<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(&Self, Self::Params<'_, '_>) -> T,
  ) -> T {
    Self::register_params(entity, world);
    let mut q = world.query::<(&Self, &mut ComponentState<Self>)>();
    let world_cell = world.as_unsafe_world_cell();
    let (this, mut params) = q
      .get_mut(unsafe { world_cell.world_mut() }, entity)
      .unwrap();
    let params = params.get_mut(unsafe { world_cell.world_mut() });
    f(this, params)
  }

  fn get_entity_mut<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(&mut Self, Self::Params<'_, '_>) -> T,
  ) -> T {
    Self::register_params(entity, world);
    let mut q = world.query::<(&mut Self, &mut ComponentState<Self>)>();
    let world_cell = world.as_unsafe_world_cell();
    let (mut this, mut params) = q
      .get_mut(unsafe { world_cell.world_mut() }, entity)
      .unwrap();
    let params = params.get_mut(unsafe { world_cell.world_mut() });
    f(this.as_mut(), params)
  }
}

impl<T> UiExtensions for T where T: Ui {}

#[derive(SystemParam)]
pub struct NoParams;

pub trait Ui: UiComponent {
  const NAME: &str;
  const UUID: Uuid;

  type Params<'w, 's>: for<'world, 'system> SystemParam<
    Item<'world, 'system> = Self::Params<'world, 'system>,
  >;

  fn spawn(params: Self::Params<'_, '_>) -> Self;

  #[allow(unused_variables)]
  fn title(&mut self, params: Self::Params<'_, '_>) -> egui::WidgetText {
    Self::NAME.into()
  }

  #[allow(unused_variables)]
  fn can_clear(&self, params: Self::Params<'_, '_>) -> bool {
    true
  }

  #[allow(unused_variables)]
  fn closeable(&mut self, params: Self::Params<'_, '_>) -> bool {
    false
  }

  #[allow(unused_variables)]
  fn on_close(&mut self, params: Self::Params<'_, '_>) {}

  fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>);

  #[allow(unused_variables)]
  fn context_menu(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {}
}

type ComponentState<'w, 's, T> = UiComponentState<<T as Ui>::Params<'w, 's>>;

impl<T> UiComponent for T
where
  T: Ui + 'static,
{
  const COMPONENT_NAME: &str = Self::NAME;
  const ID: PersistentId = PersistentId(<T as Ui>::UUID);

  fn spawn(world: &mut World) -> Self {
    let entity = world.spawn_empty().id();
    Self::register_params(entity, world);
    Self::with_params(entity, world, Ui::spawn)
  }

  fn title(entity: Entity, world: &mut World) -> egui::WidgetText {
    Self::get_entity_mut(entity, world, Ui::title)
  }

  fn can_clear(entity: Entity, world: &mut World) -> bool {
    Self::get_entity(entity, world, Ui::can_clear)
  }

  fn closeable(entity: Entity, world: &mut World) -> bool {
    Self::get_entity_mut(entity, world, Ui::closeable)
  }

  fn on_close(entity: Entity, world: &mut World) {
    Self::get_entity_mut(entity, world, |this, params| {
      this.on_close(params);
    })
  }

  fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    Self::get_entity_mut(entity, world, |this, params| {
      this.render(ui, params);
    })
  }

  fn context_menu(entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    Self::get_entity_mut(entity, world, |this, params| {
      this.context_menu(ui, params);
    })
  }
}

#[derive(Component)]
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

impl Default for InspectorSelection {
  fn default() -> Self {
    Self::Entities(default())
  }
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

#[derive(Default, Deref, DerefMut, Debug)]
pub struct SelectedEntities(bevy_inspector::hierarchy::SelectedEntities);

#[derive(Deref, DerefMut, Component, Clone, Copy, Hash, PartialEq, Eq)]
pub struct PersistentId(Uuid);

#[derive(Clone)]
struct VTable {
  spawn: fn(&mut World) -> Entity,
  title: fn(Entity, &mut World) -> egui::WidgetText,
  can_clear: fn(Entity, &mut World) -> bool,
  closable: fn(Entity, &mut World) -> bool,
  on_close: fn(Entity, &mut World),
  render: fn(Entity, &mut egui::Ui, &mut World),
  context_menu: fn(Entity, &mut egui::Ui, &mut World),
}

impl VTable {
  pub fn new<T>() -> Self
  where
    T: UiComponent,
  {
    Self {
      spawn: |world| {
        let instance = T::spawn(world);
        let entity_id = world.spawn((instance, T::ID)).id();
        world
          .entity_mut(entity_id)
          .insert(Name::new(T::COMPONENT_NAME));
        info!("Spawned UI component {}", T::COMPONENT_NAME);
        entity_id
      },
      title: T::title,
      can_clear: T::can_clear,
      closable: T::closeable,
      on_close: T::on_close,
      render: T::render,
      context_menu: T::context_menu,
    }
  }
}

#[derive(Deref, DerefMut, Serialize, Deserialize)]
struct LayoutState(DockState<Uuid>);

impl Saveable for LayoutState {
  const KEY: &str = "Layout";
}

#[derive(Resource)]
pub(crate) struct Layout {
  state: DockState<Entity>,
  vtables: HashMap<PersistentId, VTable>,
  id: egui::Id,
}

impl Default for Layout {
  fn default() -> Self {
    let mut this = Self {
      state: DockState::new(Vec::new()),
      vtables: default(),
      id: egui::Id::new(TypeId::of::<Self>()),
    };

    this.register::<GameView>();
    this.register::<Hierarchy>();
    this.register::<ControlPanel>();
    this.register::<Inspector>();
    this.register::<Prefabs>();
    this.register::<Resources>();
    this.register::<Assets>();

    this
  }
}

impl Layout {
  pub fn restore_or_init(&mut self, world: &mut World) {
    let state = world
      .resource_scope(|world, cache: Mut<Cache>| {
        cache.get::<LayoutState>().map(|persistent_layout| {
          persistent_layout.map_tabs(|tab| (self.vtables[&PersistentId(*tab)].spawn)(world))
        })
      })
      .unwrap_or_else(|| {
        let mut state = DockState::new(vec![self.spawn_type::<GameView>(world)]);

        let tree = state.main_surface_mut();

        let root = NodeIndex::root();

        let tabs = vec![
          self.spawn_type::<Hierarchy>(world),
          self.spawn_type::<ControlPanel>(world),
        ];
        let [central_panel, _left_panel] = tree.split_left(root, 1.0 / 6.0, tabs);

        let tabs = vec![self.spawn_type::<Inspector>(world)];
        let [central_panel, _right_panel] = tree.split_right(central_panel, 4.0 / 5.0, tabs);

        let tabs = vec![
          self.spawn_type::<Prefabs>(world),
          self.spawn_type::<Resources>(world),
          self.spawn_type::<Assets>(world),
        ];
        tree.split_below(central_panel, 0.7, tabs);

        state
      });

    self.state = state;
  }

  pub fn register<T: UiComponent>(&mut self) {
    self.vtables.insert(T::ID, VTable::new::<T>());
  }

  fn spawn_type<T: UiComponent>(&self, world: &mut World) -> Entity {
    self.spawn(T::ID, world)
  }

  fn spawn(&self, id: PersistentId, world: &mut World) -> Entity {
    (self.vtables[&id].spawn)(world)
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
          vtables: &mut self.vtables,
          world: RefCell::new(world),
        };

        DockArea::new(&mut self.state)
          .id(self.id)
          .show_inside(ui, &mut tab_viewer);
      });
  }

  fn menu_bar_ui(&mut self, ui: &mut egui::Ui) {
    ui.menu_button("Tools", |ui| {
      if ui.button("Generate UUID").clicked() {
        ui.output_mut(|output| {
          output.copied_text = Uuid::new_v4().to_string();
        });
      }
    });
  }
}

struct TabViewer<'a> {
  world: RefCell<&'a mut World>,
  vtables: &'a mut HashMap<PersistentId, VTable>,
}

impl TabViewer<'_> {
  fn vtable_of(&self, entity: Entity) -> VTable {
    let mut world = self.world.borrow_mut();
    let mut q_ids = world.query::<&PersistentId>();
    let id = q_ids.get(&world, entity).unwrap();
    self.vtables[id].clone()
  }
}

impl egui_dock::TabViewer for TabViewer<'_> {
  type Tab = Entity;

  fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
    let vtable = self.vtable_of(*tab);
    (vtable.title)(*tab, &mut self.world.borrow_mut())
  }

  fn clear_background(&self, tab: &Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    (vtable.can_clear)(*tab, &mut self.world.borrow_mut())
  }

  fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    (vtable.closable)(*tab, &mut self.world.borrow_mut())
  }

  fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    (vtable.on_close)(*tab, &mut self.world.borrow_mut());
    true
  }

  fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
    let vtable = self.vtable_of(*tab);
    (vtable.render)(*tab, ui, &mut self.world.borrow_mut());
  }

  fn context_menu(
    &mut self,
    ui: &mut egui::Ui,
    tab: &mut Self::Tab,
    _surface: egui_dock::SurfaceIndex,
    _node: NodeIndex,
  ) {
    let vtable = self.vtable_of(*tab);
    (vtable.context_menu)(*tab, ui, &mut self.world.borrow_mut());
  }

  fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
    egui::Id::new(tab)
  }
}
