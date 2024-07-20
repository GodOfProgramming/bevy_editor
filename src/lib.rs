use bevy::prelude::*;
use bevy::render::camera::CameraProjection;
use bevy::{
    asset::{ReflectAsset, UntypedAssetId},
    reflect::TypeRegistry,
    render::camera::Viewport,
    window::PrimaryWindow,
};
use bevy_egui::{egui, EguiContext, EguiSet};
use bevy_inspector_egui::{
    bevy_inspector::{
        self,
        hierarchy::{hierarchy_ui, SelectedEntities},
        ui_for_entities_shared_components, ui_for_entity_with_children,
    },
    DefaultInspectorConfigPlugin,
};
use egui_dock::{DockArea, DockState, NodeIndex, Style};
use std::any::TypeId;
use std::marker::PhantomData;
use transform_gizmo_egui::mint::RowMatrix4;
use transform_gizmo_egui::{EnumSet, Gizmo, GizmoExt, GizmoMode, GizmoOrientation};

pub struct EditorPlugin<C>
where
    C: Component,
{
    cam_component: PhantomData<C>,
}

impl<C> Default for EditorPlugin<C>
where
    C: Component,
{
    fn default() -> Self {
        Self {
            cam_component: default(),
        }
    }
}

impl<C> Plugin for EditorPlugin<C>
where
    C: Component,
{
    fn build(&self, app: &mut App) {
        app.add_plugins((bevy_egui::EguiPlugin, DefaultInspectorConfigPlugin))
            .insert_resource(UiState::<C>::new())
            .add_systems(
                PostUpdate,
                (
                    show_ui_system::<C>
                        .before(EguiSet::ProcessOutput)
                        .before(bevy::transform::TransformSystem::TransformPropagate),
                    set_camera_viewport::<C>,
                )
                    .chain(),
            )
            .add_systems(Update, set_gizmo_mode::<C>)
            .register_type::<Option<Handle<Image>>>()
            .register_type::<AlphaMode>();
    }
}

fn show_ui_system<C>(world: &mut World)
where
    C: Component,
{
    let Ok(egui_context) = world
        .query_filtered::<&mut EguiContext, With<PrimaryWindow>>()
        .get_single(world)
    else {
        return;
    };
    let mut egui_context = egui_context.clone();

    world.resource_scope::<UiState<C>, _>(|world, mut ui_state| {
        ui_state.ui(world, egui_context.get_mut())
    });
}

// make camera only render to view not obstructed by UI
fn set_camera_viewport<C: Component>(
    ui_state: Res<UiState<C>>,
    primary_window: Query<&mut Window, With<PrimaryWindow>>,
    egui_settings: Res<bevy_egui::EguiSettings>,
    mut cameras: Query<&mut Camera, With<C>>,
) {
    let mut cam = cameras.single_mut();

    let Ok(window) = primary_window.get_single() else {
        return;
    };

    let scale_factor = window.scale_factor() * egui_settings.scale_factor;

    let viewport_pos = ui_state.viewport_rect.left_top().to_vec2() * scale_factor;
    let viewport_size = ui_state.viewport_rect.size() * scale_factor;

    let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
    let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

    // The desired viewport rectangle at its offset in "physical pixel space"
    let rect = physical_position + physical_size;

    let window_size = window.physical_size();
    // wgpu will panic if trying to set a viewport rect which has coordinates extending
    // past the size of the render target, i.e. the physical window in our case.
    // Typically this shouldn't happen- but during init and resizing etc. edge cases might occur.
    // Simply do nothing in those cases.
    if rect.x <= window_size.x && rect.y <= window_size.y {
        cam.viewport = Some(Viewport {
            physical_position,
            physical_size,
            depth: 0.0..1.0,
        });
    }
}

fn set_gizmo_mode<C>(input: Res<ButtonInput<KeyCode>>, mut ui_state: ResMut<UiState<C>>)
where
    C: Component,
{
    for (key, mode) in [
        (KeyCode::Numpad1, GizmoMode::ScaleUniform),
        (KeyCode::Numpad2, GizmoMode::RotateView),
        (KeyCode::Numpad3, GizmoMode::TranslateView),
    ] {
        if input.just_pressed(key) {
            ui_state.gizmo_mode = mode;
        }
    }
}

#[derive(Eq, PartialEq)]
enum InspectorSelection {
    Entities,
    Resource(TypeId, String),
    Asset(TypeId, String, UntypedAssetId),
}

#[derive(Resource)]
struct UiState<C: Component> {
    state: DockState<EguiWindow>,
    viewport_rect: egui::Rect,
    selected_entities: SelectedEntities,
    selection: InspectorSelection,
    gizmo_mode: GizmoMode,
    cam_component: PhantomData<C>,
}

impl<C> UiState<C>
where
    C: Component,
{
    pub fn new() -> Self {
        let mut state = DockState::new(vec![EguiWindow::GameView]);
        let tree = state.main_surface_mut();
        let [game, _inspector] =
            tree.split_right(NodeIndex::root(), 0.75, vec![EguiWindow::Inspector]);
        let [game, _hierarchy] = tree.split_left(game, 0.2, vec![EguiWindow::Hierarchy]);
        let [_game, _bottom] =
            tree.split_below(game, 0.8, vec![EguiWindow::Resources, EguiWindow::Assets]);

        Self {
            state,
            selected_entities: SelectedEntities::default(),
            selection: InspectorSelection::Entities,
            viewport_rect: egui::Rect::NOTHING,
            gizmo_mode: GizmoMode::TranslateView,
            cam_component: default(),
        }
    }

    fn ui(&mut self, world: &mut World, ctx: &mut egui::Context) {
        let mut tab_viewer = TabViewer::<C> {
            world,
            viewport_rect: &mut self.viewport_rect,
            selected_entities: &mut self.selected_entities,
            selection: &mut self.selection,
            gizmo_mode: self.gizmo_mode,
            cam_component: default(),
        };
        DockArea::new(&mut self.state)
            .style(Style::from_egui(ctx.style().as_ref()))
            .show(ctx, &mut tab_viewer);
    }
}

