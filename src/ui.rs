pub mod events;
pub mod managers;
pub mod misc;

pub mod prebuilt;

use crate::cache::{Cache, Saveable};
use bevy::{
  asset::UntypedAssetId, ecs::system::SystemParam, prelude::*, reflect::GetTypeRegistration,
  utils::HashMap,
};
use bevy_egui::{
  egui::{self},
  EguiPlugin,
};
use bevy_inspector_egui::bevy_inspector;
use derive_more::derive::From;
use egui_dock::{DockState, NodeIndex, SurfaceIndex};
use events::{AddUiEvent, RemoveUiEvent, SaveLayoutEvent};
use itertools::Itertools;
use managers::UiManager;
use misc::{MissingUi, UiExtensions};
use parking_lot::Mutex;
use prebuilt::{
  assets::Assets, control_panel::ControlPanel, editor_view::EditorView, hierarchy::Hierarchy,
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
      .register_type::<ControlPanel>()
      .register_type::<Inspector>()
      .register_type::<Prefabs>()
      .register_type::<Resources>()
      .register_type::<Assets>()
      .add_event::<AddUiEvent>()
      .add_event::<RemoveUiEvent>()
      .add_event::<SaveLayoutEvent>()
      .init_resource::<InspectorSelection>()
      .add_plugins(EguiPlugin)
      .add_systems(Startup, Self::init_resources)
      .add_systems(
        Update,
        (
          RemoveUiEvent::on_event,
          (Self::render, AddUiEvent::on_event),
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
    world.resource_scope(|world, mut layout: Mut<UiManager>| {
      layout.restore_or_init(world);
    });
  }

  pub fn render(world: &mut World) {
    world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
      ui_manager.render(world);
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

  fn spawn(world: &mut World) -> Self;

  /// Used to prevent this Ui from appearing in the view menu
  ///
  /// Typically for Ui's that are programmatically created
  fn hidden() -> bool {
    false
  }

  #[allow(unused_variables)]
  fn title(entity: Entity, world: &mut World) -> egui::WidgetText {
    Self::NAME.into()
  }

  #[allow(unused_variables)]
  fn can_clear(entity: Entity, world: &mut World) -> bool {
    true
  }

  #[allow(unused_variables)]
  fn closeable(world: &mut World) -> bool {
    true
  }

  #[allow(unused_variables)]
  fn on_close(entity: Entity, world: &mut World) {}

  fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World);

  #[allow(unused_variables)]
  fn context_menu(entity: Entity, ui: &mut egui::Ui, world: &mut World) {}

  fn unique() -> bool {
    false
  }
}

#[derive(SystemParam)]
pub struct NoParams;

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

  /// Used to prevent this Ui from appearing in the view menu
  ///
  /// Typically for Ui's that are programmatically created
  fn hidden() -> bool {
    false
  }

  #[allow(unused_variables)]
  fn title(&mut self, params: Self::Params<'_, '_>) -> egui::WidgetText {
    <Self as Ui>::NAME.into()
  }

  #[allow(unused_variables)]
  fn can_clear(&self, params: Self::Params<'_, '_>) -> bool {
    true
  }

  #[allow(unused_variables)]
  fn closeable() -> bool {
    true
  }

  #[allow(unused_variables)]
  fn on_close(&mut self, params: Self::Params<'_, '_>) {}

  fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>);

  #[allow(unused_variables)]
  fn context_menu(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {}

  fn unique() -> bool {
    false
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

  fn spawn(world: &mut World) -> Self {
    let entity = world.spawn_empty().id();
    Self::register_params(entity, world);
    Self::with_params(entity, world, Ui::spawn)
  }

  fn hidden() -> bool {
    <Self as Ui>::hidden()
  }

  fn title(entity: Entity, world: &mut World) -> egui::WidgetText {
    Self::get_entity_mut(entity, world, Ui::title)
  }

  fn can_clear(entity: Entity, world: &mut World) -> bool {
    Self::get_entity(entity, world, Ui::can_clear)
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

  fn unique() -> bool {
    <Self as Ui>::unique()
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

#[derive(Default, Deref, DerefMut, Component, Clone, Copy, Hash, PartialEq, Eq, Reflect, From)]
pub struct PersistentId(#[reflect(ignore)] pub Uuid);

#[derive(Clone)]
struct VTable {
  name: fn() -> &'static str,
  init: fn(&mut App),
  spawn: fn(&mut World) -> Entity,
  title: fn(Entity, &mut World) -> egui::WidgetText,
  can_clear: fn(Entity, &mut World) -> bool,
  on_close: fn(Entity, &mut World),
  render: fn(Entity, &mut egui::Ui, &mut World),
  context_menu: fn(Entity, &mut egui::Ui, &mut World),
  unique: fn() -> bool,
  hidden: fn() -> bool,
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
      spawn: |world| {
        let instance = T::spawn(world);
        let entity_id = world.spawn((instance, PersistentId(T::ID))).id();
        world.entity_mut(entity_id).insert(Name::new(T::NAME));
        info!("Spawned UI component {}", T::NAME);
        entity_id
      },
      title: T::title,
      can_clear: T::can_clear,
      on_close: T::on_close,
      render: T::render,
      context_menu: T::context_menu,
      unique: T::unique,
      hidden: T::hidden,
      count: |world| {
        let mut q_uis = world.query::<&T>();
        q_uis.iter(world).len()
      },
    }
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

  fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    (vtable.on_close)(*tab, &mut self.world.borrow_mut());

    let mut world = self.world.borrow_mut();
    world.send_event(RemoveUiEvent::new(*tab));

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
    surface: SurfaceIndex,
    node: NodeIndex,
  ) {
    ui.menu_button("View", |ui| {
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
        ui.menu_button("Insert", |ui| {
          for (id, name) in spawnable_tables {
            let vtable = &self.vtables[id];
            if ui.button(name).clicked() {
              let mut world = self.world.borrow_mut();
              let entity = (vtable.spawn)(&mut world);
              world.send_event(AddUiEvent::new(surface, node, entity));
            }
          }
        });
      }
    });

    let vtable = self.vtable_of(*tab);
    (vtable.context_menu)(*tab, ui, &mut self.world.borrow_mut());
  }

  fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
    egui::Id::new(tab)
  }
}
