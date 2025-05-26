use crate::UiManager;

use super::{LayoutInfo, RawUi, Ui, VTable};
use bevy::{
  ecs::{
    component::Mutable,
    system::{SystemParam, SystemState},
  },
  platform::collections::HashMap,
  prelude::*,
};
use bevy_egui::egui::{self, text::LayoutJob};
use derive_more::derive::Deref;
use egui_dock::DockState;
use persistent_id::PersistentId;
use uuid::{Uuid, uuid};

#[derive(SystemParam)]
pub struct NoParams;

#[derive(Component, Default)]
pub struct UiInfo {
  pub(super) rendered: bool,
  pub(super) hovered: bool,
}

impl UiInfo {
  pub fn rendered(&self) -> bool {
    self.rendered
  }

  pub fn hovered(&self) -> bool {
    self.hovered
  }
}

pub(super) trait UiComponentExtensions {
  const VTABLE: VTable;
}

impl<T> UiComponentExtensions for T
where
  T: RawUi,
{
  const VTABLE: VTable = VTable::new::<Self>();
}

type UiParams<'w, 's, T> = UiComponentState<<T as Ui>::Params<'w, 's>>;

/// # Safety
/// Cannot access the world mutably in the system params
/// Though it is on the user to not query for a mutable reference to themselves when they also have a self reference
pub unsafe trait UiExtensions: Ui {
  fn get_entity<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(&Self, Self::Params<'_, '_>) -> T,
  ) -> T {
    let mut q = world.query::<(&Self, &mut UiParams<Self>)>();
    let world_cell = world.as_unsafe_world_cell();
    let Ok((this, mut params)) = q.get_mut(unsafe { world_cell.world_mut() }, entity) else {
      panic!("Failed to query {}", <Self as Ui>::NAME);
    };
    let params = params.get_mut(unsafe { world_cell.world_mut() });
    f(this, params)
  }

  fn get_entity_mut<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(&mut Self, Self::Params<'_, '_>) -> T,
  ) -> T
  where
    Self: Component<Mutability = Mutable>,
  {
    let mut q = world.query::<(&mut Self, &mut UiParams<Self>)>();
    let world_cell = world.as_unsafe_world_cell();
    let (mut this, mut params) = q
      .get_mut(unsafe { world_cell.world_mut() }, entity)
      .unwrap();
    let params = params.get_mut(unsafe { world_cell.world_mut() });
    f(this.as_mut(), params)
  }

  fn register_params(entity: Entity, world: &mut World) {
    if !world.entity(entity).contains::<UiParams<Self>>() {
      let state = SystemState::<<Self as Ui>::Params<'_, '_>>::new(world);
      world.entity_mut(entity).insert(UiComponentState(state));
    }
  }

  fn with_params<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(Self::Params<'_, '_>) -> T,
  ) -> T {
    let world_cell = world.as_unsafe_world_cell();
    let mut entity = unsafe { world_cell.world_mut() }.entity_mut(entity);
    let mut params = entity.get_mut::<UiParams<Self>>().unwrap();
    let params = params.get_mut(unsafe { world_cell.world_mut() });
    f(params)
  }
}

unsafe impl<T> UiExtensions for T where T: Ui {}

#[derive(Component, Deref, DerefMut)]
struct UiComponentState<P>(SystemState<P>)
where
  P: SystemParam + 'static;

#[derive(Component, Reflect, Default)]
pub struct MissingUi {
  message: String,
  id: PersistentId,
  name: String,
}

impl MissingUi {
  pub fn new(name: impl Into<String>, id: impl Into<PersistentId>) -> Self {
    let id = id.into();
    let name = name.into();
    Self {
      message: format!("Failed to find ui component {name} with uuid: {}", *id),
      id,
      name,
    }
  }
  pub fn id(&self) -> &PersistentId {
    &self.id
  }
}

impl Ui for MissingUi {
  const NAME: &str = "Missing Ui";
  const ID: Uuid = uuid!("d0f32ae1-2851-4bcd-a0c9-f83ae030d85f");

  type Params<'w, 's> = NoParams;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn hidden() -> bool {
    true
  }

  fn render(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {
    let mut job = LayoutJob::single_section(self.message.to_owned(), egui::TextFormat::default());
    job.wrap = egui::text::TextWrapping::default();
    ui.label(job);
  }

  fn unique() -> bool {
    true // prevents this from showing up in the spawn ui menu
  }
}

pub(super) trait DockExtensions {
  fn decouple(
    &self,
    ui_manager: &UiManager,
    q_uuids: &Query<&PersistentId, Without<MissingUi>>,
    q_missing: &Query<&MissingUi>,
  ) -> DockState<LayoutInfo>;

  fn restore(
    dock: &DockState<LayoutInfo>,
    vtables: &HashMap<PersistentId, VTable>,
    world: &mut World,
  ) -> Self;
}

impl DockExtensions for DockState<Entity> {
  fn decouple(
    &self,
    ui_manager: &UiManager,
    q_persistent_ids: &Query<&PersistentId, Without<MissingUi>>,
    q_missing: &Query<&MissingUi>,
  ) -> DockState<LayoutInfo> {
    self.map_tabs(|tab| {
      let id;
      let name;

      if let Ok(missing_uuid) = q_missing.get(*tab) {
        id = *missing_uuid.id();
        name = missing_uuid.name.clone();
      } else {
        id = *q_persistent_ids.get(*tab).unwrap();
        name = ui_manager
          .get_vtable_by_id(&id)
          .map(|vt| vt.name.to_string())
          .unwrap_or_default();
      }

      LayoutInfo { id, name }
    })
  }

  fn restore(
    dock: &DockState<LayoutInfo>,
    vtables: &HashMap<PersistentId, VTable>,
    world: &mut World,
  ) -> Self {
    dock.map_tabs(|layout_info| {
      vtables
        .get(&layout_info.id)
        .map(|vtable| (vtable.spawn)(world))
        .unwrap_or_else(|| {
          let name = &layout_info.name;
          let state = SystemState::<<MissingUi as Ui>::Params<'_, '_>>::new(world);

          warn!(
            "Failed to find ui component {name} with uuid {}",
            *layout_info.id
          );

          world
            .spawn((
              Name::new(<MissingUi as RawUi>::NAME),
              MissingUi::new(name, layout_info.id),
              PersistentId(<MissingUi as RawUi>::ID),
              UiInfo::default(),
              UiComponentState(state),
            ))
            .id()
        })
    })
  }
}
