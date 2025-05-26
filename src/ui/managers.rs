use super::{
  LayoutInfo, LayoutState, RawUi, TabViewer, VTable,
  misc::{DockExtensions, MissingUi, UiComponentExtensions},
  prebuilt::{
    assets::Assets, components::Components, debug::DebugMenu, editor_view::EditorView,
    hierarchy::Hierarchy, inspector::Inspector, menu_bar::MenuBar, prefabs::Prefabs,
    resources::Resources, type_browser::TypeBrowser,
  },
};
use crate::cache::Cache;
use bevy::{platform::collections::HashMap, prelude::*};
use bevy_egui::egui::{self};
use derive_new::new;
use egui_dock::{DockArea, DockState, NodeIndex, Surface, SurfaceIndex};
use persistent_id::PersistentId;
use std::{any::TypeId, cell::RefCell, collections::BTreeMap};

#[derive(Resource)]
pub(crate) struct UiManager {
  state: DockState<Entity>,

  vtables: HashMap<PersistentId, VTable>,

  id: egui::Id,
}

impl UiManager {
  pub fn new(app: &mut App) -> Self {
    let mut this = Self {
      state: DockState::new(Vec::new()),
      vtables: default(),
      id: egui::Id::new(TypeId::of::<Self>()),
    };

    this.register::<MissingUi>(app);

    this.register::<Assets>(app);
    this.register::<Components>(app);
    this.register::<DebugMenu>(app);
    this.register::<EditorView>(app);
    this.register::<Hierarchy>(app);
    this.register::<Inspector>(app);
    this.register::<MenuBar>(app);
    this.register::<Prefabs>(app);
    this.register::<Resources>(app);
    this.register::<TypeBrowser>(app);

    this
  }

  pub fn restore_or_init(&mut self, world: &mut World) {
    let (state, layouts) = world
      .resource_scope(|world, cache: Mut<Cache>| {
        cache.get::<LayoutState>().map(|layout| {
          (
            DockState::restore(&layout.dock, &self.vtables, world),
            layout.layouts,
          )
        })
      })
      .unwrap_or_else(|| (self.default_dock_state(world), default()));

    self.state = state;

    world.insert_resource(LayoutManager::new(layouts));
  }

  pub fn register<T: RawUi>(&mut self, app: &mut App) {
    T::init(app);
    app.register_type::<T>();
    self.vtables.insert(PersistentId(T::ID), T::VTABLE);
  }

  pub fn render(&mut self, world: &mut World) {
    let Ok(ctx) = world
      .query::<&mut bevy_egui::EguiContext>()
      .single_mut(world)
      .map(|mut ctx| ctx.get_mut().clone())
    else {
      return;
    };

    egui::CentralPanel::default()
      .frame(
        egui::Frame::central_panel(&ctx.style())
          // this makes it so the egui dock panels all surround the window's edges
          .inner_margin(0)
          // this allows the game to be rendered behind egui
          .fill(egui::Color32::TRANSPARENT),
      )
      .show(&ctx, |ui| {
        let mut tab_viewer = TabViewer {
          vtables: &mut self.vtables,
          world: RefCell::new(world),
        };

        DockArea::new(&mut self.state)
          .id(self.id)
          .show_add_buttons(true)
          .show_add_popup(true)
          .show_inside(ui, &mut tab_viewer);
      });
  }

  pub fn save_current_layout(
    &self,
    q_uuids: &Query<&PersistentId, Without<MissingUi>>,
    q_missing: &Query<&MissingUi>,
  ) -> DockState<LayoutInfo> {
    self.state.decouple(self, q_uuids, q_missing)
  }

  pub fn surface_mut(&mut self, index: SurfaceIndex) -> Option<&mut Surface<Entity>> {
    self.state.get_surface_mut(index)
  }

  pub(crate) fn vtables(&self) -> &HashMap<PersistentId, VTable> {
    &self.vtables
  }

  pub(super) fn vtable_of(&self, entity: Entity, world: &mut World) -> Option<&VTable> {
    let mut q_ids = world.query::<&PersistentId>();
    let id = q_ids.get(world, entity).unwrap();
    self.get_vtable_by_id(id)
  }

  pub(super) fn get_vtable_by_id(&self, id: &PersistentId) -> Option<&VTable> {
    self.vtables.get(id)
  }

  pub(crate) fn switch_state(&mut self, new_state: DockState<Entity>, world: &mut World) {
    for entity in self.state.iter_all_tabs().map(|(_, entity)| *entity) {
      if let Some(vtable) = self.vtable_of(entity, world) {
        (vtable.despawn)(entity, world);
      } else {
        world.despawn(entity);
      }
    }
    self.state = new_state;
  }

  pub(crate) fn default_dock_state(&self, world: &mut World) -> DockState<Entity> {
    let mut state = DockState::new(vec![self.spawn_type::<EditorView>(world)]);

    let tree = state.main_surface_mut();

    let root = NodeIndex::root();

    let tabs = vec![self.spawn_type::<MenuBar>(world)];
    let [central_panel, _top_bar] = tree.split_above(root, 0.1, tabs);

    let tabs = vec![
      self.spawn_type::<Hierarchy>(world),
      self.spawn_type::<DebugMenu>(world),
    ];
    let [central_panel, _left_panel] = tree.split_left(central_panel, 1.0 / 6.0, tabs);

    let tabs = vec![self.spawn_type::<Inspector>(world)];
    let [central_panel, _right_panel] = tree.split_right(central_panel, 4.0 / 5.0, tabs);

    let tabs = vec![
      self.spawn_type::<Prefabs>(world),
      self.spawn_type::<Components>(world),
      self.spawn_type::<Resources>(world),
      self.spawn_type::<Assets>(world),
    ];
    tree.split_below(central_panel, 0.7, tabs);

    state
  }

  fn spawn_type<T: RawUi>(&self, world: &mut World) -> Entity {
    self.spawn(PersistentId(T::ID), world)
  }

  fn spawn(&self, id: PersistentId, world: &mut World) -> Entity {
    (self.vtables[&id].spawn)(world)
  }

  pub fn state(&self) -> &DockState<Entity> {
    &self.state
  }
}

#[derive(new, Resource, Default, Deref, DerefMut)]
pub struct LayoutManager(BTreeMap<String, DockState<LayoutInfo>>);
