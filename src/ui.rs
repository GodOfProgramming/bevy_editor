pub mod events;
pub mod managers;
pub mod misc;
pub mod prebuilt;

use crate::cache::{Cache, Saveable};
use bevy::{
  asset::UntypedAssetId, ecs::system::SystemParam, platform::collections::HashMap, prelude::*,
  reflect::GetTypeRegistration,
};
use bevy_egui::{
  EguiPlugin,
  egui::{self},
};
use bevy_inspector_egui::bevy_inspector;
use derive_more::derive::From;
use egui_dock::{DockState, NodeIndex, SurfaceIndex};
use events::{AddUiEvent, RemoveUiEvent, SaveLayoutEvent};
use itertools::{Either, Itertools};
use managers::UiManager;
use misc::{MissingUi, UiExtensions, UiInfo};
use parking_lot::Mutex;
use prebuilt::{
  assets::Assets, debug::DebugMenu, editor_view::EditorView, hierarchy::Hierarchy,
  inspector::Inspector, prefabs::Prefabs, resources::Resources,
};
use serde::{Deserialize, Serialize};
use std::{any::TypeId, borrow::BorrowMut, cell::RefCell, collections::BTreeMap};
use uuid::Uuid;

pub(crate) struct UiPlugin(pub Mutex<RefCell<Option<UiManager>>>);

impl Plugin for UiPlugin {
  fn build(&self, app: &mut App) {
    debug!("Building UI Plugin");

    let mut layout = self.0.lock();
    let layout = layout.borrow_mut();
    let ui_manager = layout.take().unwrap();

    app
      .register_type::<MissingUi>()
      .register_type::<EditorView>()
      .register_type::<Hierarchy>()
      .register_type::<DebugMenu>()
      .register_type::<Inspector>()
      .register_type::<Prefabs>()
      .register_type::<Resources>()
      .register_type::<Assets>()
      .add_event::<AddUiEvent>()
      .add_event::<RemoveUiEvent>()
      .add_event::<SaveLayoutEvent>()
      .init_resource::<InspectorSelection>()
      .add_plugins(EguiPlugin {
        enable_multipass_for_primary_context: false,
      })
      .add_systems(Startup, Self::init_resources)
      .add_systems(
        Update,
        (
          RemoveUiEvent::on_event,
          (
            (
              Self::dispatch_render_events,
              Self::reset_ui_info,
              Self::render,
            )
              .chain(),
            AddUiEvent::on_event,
          ),
        )
          .chain(),
      )
      .add_systems(FixedUpdate, SaveLayoutEvent::on_event);

    for vtable in ui_manager.vtables() {
      (vtable.init)(app);
    }

    app.insert_resource(ui_manager);
  }
}

impl UiPlugin {
  fn init_resources(world: &mut World) {
    world.spawn((Name::new("Editor Ui Panels"), UiPanels));
    world.resource_scope(|world, mut layout: Mut<UiManager>| {
      layout.restore_or_init(world);
    });
  }

  pub fn reset_ui_info(mut q_ui_infos: Query<&mut UiInfo>) {
    q_ui_infos.par_iter_mut().for_each(|mut ui_info| {
      ui_info.rendered = false;
    });
  }

  pub fn render(world: &mut World) {
    world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
      ui_manager.render(world);
    });
  }

  pub fn dispatch_render_events(world: &mut World) {
    let mut q_entities = world.query::<(Entity, &UiInfo)>();
    let (rendered, unrendered): (Vec<Entity>, Vec<Entity>) =
      q_entities.iter(world).partition_map(|(entity, ui_info)| {
        if ui_info.rendered {
          Either::Left(entity)
        } else {
          Either::Right(entity)
        }
      });

    world.resource_scope(|world, ui_manager: Mut<UiManager>| {
      for entity in rendered {
        let vtable = ui_manager.vtable_of(entity, world);
        (vtable.when_rendered)(entity, world);
      }

      for entity in unrendered {
        let vtable = ui_manager.vtable_of(entity, world);
        (vtable.when_not_rendered)(entity, world);
      }
    });
  }

  pub fn on_app_exit(
    mut cache: ResMut<Cache>,
    ui_manager: Res<UiManager>,
    q_uuids: Query<&PersistentId, Without<MissingUi>>,
    q_missing: Query<&MissingUi>,
  ) {
    let new_state = ui_manager.save_current_layout(&q_uuids, &q_missing);
    cache.store(&LayoutState {
      dock: new_state,
      layouts: ui_manager.saved_layouts().clone(),
    });
  }
}

