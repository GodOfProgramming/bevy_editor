use crate::{Ui, registry::components::ComponentRegistry};
use bevy::{
  ecs::{
    component::Component,
    system::{Res, SystemParam},
  },
  reflect::Reflect,
};
use bevy_egui::egui::{self};
use std::marker::PhantomData;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct Components;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  component_registry: Res<'w, ComponentRegistry>,
  _pd: PhantomData<&'s ()>,
}

impl Ui for Components {
  const NAME: &str = "Components";

  const ID: uuid::Uuid = uuid!("5b376389-2acf-4945-807b-94ee16c09088");

  type Params<'w, 's> = Params<'w, 's>;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    Self
  }

  fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
    for (id, comp) in params.component_registry.iter() {
      ui.dnd_drag_source(egui::Id::new(id), *id, |ui| {
        ui.label(comp.name());
      });
    }
  }
}
