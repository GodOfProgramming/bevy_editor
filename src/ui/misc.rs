use super::{PersistentId, Ui, UiComponent, UiComponentState, VTable};
use bevy::{
  ecs::system::{SystemParam, SystemState},
  prelude::*,
  utils::HashMap,
};
use bevy_egui::egui::{self, text::LayoutJob};
use egui_dock::DockState;
use uuid::{uuid, Uuid};

pub(super) trait UiComponentExtensions {
  const VTABLE: VTable;
}

impl<T> UiComponentExtensions for T
where
  T: UiComponent,
{
  const VTABLE: VTable = VTable::new::<Self>();
}

type UiParams<'w, 's, T> = UiComponentState<<T as Ui>::Params<'w, 's>>;

pub trait UiExtensions: Ui {
  fn get_entity<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(&Self, Self::Params<'_, '_>) -> T,
  ) -> T {
    Self::register_params(entity, world);
    let mut q = world.query::<(&Self, &mut UiParams<Self>)>();
    let world_cell = world.as_unsafe_world_cell();
    let (this, mut params) = q
      .get_mut(unsafe { world_cell.world_mut() }, entity)
      .unwrap();
    let params = params.get_mut(unsafe { world_cell.world_mut() });
    f(this, params)
  }

  fn get_entity_mut<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(&mut Self, Self::Params<'_, '_>) -> T,
  ) -> T {
    Self::register_params(entity, world);
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

impl<T> UiExtensions for T where T: Ui {}

#[derive(Component, Reflect)]
pub struct MissingUi {
  message: String,
  uuid: Uuid,
}

impl MissingUi {
  pub fn new(id: impl Into<PersistentId>) -> Self {
    let id = id.into();
    Self {
      message: format!("Failed to find ui component with uuid: {}", id.to_string()),
      uuid: *id,
    }
  }
  pub fn id(&self) -> &Uuid {
    &self.uuid
  }
}

#[derive(SystemParam)]
pub struct NoUiParams;

impl Ui for MissingUi {
  const NAME: &str = "No Ui";
  const UUID: Uuid = uuid!("d0f32ae1-2851-4bcd-a0c9-f83ae030d85f");

  type Params<'w, 's> = NoUiParams;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    Self {
      message: default(),
      uuid: default(),
    }
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
    q_uuids: &Query<&PersistentId, Without<MissingUi>>,
    q_missing: &Query<&MissingUi>,
  ) -> DockState<Uuid>;

  fn restore(
    dock: &DockState<Uuid>,
    vtables: &HashMap<PersistentId, VTable>,
    world: &mut World,
  ) -> Self;
}

impl DockExtensions for DockState<Entity> {
  fn decouple(
    &self,
    q_uuids: &Query<&PersistentId, Without<MissingUi>>,
    q_missing: &Query<&MissingUi>,
  ) -> DockState<Uuid> {
    self.map_tabs(|tab| {
      if let Ok(missing_uuid) = q_missing.get(*tab) {
        missing_uuid.id().clone()
      } else {
        **q_uuids.get(*tab).unwrap()
      }
    })
  }

  fn restore(
    dock: &DockState<Uuid>,
    vtables: &HashMap<PersistentId, VTable>,
    world: &mut World,
  ) -> Self {
    dock.map_tabs(|tab| {
      vtables
        .get(&PersistentId(*tab))
        .map(|vtable| (vtable.spawn)(world))
        .unwrap_or_else(|| {
          let entity_id = world.spawn((MissingUi::new(*tab), MissingUi::ID)).id();
          world.entity_mut(entity_id).insert(Name::new("Missing Ui"));
          info!("Failed to find ui with uuid: {tab}");
          entity_id
        })
    })
  }
}
