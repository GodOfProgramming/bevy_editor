use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::Element;

#[derive(Default, Component, Reflect, Serialize, Deserialize, Clone)]
#[require(Button)]
#[reflect(Default)]
#[reflect(Component)]
pub struct UiButton;

impl Element for UiButton {}