pub trait RawUi: Component + GetTypeRegistration + Send + Sync + Sized {
  const NAME: &str;
  const ID: Uuid;

  /// Add systems or resources that this UI needs in order to function
  #[allow(unused_variables)]
  fn init(app: &mut App) {}

  fn spawn(entity: Entity, world: &mut World) -> Self;

  #[allow(unused_variables)]
  fn on_despawn(entity: Entity, world: &mut World) {}

  #[allow(unused_variables)]
  fn title(entity: Entity, world: &mut World) -> egui::WidgetText {
    Self::NAME.into()
  }

  fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World);

  #[allow(unused_variables)]
  fn when_rendered(entity: Entity, world: &mut World) {}

  #[allow(unused_variables)]
  fn when_not_rendered(entity: Entity, world: &mut World) {}

  #[allow(unused_variables)]
  fn context_menu(
    entity: Entity,
    ui: &mut egui::Ui,
    world: &mut World,
    surface: SurfaceIndex,
    node: NodeIndex,
  ) {
  }

  #[allow(unused_variables)]
  fn handle_tab_response(entity: Entity, world: &mut World, response: &egui::Response) {}

  #[allow(unused_variables)]
  fn closeable(entity: Entity, world: &mut World) -> bool {
    true
  }

  /// Used to prevent this Ui from appearing in the view menu
  ///
  /// Typically for Ui's that are programmatically created
  fn hidden() -> bool {
    false
  }

  #[allow(unused_variables)]
  fn can_clear(entity: Entity, world: &mut World) -> bool {
    true
  }

  fn unique() -> bool {
    false
  }

  fn popout() -> bool {
    true
  }
}

pub trait Ui: RawUi {
  const NAME: &str;
  const ID: Uuid;

  type Params<'w, 's>: for<'world, 'system> SystemParam<
    Item<'world, 'system> = Self::Params<'world, 'system>,
  >;

  /// Add systems or resources that this UI needs in order to function
  #[allow(unused_variables)]
  fn init(app: &mut App) {}

  fn spawn(params: Self::Params<'_, '_>) -> Self;

  #[allow(unused_variables)]
  fn title(&mut self, params: Self::Params<'_, '_>) -> egui::WidgetText {
    <Self as Ui>::NAME.into()
  }

  fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>);

  #[allow(unused_variables)]
  fn when_rendered(&mut self, params: Self::Params<'_, '_>) {}

  #[allow(unused_variables)]
  fn when_not_rendered(&mut self, params: Self::Params<'_, '_>) {}

  #[allow(unused_variables)]
  fn context_menu(
    &mut self,
    ui: &mut egui::Ui,
    params: Self::Params<'_, '_>,
    surface: SurfaceIndex,
    node: NodeIndex,
  ) {
  }

  #[allow(unused_variables)]
  fn handle_tab_response(&mut self, params: Self::Params<'_, '_>, response: &egui::Response) {}

  #[allow(unused_variables)]
  fn closeable(&self, params: Self::Params<'_, '_>) -> bool {
    true
  }

  #[allow(unused_variables)]
  fn on_despawn(&mut self, params: Self::Params<'_, '_>) {}

  /// Used to prevent this Ui from appearing in the view menu
  ///
  /// Typically for Ui's that are programmatically created
  fn hidden() -> bool {
    false
  }

  #[allow(unused_variables)]
  fn can_clear(&self, params: Self::Params<'_, '_>) -> bool {
    true
  }

  fn unique() -> bool {
    false
  }

  fn popout() -> bool {
    true
  }
}

