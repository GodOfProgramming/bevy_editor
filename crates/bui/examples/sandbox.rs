use bevy::prelude::*;
use ron::ser::PrettyConfig;

fn main() {
  println!(
    "{}",
    ron::ser::to_string_pretty(&Color::linear_rgb(1.0, 0.0, 0.0), PrettyConfig::default()).unwrap()
  );
}
