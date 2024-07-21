use bevy::prelude::*;
use bevy::render::camera::CameraProjection;
use bevy::{
  asset::{ReflectAsset, UntypedAssetId},
  reflect::TypeRegistry,
};
use bevy_egui::egui;
use bevy_egui::egui::mutex::Mutex;
use bevy_inspector_egui::bevy_inspector::{
  self,
  hierarchy::{hierarchy_ui, SelectedEntities},
  ui_for_entities_shared_components, ui_for_entity_with_children,
};
use egui_dock::{DockArea, DockState, NodeIndex, Style};
use std::any::TypeId;
use std::marker::PhantomData;
use transform_gizmo_egui::mint::RowMatrix4;
use transform_gizmo_egui::{EnumSet, Gizmo, GizmoExt, GizmoMode, GizmoOrientation};

#[derive(Eq, PartialEq)]
enum InspectorSelection {
  Entities,
  Resource(TypeId, String),
  Asset(TypeId, String, UntypedAssetId),
}

pub struct UiPlugin<C>
where
  C: Component,
{
  _cam_component: PhantomData<C>,
}

impl<C> Default for UiPlugin<C>
where
  C: Component,
{
  fn default() -> Self {
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
  dialog: Mutex<egui_file_dialog::FileDialog>,
}

impl FileDialog {
  fn new() -> Self {
    Self {
      dialog: Mutex::new(egui_file_dialog::FileDialog::new()),
    }
  }

  fn access(&self, f: impl FnOnce(&egui_file_dialog::FileDialog)) {
    f(&self.dialog.lock());
  }

  fn access_mut(&mut self, f: impl FnOnce(&mut egui_file_dialog::FileDialog)) {
    f(&mut self.dialog.lock());
  }
}

#[derive(Resource)]
pub(crate) struct State<C: Component> {
  pub(crate) viewport_rect: egui::Rect,
  pub(crate) gizmo_mode: GizmoMode,
  pub(crate) selected_entities: SelectedEntities,
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
    let [game, _inspector] = tree.split_right(NodeIndex::root(), 0.75, vec![Tabs::Inspector]);
    let [game, _hierarchy] = tree.split_left(game, 0.2, vec![Tabs::Hierarchy, Tabs::Options]);
    let [_game, _bottom] = tree.split_below(game, 0.8, vec![Tabs::Resources, Tabs::Assets]);

    Self {
      viewport_rect: egui::Rect::NOTHING,
      gizmo_mode: GizmoMode::TranslateView,
      selected_entities: SelectedEntities::default(),
      dock_state: state,
      selection: InspectorSelection::Entities,
      cam_component: default(),
    }
  }

  pub(crate) fn ui(&mut self, world: &mut World, ctx: &mut egui::Context) {
    let mut tab_viewer = TabViewer::<C> {
      world,
      viewport_rect: &mut self.viewport_rect,
      selected_entities: &mut self.selected_entities,
      selection: &mut self.selection,
      gizmo_mode: self.gizmo_mode,
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
  Resources,
  Assets,
  Inspector,
  Options,
}

struct TabViewer<'a, C: Component> {
  world: &'a mut World,
  selected_entities: &'a mut SelectedEntities,
  selection: &'a mut InspectorSelection,
  viewport_rect: &'a mut egui::Rect,
  gizmo_mode: GizmoMode,
  cam_component: PhantomData<C>,
}

impl<C> TabViewer<'_, C>
where
  C: Component,
{
  fn draw_gizmo(&mut self, ui: &mut egui::Ui) {
    if self.selected_entities.len() != 1 {
      return;
    }

    let (cam_transform, projection) = self
      .world
      .query_filtered::<(&GlobalTransform, &Projection), With<C>>()
      .single(self.world);

    let view_matrix = Mat4::from(cam_transform.affine().inverse());
    let projection_matrix = projection.get_clip_from_view();

    let Some(selected) = self.selected_entities.iter().next() else {
      return;
    };

    let Some(transform) = self.world.get::<Transform>(selected) else {
      return;
    };

    let mut gizmo = Gizmo::new(transform_gizmo_egui::GizmoConfig {
      view_matrix: RowMatrix4::<f64>::from(view_matrix.to_cols_array().map(f64::from)),
      projection_matrix: RowMatrix4::<f64>::from(projection_matrix.to_cols_array().map(f64::from)),
      orientation: GizmoOrientation::Local,
      modes: EnumSet::from(self.gizmo_mode),
      ..Default::default()
    });

    let Some(results) = gizmo
      .interact(
        ui,
        &[transform_gizmo_egui::math::Transform {
          translation: transform_gizmo_egui::mint::Vector3::<f64>::from(
            transform.translation.to_array().map(f64::from),
          ),
          rotation: transform_gizmo_egui::mint::Quaternion::<f64>::from(
            transform.rotation.to_array().map(f64::from),
          ),
          scale: transform_gizmo_egui::mint::Vector3::<f64>::from(
            transform.scale.to_array().map(f64::from),
          ),
        }],
      )
      .map(|(_, res)| res)
    else {
      return;
    };

    let Some(result) = results.iter().next() else {
      return;
    };

    let mut transform = self.world.get_mut::<Transform>(selected).unwrap();
    *transform = Transform {
      translation: Vec3::new(
        result.translation.x as f32,
        result.translation.y as f32,
        result.translation.z as f32,
      ),
      rotation: Quat::from_axis_angle(
        Vec3::new(
          result.rotation.v.x as f32,
          result.rotation.v.y as f32,
          result.rotation.v.z as f32,
        ),
        result.rotation.s as f32,
      ),
      scale: Vec3::new(
        result.scale.x as f32,
        result.scale.y as f32,
        result.scale.z as f32,
      ),
    };
  }

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

        self.draw_gizmo(ui);
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
    }
  }
}