impl<T> RawUi for T
where
  T: Ui + 'static,
{
  const NAME: &str = <Self as Ui>::NAME;
  const ID: Uuid = <T as Ui>::ID;

  fn init(app: &mut App) {
    <Self as Ui>::init(app)
  }

  fn spawn(entity: Entity, world: &mut World) -> Self {
    Self::register_params(entity, world);
    Self::with_params(entity, world, Ui::spawn)
  }

  fn title(entity: Entity, world: &mut World) -> egui::WidgetText {
    Self::get_entity_mut(entity, world, Ui::title)
  }

  fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    Self::get_entity_mut(entity, world, |this, params| {
      this.render(ui, params);
    })
  }

  fn when_rendered(entity: Entity, world: &mut World) {
    Self::get_entity_mut(entity, world, <Self as Ui>::when_rendered)
  }

  fn when_not_rendered(entity: Entity, world: &mut World) {
    Self::get_entity_mut(entity, world, <Self as Ui>::when_not_rendered)
  }

  fn context_menu(
    entity: Entity,
    ui: &mut egui::Ui,
    world: &mut World,
    surface: SurfaceIndex,
    node: NodeIndex,
  ) {
    Self::get_entity_mut(entity, world, |this, params| {
      this.context_menu(ui, params, surface, node);
    })
  }

  fn closeable(entity: Entity, world: &mut World) -> bool {
    Self::get_entity(entity, world, Ui::closeable)
  }

  fn on_despawn(entity: Entity, world: &mut World) {
    Self::get_entity_mut(entity, world, <Self as Ui>::on_despawn)
  }

  fn handle_tab_response(entity: Entity, world: &mut World, response: &egui::Response) {
    Self::get_entity_mut(entity, world, |this, params| {
      this.handle_tab_response(params, response);
    });
  }

  fn hidden() -> bool {
    <Self as Ui>::hidden()
  }

  fn can_clear(entity: Entity, world: &mut World) -> bool {
    Self::get_entity(entity, world, Ui::can_clear)
  }

  fn unique() -> bool {
    <Self as Ui>::unique()
  }

  fn popout() -> bool {
    <Self as Ui>::popout()
  }
}

#[derive(Clone)]
struct VTable {
  name: fn() -> &'static str,
  init: fn(&mut App),
  spawn: fn(&mut World) -> Entity,
  despawn: fn(Entity, &mut World),
  title: fn(Entity, &mut World) -> egui::WidgetText,
  render: fn(Entity, &mut egui::Ui, &mut World),
  when_rendered: fn(Entity, &mut World),
  when_not_rendered: fn(Entity, &mut World),
  context_menu: fn(Entity, &mut egui::Ui, &mut World, SurfaceIndex, NodeIndex),
  handle_tab_response: fn(Entity, &mut World, &egui::Response),
  closeable: fn(Entity, &mut World) -> bool,
  hidden: fn() -> bool,
  can_clear: fn(Entity, &mut World) -> bool,
  unique: fn() -> bool,
  popout: fn() -> bool,
  count: fn(&mut World) -> usize,
}

impl VTable {
  const fn new<T>() -> Self
  where
    T: RawUi,
  {
    Self {
      name: || T::NAME,
      init: T::init,
      spawn: Self::spawn::<T>,
      despawn: Self::despawn::<T>,
      title: T::title,
      render: T::render,
      when_rendered: T::when_rendered,
      when_not_rendered: T::when_not_rendered,
      context_menu: T::context_menu,
      handle_tab_response: T::handle_tab_response,
      closeable: T::closeable,
      hidden: T::hidden,
      can_clear: T::can_clear,
      unique: T::unique,
      popout: T::popout,
      count: Self::count::<T>,
    }
  }

  fn spawn<T: RawUi>(world: &mut World) -> Entity {
    info!("Spawning UI component {}", T::NAME);
    let entity = world
      .spawn((Name::new(T::NAME), PersistentId(T::ID), UiInfo::default()))
      .id();

    let ui_scene = world
      .query_filtered::<Entity, With<UiPanels>>()
      .iter(world)
      .next()
      .unwrap();
    world.entity_mut(ui_scene).add_child(entity);

    let instance = T::spawn(entity, world);
    world.entity_mut(entity).insert(instance).id()
  }

  fn despawn<T: RawUi>(entity: Entity, world: &mut World) {
    info!("Despawning UI component {}", T::NAME);
    <T as RawUi>::on_despawn(entity, world);
    world.send_event(RemoveUiEvent::new(entity));
  }

