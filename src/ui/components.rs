use bevy::prelude::{Deref, DerefMut};
use bevy_egui::egui;
use derive_new::new;

#[derive(new)]
pub struct Dialog<T>
where
  T: Into<egui::WidgetText>,
{
  title: T,
}

impl<T> Dialog<T>
where
  T: Into<egui::WidgetText>,
{
  pub fn open(
    self,
    ctx: &egui::Context,
    opened: bool,
    contents: impl FnOnce(&mut egui::Ui),
  ) -> bool {
    let mut opened = opened;
    if opened {
      let window = egui::Window::new(self.title).open(&mut opened);
      Self::ui(ctx, window, contents);
    }
    opened
  }

  pub fn show(self, ctx: &egui::Context, contents: impl FnOnce(&mut egui::Ui)) {
    let window = egui::Window::new(self.title);
    Self::ui(ctx, window, contents);
  }

  fn ui(ctx: &egui::Context, window: egui::Window, contents: impl FnOnce(&mut egui::Ui)) {
    window
      .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
      .title_bar(true)
      .resizable(false)
      .movable(false)
      .collapsible(false)
      .show(ctx, contents);
  }
}

#[derive(new)]
pub struct Button<T>
where
  T: Into<egui::WidgetText>,
{
  text: T,
}

impl<T> Button<T>
where
  T: Into<egui::WidgetText>,
{
  pub fn show(self, ui: &mut egui::Ui) -> Response {
    Response(ui.button(self.text))
  }
}

#[derive(Deref, DerefMut)]
pub struct Response(egui::Response);

impl Response {
  pub fn then(self, handler: impl FnOnce(egui::Response)) {
    (handler)(self.0)
  }

  pub fn filter<P>(self, pred: P) -> ConditionalResponse<P>
  where
    P: FnOnce(egui::Response) -> bool,
  {
    ConditionalResponse::new(self, pred)
  }
}

#[derive(new)]
pub struct ConditionalResponse<P>
where
  P: FnOnce(egui::Response) -> bool,
{
  response: Response,
  pred: P,
}

impl<P> ConditionalResponse<P>
where
  P: FnOnce(egui::Response) -> bool,
{
  pub fn then(self, handler: impl FnOnce()) {
    if (self.pred)(self.response.0) {
      (handler)();
    }
  }
}

pub struct BorderedBox {
  pos: egui::Pos2,
  size: egui::Vec2,
  thickness: f32,
}

impl BorderedBox {
  pub fn new(pos: impl Into<egui::Pos2>, size: impl Into<egui::Vec2>) -> Self {
    Self {
      pos: pos.into(),
      size: size.into(),
      thickness: 1.0,
    }
  }

  pub fn with_thickness(mut self, thickness: f32) -> Self {
    self.thickness = thickness;
    self
  }

  pub fn draw(&self, ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    Self::ui(ui, self.pos, self.size, self.thickness, contents);
  }

  fn ui(
    ui: &mut egui::Ui,
    pos: egui::Pos2,
    size: egui::Vec2,
    thickness: f32,
    contents: impl FnOnce(&mut egui::Ui),
  ) {
    let rect = egui::Rect::from_min_size(pos, size);
    let stroke = egui::Stroke::new(thickness, ui.style().visuals.widgets.active.fg_stroke.color);

    egui::Frame::default().stroke(stroke).show(ui, |ui| {
      ui.set_min_size(rect.size());
      ui.set_max_size(rect.size());
      (contents)(ui);
    });
  }
}
