use crate::assets::Prefabs;
use crate::scenes::{LoadEvent, SaveEvent};
use bevy::prelude::*;
use bevy::{
  asset::{ReflectAsset, UntypedAssetId},
  reflect::TypeRegistry,
};
use bevy_egui::{egui, EguiPlugin};
use bevy_inspector_egui::bevy_inspector::{
  self,
  hierarchy::{hierarchy_ui, SelectedEntities},
  ui_for_entities_shared_components, ui_for_entity_with_children,
};
use egui_dock::{DockArea, DockState, NodeIndex, Style};
use std::any::TypeId;
use std::path::PathBuf;

pub fn render(world: &mut World) {
  world.resource_scope(|world, mut ui_state: Mut<State>| {
    ui_state.ui(world);
  });
}

#[derive(Eq, PartialEq)]
enum InspectorSelection {
  Entities,
  Resource(TypeId, String),
  Asset(TypeId, String, UntypedAssetId),
}

pub(crate) struct UiPlugin;

impl Plugin for UiPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins(EguiPlugin)
      .insert_resource(State::new())
      .insert_resource(FileDialog::new());
  }
}

#[derive(Resource)]
pub(crate) struct State {
  pub(crate) viewport_rect: egui::Rect,
  selected_entities: SelectedEntities,
  dock_state: DockState<Tabs>,
  selection: InspectorSelection,
  on_file_select: Option<fn(&mut World, PathBuf)>,
}

impl State {
  pub fn new() -> Self {
    Self {
      viewport_rect: egui::Rect::NOTHING,
      selected_entities: SelectedEntities::default(),
      dock_state: Self::build_dock(),
      selection: InspectorSelection::Entities,
      on_file_select: None,
    }
  }

  pub fn add_selected(&mut self, entity: Entity, add: bool) {
    self.selected_entities.select_maybe_add(entity, add);
    self.selection = InspectorSelection::Entities;
  }

  pub(crate) fn ui(&mut self, world: &mut World) {
    let Ok(mut ctx) = world
      .query::<&mut bevy_egui::EguiContext>()
      .get_single_mut(world)
    else {
      return;
    };

    let ctx = ctx.get_mut().clone();

    let mut tab_viewer = TabViewer {
      world,
      viewport_rect: &mut self.viewport_rect,
      selected_entities: &mut self.selected_entities,
      selection: &mut self.selection,
      on_file_select: &mut self.on_file_select,
    };

    DockArea::new(&mut self.dock_state)
      .style(Style::from_egui(ctx.style().as_ref()))
      .show(&ctx, &mut tab_viewer);
  }

  fn build_dock() -> DockState<Tabs> {
    let mut state = DockState::new(vec![Tabs::GameView]);

    let tree = state.main_surface_mut();

    let node = NodeIndex::root();

    // [Menubar]
    // [GameView]
    let node = tree.split_above(node, 0.1, vec![Tabs::MenuBar])[0];

    // [Menubar]
    // [Hierarchy | GameView]
    let node = tree.split_left(node, 1.0 / 6.0, vec![Tabs::Hierarchy])[0];

    // [Menubar]
    // [Hierarchy | Game | Inspector]
    let node = tree.split_right(node, 4.0 / 5.0, vec![Tabs::Inspector])[0];

    // [Menubar]
    // [Hierarchy | Game | Inspector]
    // [Prefabs/Resources/Assets]
    tree.split_below(
      node,
      0.7,
      vec![Tabs::Prefabs, Tabs::Resources, Tabs::Assets],
    );

    state
  }
}

#[derive(Debug)]
enum Tabs {
  MenuBar,
  GameView,
  Hierarchy,
  Prefabs,
  Resources,
  Assets,
  Inspector,
}

struct TabViewer<'a> {
  world: &'a mut World,
  selected_entities: &'a mut SelectedEntities,
  selection: &'a mut InspectorSelection,
  viewport_rect: &'a mut egui::Rect,
  on_file_select: &'a mut Option<fn(&mut World, PathBuf)>,
}

impl TabViewer<'_> {
  fn menu_bar_ui(&mut self, ui: &mut egui::Ui) {
    self.world.resource_scope(|world, mut fd: Mut<FileDialog>| {
      ui.horizontal(|ui| {
        ui.menu_button("File", |ui| {
          if ui.button("Save").clicked() {
            fd.access_mut(|dlg| dlg.save_file());
            *self.on_file_select = Some(Self::on_save);
          }

          if ui.button("Open Map").clicked() {
            fd.access_mut(|dlg| dlg.select_file());
            *self.on_file_select = Some(Self::on_load);
          }
        });
      });

      fd.access_mut(|dlg| {
        dlg.update(ui.ctx());
        if let Some(path) = dlg.take_selected() {
          if let Some(on_file_select) = self.on_file_select.take() {
            (on_file_select)(world, path);
          }
        }
      })
    });
  }

  fn prefab_ui(&mut self, ui: &mut egui::Ui) {
    self
      .world
      .resource_scope(|world, mut prefabs: Mut<Prefabs>| {
        let mut prefab_ids = prefabs.keys().cloned().collect::<Vec<_>>();

        prefab_ids.sort();

        for id in prefab_ids {
          ui.horizontal(|ui| {
            ui.label(&id);
            if ui.button("Spawn").clicked() {
              prefabs.spawn(id, world);
            }
          });
        }
      });
  }

  fn resource_ui(&mut self, ui: &mut egui::Ui, type_registry: &TypeRegistry) {
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

  fn asset_ui(&mut self, ui: &mut egui::Ui, type_registry: &TypeRegistry) {
    let mut assets = type_registry
      .iter()
      .filter_map(|registration| {
        let reflect_asset = registration.data::<ReflectAsset>()?;
        Some((
          registration.type_info().type_path_table().short_path(),
          registration.type_id(),
          reflect_asset,
        ))
      })
      .collect::<Vec<_>>();

    assets.sort_by(|(name_a, ..), (name_b, ..)| name_a.cmp(name_b));

    for (asset_name, asset_type_id, reflect_asset) in assets {
      let handles = reflect_asset.ids(self.world).collect::<Vec<_>>();

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

  fn on_save(world: &mut World, path: PathBuf) {
    world.send_event(SaveEvent::new(path));
  }

  fn on_load(world: &mut World, path: PathBuf) {
    world.send_event(LoadEvent::new(path));
  }
}

impl egui_dock::TabViewer for TabViewer<'_> {
  type Tab = Tabs;

  fn title(&mut self, window: &mut Self::Tab) -> egui::WidgetText {
    format!("{window:?}").into()
  }

  fn clear_background(&self, window: &Self::Tab) -> bool {
    !matches!(window, Tabs::GameView)
  }

  fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
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
        if hierarchy_ui(self.world, ui, self.selected_entities) {
          *self.selection = InspectorSelection::Entities;
        }
      }
      Tabs::Prefabs => {
        self.prefab_ui(ui);
      }
      Tabs::Resources => {
        self.resource_ui(ui, &type_registry);
      }
      Tabs::Assets => {
        self.asset_ui(ui, &type_registry);
      }
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
    }
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