#[derive(Debug)]
enum EguiWindow {
    GameView,
    Hierarchy,
    Resources,
    Assets,
    Inspector,
}

struct TabViewer<'a, C: Component> {
    world: &'a mut World,
    selected_entities: &'a mut SelectedEntities,
    selection: &'a mut InspectorSelection,
    viewport_rect: &'a mut egui::Rect,
    gizmo_mode: GizmoMode,
    cam_component: PhantomData<C>,
}

impl<C> egui_dock::TabViewer for TabViewer<'_, C>
where
    C: Component,
{
    type Tab = EguiWindow;

    fn ui(&mut self, ui: &mut egui_dock::egui::Ui, window: &mut Self::Tab) {
        let type_registry = self.world.resource::<AppTypeRegistry>().0.clone();
        let type_registry = type_registry.read();

        match window {
            EguiWindow::GameView => {
                *self.viewport_rect = ui.clip_rect();

                draw_gizmo::<C>(ui, self.world, self.selected_entities, self.gizmo_mode);
            }
            EguiWindow::Hierarchy => {
                let selected = hierarchy_ui(self.world, ui, self.selected_entities);
                if selected {
                    *self.selection = InspectorSelection::Entities;
                }
            }
            EguiWindow::Resources => select_resource(ui, &type_registry, self.selection),
            EguiWindow::Assets => select_asset(ui, &type_registry, self.world, self.selection),
            EguiWindow::Inspector => match *self.selection {
                InspectorSelection::Entities => match self.selected_entities.as_slice() {
                    &[entity] => ui_for_entity_with_children(self.world, entity, ui),
                    entities => ui_for_entities_shared_components(self.world, entities, ui),
                },
                InspectorSelection::Resource(type_id, ref name) => {
                    ui.label(name);
                    bevy_inspector::by_type_id::ui_for_resource(
                        self.world,
                        type_id,
                        ui,
                        name,
                        &type_registry,
                    )
                }
                InspectorSelection::Asset(type_id, ref name, handle) => {
                    ui.label(name);
                    bevy_inspector::by_type_id::ui_for_asset(
                        self.world,
                        type_id,
                        handle,
                        ui,
                        &type_registry,
                    );
                }
            },
        }
    }

    fn title(&mut self, window: &mut Self::Tab) -> egui_dock::egui::WidgetText {
        format!("{window:?}").into()
    }

    fn clear_background(&self, window: &Self::Tab) -> bool {
        !matches!(window, EguiWindow::GameView)
    }
}

fn draw_gizmo<C: Component>(
    ui: &mut egui::Ui,
    world: &mut World,
    selected_entities: &SelectedEntities,
    gizmo_mode: GizmoMode,
) {
    let (cam_transform, projection) = world
        .query_filtered::<(&GlobalTransform, &Projection), With<C>>()
        .single(world);
    let view_matrix = Mat4::from(cam_transform.affine().inverse());
    let projection_matrix = projection.get_clip_from_view();

    if selected_entities.len() != 1 {
        return;
    }

    for selected in selected_entities.iter() {
        let Some(transform) = world.get::<Transform>(selected) else {
            continue;
        };

        let mut gizmo = Gizmo::new(transform_gizmo_egui::GizmoConfig {
            view_matrix: RowMatrix4::<f64>::from(view_matrix.to_cols_array().map(f64::from)),
            projection_matrix: RowMatrix4::<f64>::from(
                projection_matrix.to_cols_array().map(f64::from),
            ),
            orientation: GizmoOrientation::Local,
            modes: EnumSet::from(gizmo_mode),
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
            continue;
        };

        let Some(result) = results.iter().next() else {
            continue;
        };

        let mut transform = world.get_mut::<Transform>(selected).unwrap();
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
}

fn select_resource(
    ui: &mut egui::Ui,
    type_registry: &TypeRegistry,
    selection: &mut InspectorSelection,
) {
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
        let selected = match *selection {
            InspectorSelection::Resource(selected, _) => selected == type_id,
            _ => false,
        };

        if ui.selectable_label(selected, resource_name).clicked() {
            *selection = InspectorSelection::Resource(type_id, resource_name.to_string());
        }
    }
}

fn select_asset(
    ui: &mut egui::Ui,
    type_registry: &TypeRegistry,
    world: &World,
    selection: &mut InspectorSelection,
) {
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
        let handles: Vec<_> = reflect_asset.ids(world).collect();

        ui.collapsing(format!("{asset_name} ({})", handles.len()), |ui| {
            for handle in handles {
                let selected = match *selection {
                    InspectorSelection::Asset(_, _, selected_id) => selected_id == handle,
                    _ => false,
                };

                if ui
                    .selectable_label(selected, format!("{:?}", handle))
                    .clicked()
                {
                    *selection =
                        InspectorSelection::Asset(asset_type_id, asset_name.to_string(), handle);
                }
            }
        });
    }
}
