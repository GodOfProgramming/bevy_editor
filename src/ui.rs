use bevy::prelude::*;
use bevy::{
  asset::{ReflectAsset, UntypedAssetId},
  reflect::TypeRegistry,
};
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector::{
  self,
  hierarchy::{hierarchy_ui, SelectedEntities},
  ui_for_entities_shared_components, ui_for_entity_with_children,
};
use egui_dock::{DockArea, DockState, NodeIndex, Style};
use std::any::TypeId;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::Mutex;

pub(crate) type SpawnFn = Box<dyn FnMut(&mut World) + Send + Sync + 'static>;

#[derive(Eq, PartialEq)]
enum InspectorSelection {
  Entities,
  Resource(TypeId, String),
  Asset(TypeId, String, UntypedAssetId),
}

pub(crate) struct UiPlugin<C>
where
  C: Component,
{
  spawners: Mutex<RefCell<Vec<(String, SpawnFn)>>>,
  _cam_component: PhantomData<C>,
}

impl<C> UiPlugin<C>
where
  C: Component,
{
  pub fn new(spawners: Vec<(String, SpawnFn)>) -> Self {
    Self {
      spawners: Mutex::new(RefCell::new(spawners)),
      _cam_component: default(),
    }
  }
}

impl<C> Plugin for UiPlugin<C>
where
  C: Component,
{
  fn build(&self, app: &mut App) {
    let Ok(spawners_mx) = self.spawners.lock() else {
      error!("failed to acquire spawner mutex when building editor ui");
      return;
    };

    let spawners = spawners_mx.borrow_mut().drain(..).collect();

    app
      .insert_resource(State::<C>::new(spawners))
      .insert_resource(FileDialog::new());
  }
}

#[derive(Resource)]
struct FileDialog {
  dialog: egui::mutex::Mutex<egui_file_dialog::FileDialog>,
}

impl FileDialog {
  fn new() -> Self {
    Self {
      dialog: egui::mutex::Mutex::new(egui_file_dialog::FileDialog::new()),
    }
  }

  fn access_mut(&mut self, f: impl FnOnce(&mut egui_file_dialog::FileDialog)) {
    f(&mut self.dialog.lock());
  }
}

#[derive(Resource)]
pub(crate) struct State<C: Component> {
  pub(crate) viewport_rect: egui::Rect,
  pub(crate) selected_entities: SelectedEntities,
  dock_state: DockState<Tabs>,
  selection: InspectorSelection,
  cam_component: PhantomData<C>,
  spawners: Vec<(String, SpawnFn)>,
}

impl<C> State<C>
where
  C: Component,
{
  pub fn new(spawners: Vec<(String, SpawnFn)>) -> Self {
    let mut state = DockState::new(vec![Tabs::GameView]);
    let tree = state.main_surface_mut();
    let [game, _inspector] =
      tree.split_right(NodeIndex::root(), 0.75, vec![Tabs::Inspector, Tabs::Spawn]);
    let [game, _hierarchy] = tree.split_left(game, 0.2, vec![Tabs::Hierarchy, Tabs::Options]);
    let [_game, _bottom] = tree.split_below(game, 0.8, vec![Tabs::Resources, Tabs::Assets]);

    Self {
      viewport_rect: egui::Rect::NOTHING,
      selected_entities: SelectedEntities::default(),
      dock_state: state,
      selection: InspectorSelection::Entities,
      cam_component: default(),
      spawners,
    }
  }

  pub(crate) fn ui(&mut self, world: &mut World, ctx: &mut egui::Context) {
    let mut tab_viewer = TabViewer::<C> {
      world,
      viewport_rect: &mut self.viewport_rect,
      selected_entities: &mut self.selected_entities,
      selection: &mut self.selection,
      spawners: &mut self.spawners,
      cam_component: default(),
    };
    DockArea::new(&mut self.dock_state)
      .style(Style::from_egui(ctx.style().as_ref()))
      .show(ctx, &mut tab_viewer);
  }
}

#[derive(Debug)]
enum Tabs {
  GameView,
  Hierarchy,
  Options,
  Resources,
  Assets,
  Inspector,
  Spawn,
}

struct TabViewer<'a, C: Component> {
  world: &'a mut World,
  selected_entities: &'a mut SelectedEntities,
  selection: &'a mut InspectorSelection,
  viewport_rect: &'a mut egui::Rect,
  spawners: &'a mut Vec<(String, SpawnFn)>,
  cam_component: PhantomData<C>,
}

