use super::{
  events::SaveLayoutEvent,
  misc::{DockExtensions, MissingUi, UiComponentExtensions},
  prebuilt::{
    assets::Assets, control_panel::ControlPanel, editor_view::EditorView, hierarchy::Hierarchy,
    inspector::Inspector, prefabs::Prefabs, resources::Resources,
  },
  LayoutState, PersistentId, TabViewer, UiComponent, VTable,
};
use crate::cache::Cache;
use bevy::{
  prelude::*,
  utils::{hashbrown::hash_map, HashMap},
};
use bevy_egui::egui::{self, Align2, TextBuffer};
use egui_dock::{DockArea, DockState, NodeIndex, Surface, SurfaceIndex};
use std::{any::TypeId, cell::RefCell, collections::BTreeMap};
use uuid::Uuid;

#[derive(Resource)]
pub(crate) struct UiManager {
  state: DockState<Entity>,

  vtables: HashMap<PersistentId, VTable>,

  layout_manager: LayoutManager,

  id: egui::Id,
}

impl Default for UiManager {
  fn default() -> Self {
    let mut this = Self {
      state: DockState::new(Vec::new()),
      vtables: default(),
      id: egui::Id::new(TypeId::of::<Self>()),
      layout_manager: default(),
    };

    this.register::<MissingUi>();
    this.register::<EditorView>();
    this.register::<Hierarchy>();
    this.register::<ControlPanel>();
    this.register::<Inspector>();
    this.register::<Prefabs>();
    this.register::<Resources>();
    this.register::<Assets>();

    this
  }
}

impl UiManager {
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
      .unwrap_or_else(|| {
        let mut state = DockState::new(vec![self.spawn_type::<EditorView>(world)]);

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

        (state, default())
      });

    self.state = state;
    self.layout_manager.layouts = layouts;
  }

  pub fn register<T: UiComponent>(&mut self) {
    self.vtables.insert(T::ID, T::VTABLE);
  }

  pub fn render(&mut self, world: &mut World) {
    let Ok(ctx) = world
      .query::<&mut bevy_egui::EguiContext>()
      .get_single_mut(world)
      .map(|ctx| ctx.get().clone())
    else {
      return;
    };

    let mut save_clicked = false;
    egui::Window::new("Save Layout")
      .open(&mut self.layout_manager.show_save_layout_modal)
      .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
      .title_bar(true)
      .resizable(false)
      .movable(false)
      .collapsible(false)
      .show(&ctx, |ui| {
        ui.horizontal(|ui| {
          ui.label("Name");
          ui.text_edit_singleline(&mut self.layout_manager.save_name_text);
        });

        if ui.button("Save").clicked() {
          save_clicked = true;
          let name = self.layout_manager.save_name_text.take();
          world.send_event(SaveLayoutEvent::new(name, self.state.clone()));
        }
      });

    if save_clicked {
      self.layout_manager.show_save_layout_modal = false;
    }

    egui::CentralPanel::default()
      .frame(
        egui::Frame::central_panel(&ctx.style())
          .inner_margin(0.0)
          .fill(egui::Color32::TRANSPARENT),
      )
      .show(&ctx, |ui| {
        egui::menu::bar(ui, |ui| {
          self.menu_bar_ui(ui, world);
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

  pub(super) fn vtables(&self) -> hash_map::Values<'_, PersistentId, VTable> {
    self.vtables.values()
  }

  pub fn save_current_layout(
    &self,
    q_uuids: &Query<&PersistentId, Without<MissingUi>>,
    q_missing: &Query<&MissingUi>,
  ) -> DockState<Uuid> {
    self.state.decouple(q_uuids, q_missing)
  }

  pub fn save_layout(&mut self, name: impl Into<String>, dock: DockState<Uuid>) {
    self.layout_manager.layouts.insert(name.into(), dock);
  }

  pub fn saved_layouts(&self) -> &BTreeMap<String, DockState<Uuid>> {
    &self.layout_manager.layouts
  }

  pub fn surface_mut(&mut self, index: SurfaceIndex) -> Option<&mut Surface<Entity>> {
    self.state.get_surface_mut(index)
  }

  fn spawn_type<T: UiComponent>(&self, world: &mut World) -> Entity {
    self.spawn(T::ID, world)
  }

  fn spawn(&self, id: PersistentId, world: &mut World) -> Entity {
    (self.vtables[&id].spawn)(world)
  }

  fn menu_bar_ui(&mut self, ui: &mut egui::Ui, world: &mut World) {
    ui.menu_button("Tools", |ui| {
      if ui.button("Generate UUID").clicked() {
        ui.output_mut(|output| {
          output.copied_text = Uuid::new_v4().to_string();
        });
      }
    });

    ui.menu_button("View", |ui| {
      if ui.button("Save Layout").clicked() {
        self.layout_manager.save_name_text.clear();
        self.layout_manager.show_save_layout_modal = true;
      }

      if !self.layout_manager.layouts.is_empty() {
        ui.menu_button("Restore", |ui| {
          for (layout, dock) in &self.layout_manager.layouts {
            if ui.button(layout).clicked() {
              let dock = DockState::restore(dock, &self.vtables, world);
              for (_, entity) in self.state.iter_all_tabs() {
                world.despawn(*entity);
              }
              self.state = dock;
            }
          }
        });
      }
    });
  }
}

#[derive(Default)]
struct LayoutManager {
  save_name_text: String,
  show_save_layout_modal: bool,
  layouts: BTreeMap<String, DockState<Uuid>>,
}