  fn count<T: RawUi>(world: &mut World) -> usize {
    let mut q_uis = world.query::<&T>();
    q_uis.iter(world).len()
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

  fn ui_info(&self, entity: Entity, f: impl FnOnce(&mut UiInfo)) {
    let mut world = self.world.borrow_mut();
    let mut q_ids = world.query::<&mut UiInfo>();
    let mut ui_info = q_ids.get_mut(&mut world, entity).ok();
    let ui_info = ui_info.as_deref_mut();
    if let Some(ui_info) = ui_info {
      f(ui_info);
    }
  }
}

impl egui_dock::TabViewer for TabViewer<'_> {
  type Tab = Entity;

  fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
    let vtable = self.vtable_of(*tab);
    (vtable.title)(*tab, &mut self.world.borrow_mut())
  }

  #[profiling::function]
  fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
    let vtable = self.vtable_of(*tab);
    (vtable.render)(*tab, ui, &mut self.world.borrow_mut());

    self.ui_info(*tab, |ui_info| {
      ui_info.hovered = ui.ui_contains_pointer();
      ui_info.rendered = true;
    });
  }

  fn add_popup(&mut self, ui: &mut egui::Ui, surface: SurfaceIndex, node: NodeIndex) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    let unique_tabs = self
      .vtables
      .iter()
      .filter(|(_, vtable)| (vtable.unique)() && !(vtable.hidden)())
      .map(|(id, vtable)| (id, (vtable.name)()))
      .sorted_by(|(_, a), (_, b)| a.cmp(b));

    for (id, name) in unique_tabs {
      let vtable = &self.vtables[id];
      let mut world = self.world.borrow_mut();
      let count = (vtable.count)(&mut world);

      let mut exists = count > 0;
      let enabled = !exists;

      ui.add_enabled_ui(enabled, |ui| {
        if ui.checkbox(&mut exists, name).clicked() {
          let entity = (vtable.spawn)(&mut world);
          world.send_event(AddUiEvent::new(surface, node, entity));
        }
      });
    }

    let spawnable_tables = self
      .vtables
      .iter()
      .filter(|(_, vtable)| !(vtable.unique)())
      .map(|(id, vtable)| (id, (vtable.name)()))
      .sorted_by(|(_, a), (_, b)| a.cmp(b));

    if spawnable_tables.len() > 0 {
      for (id, name) in spawnable_tables {
        let vtable = &self.vtables[id];
        if ui.button(name).clicked() {
          let mut world = self.world.borrow_mut();
          let entity = (vtable.spawn)(&mut world);
          world.send_event(AddUiEvent::new(surface, node, entity));
          ui.memory_mut(|mem| mem.close_popup());
        }
      }
    }
  }

  fn context_menu(
    &mut self,
    ui: &mut egui::Ui,
    tab: &mut Self::Tab,
    surface: SurfaceIndex,
    node: NodeIndex,
  ) {
    let vtable = self.vtable_of(*tab);
    (vtable.context_menu)(*tab, ui, &mut self.world.borrow_mut(), surface, node);
  }

  fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
    let vtable = self.vtable_of(*tab);
    (vtable.handle_tab_response)(*tab, &mut self.world.borrow_mut(), response)
  }

  fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    (vtable.closeable)(*tab, &mut self.world.borrow_mut())
  }

  fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    (vtable.despawn)(*tab, &mut self.world.borrow_mut());
    true
  }

  fn clear_background(&self, tab: &Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    (vtable.can_clear)(*tab, &mut self.world.borrow_mut())
  }

  fn allowed_in_windows(&self, tab: &mut Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    (vtable.popout)()
  }

  fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
    egui::Id::new(tab)
  }
}

#[derive(Serialize, Deserialize)]
struct LayoutState {
  dock: DockState<Uuid>,
  layouts: BTreeMap<String, DockState<Uuid>>,
}

impl Saveable for LayoutState {
  const KEY: &str = "Layout";
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

#[derive(Default, Deref, DerefMut, Component, Clone, Copy, Hash, PartialEq, Eq, Reflect, From)]
pub struct PersistentId(#[reflect(ignore)] pub Uuid);

/// Component that stores all ui components as children for organization
#[derive(Component)]
pub struct UiPanels;
