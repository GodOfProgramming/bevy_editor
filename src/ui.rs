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
use std::marker::PhantomData;

use crate::SaveEvent;

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
  _cam_component: PhantomData<C>,
}

impl<C> UiPlugin<C>
where
  C: Component,
{
  pub fn new() -> Self {
    Self {
      _cam_component: default(),
    }
  }
}

impl<C> Plugin for UiPlugin<C>
where
  C: Component,
{
  fn build(&self, app: &mut App) {
    app
      .insert_resource(State::<C>::new())
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
  selected_entities: SelectedEntities,
  dock_state: DockState<Tabs>,
  selection: InspectorSelection,
  cam_component: PhantomData<C>,
}

impl<C> State<C>
where
  C: Component,
{
  pub fn new() -> Self {
    let mut state = DockState::new(vec![Tabs::GameView]);
    let tree = state.main_surface_mut();
    let [game_view, _menu_bar] = tree.split_above(NodeIndex::root(), 0.1, vec![Tabs::MenuBar]);
    let [game_view, _inspector] =
      tree.split_right(game_view, 0.75, vec![Tabs::Inspector, Tabs::Spawn]);
    let [game_view, _level_info] = tree.split_left(game_view, 0.2, vec![Tabs::Hierarchy]);
    let [_game, _game_object_tray] =
      tree.split_below(game_view, 0.8, vec![Tabs::Resources, Tabs::Assets]);

    Self {
      viewport_rect: egui::Rect::NOTHING,
      selected_entities: SelectedEntities::default(),
      dock_state: state,
      selection: InspectorSelection::Entities,
      cam_component: default(),
    }
  }

  pub fn add_selected(&mut self, entity: Entity, add: bool) {
    self.selected_entities.select_maybe_add(entity, add);
  }

  pub(crate) fn ui(&mut self, world: &mut World, ctx: &mut egui::Context) {
    let mut tab_viewer = TabViewer::<C> {
      world,
      viewport_rect: &mut self.viewport_rect,
      selected_entities: &mut self.selected_entities,
      selection: &mut self.selection,
      cam_component: default(),
    };

    DockArea::new(&mut self.dock_state)
      .style(Style::from_egui(ctx.style().as_ref()))
      .show(ctx, &mut tab_viewer);
  }
}

#[derive(Debug)]
enum Tabs {
  MenuBar,
  GameView,
  Hierarchy,
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

  fn menu_bar_ui(&mut self, ui: &mut egui_dock::egui::Ui) {
    self.world.resource_scope(|world, mut fd: Mut<FileDialog>| {
      ui.horizontal(|ui| {
        ui.menu_button("File", |ui| {
          if ui.button("Save").clicked() {
            world.send_event(SaveEvent);
          }

          if ui.button("Open Map").clicked() {
            fd.access_mut(|dlg| dlg.select_file());
          }
        });
      });

      fd.access_mut(|dlg| {
        dlg.update(ui.ctx());
        if let Some(path) = dlg.take_selected() {
          info!("selected {}", path.display());
        }
      })
    });
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
      Tabs::MenuBar => {
        self.menu_bar_ui(ui);
      }
      Tabs::GameView => {
        *self.viewport_rect = ui.clip_rect();
      }
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
        ui.label("TODO");
      }
    }
  }
}
