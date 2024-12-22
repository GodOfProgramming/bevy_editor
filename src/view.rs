mod view2d;
mod view3d;

use bevy::app::Plugin;

pub use view3d::View3dPlugin;

pub trait ViewPlugin: Plugin {}