impl<C> TabViewer<'_, C>
where
  C: Component,
{
  fn select_resource(&mut self, ui: &mut egui::Ui, type_registry: &TypeRegistry) {
    let mut resources: Vec<_> = type_registry
      .iter()
      .filter(|registration| registration.data::<ReflectResource>().is_some())
      .map(|registration| {
        (
          registration.type_info().type_path_table().short_path(),
          registration.type_id(),
        )
      })
      .collect();
    resources.sort_by(|(name_a, _), (name_b, _)| name_a.cmp(name_b));

    for (resource_name, type_id) in resources {
      let selected = match *self.selection {
        InspectorSelection::Resource(selected, _) => selected == type_id,
        _ => false,
      };

      if ui.selectable_label(selected, resource_name).clicked() {
        *self.selection = InspectorSelection::Resource(type_id, resource_name.to_string());
      }
    }
  }

  fn select_asset(&mut self, ui: &mut egui::Ui, type_registry: &TypeRegistry) {
    let mut assets: Vec<_> = type_registry
      .iter()
      .filter_map(|registration| {
        let reflect_asset = registration.data::<ReflectAsset>()?;
        Some((
          registration.type_info().type_path_table().short_path(),
          registration.type_id(),
          reflect_asset,
        ))
      })
      .collect();
    assets.sort_by(|(name_a, ..), (name_b, ..)| name_a.cmp(name_b));

    for (asset_name, asset_type_id, reflect_asset) in assets {
      let handles: Vec<_> = reflect_asset.ids(self.world).collect();

      ui.collapsing(format!("{asset_name} ({})", handles.len()), |ui| {
        for handle in handles {
          let selected = match *self.selection {
            InspectorSelection::Asset(_, _, selected_id) => selected_id == handle,
            _ => false,
          };

          if ui
            .selectable_label(selected, format!("{:?}", handle))
            .clicked()
          {
            *self.selection =
              InspectorSelection::Asset(asset_type_id, asset_name.to_string(), handle);
          }
        }
      });
    }
  }

  fn options_ui(&mut self, ui: &mut egui_dock::egui::Ui) {
    let mut fd = self.world.resource_mut::<FileDialog>();

    if ui.button("Open Map").clicked() {
      fd.access_mut(|dlg| dlg.select_file());
    }

    fd.access_mut(|dlg| {
      dlg.update(ui.ctx());
      if let Some(path) = dlg.take_selected() {
        debug!("selected {}", path.display());
      }
    })
  }
}

impl<C> egui_dock::TabViewer for TabViewer<'_, C>
where
  C: Component,
{
  type Tab = Tabs;

  fn title(&mut self, window: &mut Self::Tab) -> egui_dock::egui::WidgetText {
    format!("{window:?}").into()
  }

  fn clear_background(&self, window: &Self::Tab) -> bool {
    !matches!(window, Tabs::GameView)
  }

  fn ui(&mut self, ui: &mut egui_dock::egui::Ui, tab: &mut Self::Tab) {
    let type_registry = self.world.resource::<AppTypeRegistry>().0.clone();
    let type_registry = type_registry.read();

    match tab {
      Tabs::GameView => {
        *self.viewport_rect = ui.clip_rect();
      }
      Tabs::Options => self.options_ui(ui),
      Tabs::Hierarchy => {
        let selected = hierarchy_ui(self.world, ui, self.selected_entities);
        if selected {
          *self.selection = InspectorSelection::Entities;
        }
      }
      Tabs::Resources => self.select_resource(ui, &type_registry),
      Tabs::Assets => self.select_asset(ui, &type_registry),
      Tabs::Inspector => match *self.selection {
        InspectorSelection::Entities => match self.selected_entities.as_slice() {
          &[entity] => ui_for_entity_with_children(self.world, entity, ui),
          entities => ui_for_entities_shared_components(self.world, entities, ui),
        },
        InspectorSelection::Resource(type_id, ref name) => {
          ui.label(name);
          bevy_inspector::by_type_id::ui_for_resource(self.world, type_id, ui, name, &type_registry)
        }
        InspectorSelection::Asset(type_id, ref name, handle) => {
          ui.label(name);
          bevy_inspector::by_type_id::ui_for_asset(self.world, type_id, handle, ui, &type_registry);
        }
      },
      Tabs::Spawn => {
        for (name, spawn_func) in self.spawners.iter_mut() {
          if ui.button(name.as_str()).clicked() {
            spawn_func(self.world);
          }
        }
      }
    }
  }
}
