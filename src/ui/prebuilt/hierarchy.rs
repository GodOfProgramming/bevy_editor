use crate::ui::{InspectorSelection, RawUi, SelectedEntities, components};
use async_std::path::PathBuf;
use bevy::{prelude::*, reflect::TypeRegistry, tasks::IoTaskPool};
use bevy_egui::egui::{self, TextBuffer};
use bevy_inspector_egui::bevy_inspector;
use bui::PrimaryType;
use uuid::{Uuid, uuid};

#[derive(Default, Component, Reflect)]
pub struct Hierarchy;

impl RawUi for Hierarchy {
  const NAME: &str = stringify!(Hierarchy);
  const ID: Uuid = uuid!("860ac319-5c6e-4a2e-83ae-8bb0000d5cb4");

  fn init(app: &mut App) {
    app
      .add_event::<SelectEntityEvent>()
      .add_event::<ReparentEvent>()
      .add_event::<SerializeUiEvent>()
      .add_systems(
        FixedUpdate,
        (
          SelectEntityEvent::handle,
          ReparentEvent::handle,
          SerializeUiEvent::handle,
        ),
      );
  }

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    world.resource_scope(|world, mut selection: Mut<InspectorSelection>| {
      if let InspectorSelection::Entities(selected_entities) = selection.as_mut() {
        Self::show(ui, world, selected_entities);
      } else {
        let mut selected_entities = SelectedEntities::default();
        if Self::show(ui, world, &mut selected_entities) {
          *selection = InspectorSelection::Entities(selected_entities);
        }
      }
    });
  }
}

impl Hierarchy {
  fn show(ui: &mut egui::Ui, world: &mut World, selected: &mut SelectedEntities) -> bool {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let type_registry = type_registry.read();

    let ctx_menu = &mut Self::context_menu;
    let mut hierarchy = bevy_inspector::hierarchy::Hierarchy::<&TypeRegistry> {
      world,
      type_registry: &type_registry,
      selected,
      context_menu: Some(ctx_menu),
      shortcircuit_entity: None,
      extra_state: &mut &*type_registry,
    };

    hierarchy.show_with_default_filter::<()>(ui)
  }

  fn context_menu(
    ui: &mut egui::Ui,
    entity: Entity,
    world: &mut World,
    type_registry: &mut &TypeRegistry,
  ) {
    if ui.button("Select This").clicked() {
      world.send_event(SelectEntityEvent(entity));
    }

    if ui.button("Reparent Selected").clicked() {
      world.send_event(ReparentEvent(entity));
    }

    let mut entity_ref = world.entity_mut(entity);

    if entity_ref.get::<ChildOf>().is_some() && ui.button("Remove Parent").clicked() {
      entity_ref.remove::<ChildOf>();
    }

    if let Some(pt) = entity_ref.get::<PrimaryType>() {
      if type_registry.contains(pt.type_id()) && ui.button("Export UI").clicked() {
        world.send_event(SerializeUiEvent(entity));
      }
    }
  }
}

#[derive(Event)]
struct SelectEntityEvent(Entity);

impl SelectEntityEvent {
  fn handle(mut events: EventReader<Self>, mut selection: ResMut<InspectorSelection>) {
    for event in events.read() {
      select_entity(&mut selection, event.0);
    }
  }
}

#[derive(Event)]
struct ReparentEvent(Entity);

impl ReparentEvent {
  fn handle(
    mut commands: Commands,
    mut events: EventReader<Self>,
    mut selection: ResMut<InspectorSelection>,
  ) {
    for event in events.read() {
      if let InspectorSelection::Entities(selected) = &*selection {
        commands.entity(event.0).add_children(selected.as_slice());
        select_entity(&mut selection, event.0);
      }
    }
  }
}

#[derive(Event)]
struct SerializeUiEvent(Entity);

impl SerializeUiEvent {
  fn handle(
    commands: Commands,
    mut ctx: Single<&mut bevy_egui::EguiContext>,
    mut events: EventReader<Self>,
    mut save_name_text: Local<String>,
    mut last_entity: Local<Option<Entity>>,
  ) {
    let entity = events.read().last().map(|event| event.0);

    if entity.is_some() {
      *last_entity = entity;
    }

    Self::show_dialog(
      commands,
      ctx.get_mut(),
      &mut last_entity,
      &mut save_name_text,
    );
  }

  fn show_dialog(
    mut commands: Commands,
    ctx: &egui::Context,
    last_entity: &mut Option<Entity>,
    save_name_text: &mut String,
  ) {
    let open = components::Dialog::new("Save UI").open(ctx, last_entity.is_some(), |ui| {
      ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(save_name_text);
      });
      ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
          if let Some(entity) = last_entity.take() {
            let save_file = save_name_text.take();
            commands.queue(move |world: &mut World| -> Result {
              let tp = IoTaskPool::get();

              let bui = bui::Bui::serialize(entity, world)?;

              tp.spawn(async move {
                let path = PathBuf::from("assets").join("ui");
                if let Err(err) = async_std::fs::create_dir_all(&path).await {
                  error!("Failed to create assets ui directory: {err}");
                  return;
                }

                let path = path.join(save_file);
                let data = match bui.try_into_string() {
                  Ok(data) => data,
                  Err(err) => {
                    error!("Failed to serialize ui to {}: {err}", path.display());
                    return;
                  }
                };

                let res = async_std::fs::write(&path, data).await;
                if let Err(err) = res {
                  error!("Failed to save ui to {}: {err}", path.display());
                }
              })
              .detach();

              Ok(())
            });
          }
        }
      });
    });

    if !open {
      *last_entity = None;
    }
  }
}

fn select_entity(selection: &mut InspectorSelection, entity: Entity) {
  let mut entities = SelectedEntities::default();
  entities.0.select_replace(entity);
  *selection = InspectorSelection::Entities(entities)
}
