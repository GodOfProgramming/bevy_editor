use crate::assets::Prefabs;
use crate::view::ViewState;
use crate::{LogInfo, WorldExtensions};
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
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
use std::marker::PhantomData;

#[derive(Resource)]
pub struct SystemGraph<L: ScheduleLabel> {
  graph: String,
  _pd: PhantomData<L>,
}

impl<L> SystemGraph<L>
where
  L: ScheduleLabel,
{
  pub fn new(app: &mut App, schedule: L) -> Self {
    let graph = bevy_mod_debugdump::schedule_graph_dot(
      app,
      schedule,
      &bevy_mod_debugdump::schedule_graph::Settings::default(),
    );

    Self {
      graph,
      _pd: default(),
    }
  }
}

#[derive(Resource)]
pub struct CustomTab(pub fn(&mut World, &mut egui::Ui));

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

impl UiPlugin {
  fn on_start(mut commands: Commands, custom_tab: Option<Res<CustomTab>>) {
    commands.insert_resource(State::new(custom_tab.is_some()));
  }
}

impl Plugin for UiPlugin {
  fn build(&self, app: &mut App) {
    debug!("Building UI Plugin");
    app
      .add_plugins(EguiPlugin)
      .add_systems(Startup, Self::on_start)
      .add_systems(Update, render);
  }
}

#[derive(Resource)]
pub(crate) struct State {
  pub(crate) viewport_rect: egui::Rect,
  selected_entities: SelectedEntities,
  dock_state: DockState<Tabs>,
  selection: InspectorSelection,
}

impl State {
  pub fn new(have_custom: bool) -> Self {
    Self {
      viewport_rect: egui::Rect::NOTHING,
      selected_entities: SelectedEntities::default(),
      dock_state: Self::build_dock(have_custom),
      selection: InspectorSelection::Entities,
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
    };

    DockArea::new(&mut self.dock_state)
      .style(Style::from_egui(ctx.style().as_ref()))
      .show(&ctx, &mut tab_viewer);
  }

  fn build_dock(have_custom: bool) -> DockState<Tabs> {
    let base_tabs = have_custom
      .then(|| vec![Tabs::GameView, Tabs::Custom])
      .unwrap_or_else(|| vec![Tabs::GameView]);
    let mut state = DockState::new(base_tabs);

    let tree = state.main_surface_mut();

    let game_view = NodeIndex::root();

    let [game_view, hierarchy] = tree.split_left(game_view, 1.0 / 6.0, vec![Tabs::Hierarchy]);

    let [_hierarchy, _control_panel] = tree.split_above(hierarchy, 0.1, vec![Tabs::ControlPanel]);

    let [game_view, _inspector] = tree.split_right(game_view, 4.0 / 5.0, vec![Tabs::Inspector]);

    tree.split_below(
      game_view,
      0.7,
      vec![Tabs::Prefabs, Tabs::Resources, Tabs::Assets],
    );

    state
  }
}

#[derive(Debug)]
enum Tabs {
  ControlPanel,
  GameView,
  Custom,
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
}

impl TabViewer<'_> {
  fn control_panel_ui(&mut self, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
      match self.world.get_state() {
        crate::EditorState::Editing => {
          if ui.button("▶").clicked() {
            self.world.set_state(crate::EditorState::Testing);
          }

          let mut view = self.world.get_state::<ViewState>();
          let prev_view = view;
          ui.push_id("view-selector", |ui| {
            bevy_inspector::ui_for_value(&mut view, ui, self.world);
          });
          if prev_view != view {
            self.world.set_state(view);
          }

          if ui.button("Dump Update Graph").clicked() {
            let Some(update) = self.world.get_resource::<SystemGraph<Update>>() else {
              return;
            };

            let graph = update.graph.clone();
            let _ = IoTaskPool::get().spawn(async move {
              if let Err(e) = async_std::fs::write("update.dot", graph).await {
                error!("Failed to save update graph: {e}");
              }
            });
          }
        }
        crate::EditorState::Testing => {
          if ui.button("⏸").clicked() {
            self.world.set_state(crate::EditorState::Editing);
          }
        }
      };

      self
        .world
        .resource_scope(|world, mut log_info: Mut<LogInfo>| {
          ui.push_id("log-level-selector", |ui| {
            bevy_inspector::ui_for_value(&mut log_info.level, ui, world);
          });
        });
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
      Tabs::ControlPanel => {
        self.control_panel_ui(ui);
      }
      Tabs::GameView => {
        *self.viewport_rect = ui.clip_rect();
      }
      Tabs::Custom => {
        self
          .world
          .resource_scope(|world, custom_tab: Mut<CustomTab>| {
            (custom_tab.0)(world, ui);
          });
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
