use std::marker::PhantomData;

use crate::ui::Ui;
use crate::util::LoggingSettings;
use bevy::{diagnostic::DiagnosticsStore, ecs::system::SystemParam, prelude::*};
use bevy_egui::{EguiContext, egui};
use bevy_inspector_egui::reflect_inspector::ui_for_value;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct DebugMenu {
  ui_debug_on_hover: bool,
}

impl DebugMenu {
  fn log_level_selector(&self, ui: &mut egui::Ui, params: &mut Params) {
    ui.push_id("log-level-selector", |ui| {
      ui.horizontal(|ui| {
        let type_registry = params.type_registry.as_ref().read();

        ui.label("Log Level");
        let mut level = params.logging.level();
        ui_for_value(&mut level, ui, &type_registry);

        if level != params.logging.level() {
          params.logging.set_level(level);
        }
      });
    });
  }

  fn diagnostics(&self, ui: &mut egui::Ui, params: &Params) {
    egui::Grid::new("sys-diagnostics").show(ui, |ui| {
      for diagnostic in params.diagnostics.iter() {
        ui.label(diagnostic.path().as_str());
        if let Some(average) = diagnostic.average() {
          ui.label(format!("{:.2}", average));
        }
        ui.end_row();
      }
    });
  }

  fn handle_ui_debug(
    mut events: EventReader<DebugUiEvent>,
    mut q_egui_ctx: Query<&mut EguiContext>,
  ) {
    for event in events.read() {
      for mut ctx in &mut q_egui_ctx {
        let ctx = ctx.get_mut();
        ctx.set_debug_on_hover(event.0);
      }
    }
  }
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  type_registry: Res<'w, AppTypeRegistry>,
  logging: ResMut<'w, LoggingSettings>,
  diagnostics: Res<'w, DiagnosticsStore>,
  debug_ui_event_writer: EventWriter<'w, DebugUiEvent>,

  _pd: PhantomData<&'s ()>,
}

impl Ui for DebugMenu {
  const NAME: &str = "Debug Menu";
  const ID: uuid::Uuid = uuid!("9473f6e1-a595-41e2-8e29-a4f041580fa6");

  type Params<'w, 's> = Params<'w, 's>;

  fn init(app: &mut App) {
    app
      .add_event::<DebugUiEvent>()
      .add_systems(Update, Self::handle_ui_debug);
  }

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
    self.diagnostics(ui, &params);
    ui.separator();
    self.log_level_selector(ui, &mut params);
    ui.separator();
    if ui
      .checkbox(&mut self.ui_debug_on_hover, "Debug UI")
      .clicked()
    {
      params
        .debug_ui_event_writer
        .write(DebugUiEvent(self.ui_debug_on_hover));
    }
  }
}

#[derive(Event)]
struct DebugUiEvent(bool);
