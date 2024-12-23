use bevy::{prelude::*, utils::HashMap};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn cache_path() -> PathBuf {
  std::env::current_exe()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf()
    .join("cache.ron")
}

#[derive(Resource, Serialize, Deserialize, Default)]
pub struct Cache {
  data: HashMap<String, String>,
}

impl Cache {
  pub fn load_or_default() -> Self {
    match std::fs::read_to_string(cache_path()).map(|data| ron::de::from_str(&data)) {
      Ok(Ok(cache)) => cache,
      _ => Self::default(),
    }
  }

  pub fn save(&self) {
    match ron::ser::to_string_pretty(self, PrettyConfig::default().struct_names(true))
      .map(|data| std::fs::write(cache_path(), data))
    {
      Ok(Ok(_)) => {
        info!("Saved cache");
      }
      Ok(Err(e)) => {
        error!("Failed to write cache to disk: {e}");
      }
      Err(e) => {
        error!("Failed to serialize cache: {e}");
      }
    }
  }

  pub fn store<S>(&mut self, saveable: S)
  where
    S: Saveable,
  {
    match ron::ser::to_string(&saveable) {
      Ok(data) => {
        self.data.insert(S::KEY.to_string(), data);
      }
      Err(e) => {
        error!("Failed to serialize {}: {e}", S::KEY);
      }
    }
  }

  pub fn get<S>(&self) -> Option<S>
  where
    S: Saveable,
  {
    let data = self.data.get(S::KEY).cloned()?;
    match ron::de::from_str(&data) {
      Ok(value) => Some(value),
      Err(e) => {
        error!("Failed to deserialize {}: {e}", S::KEY);
        None
      }
    }
  }
}

pub trait Saveable: Serialize + for<'de> Deserialize<'de> + Sized + 'static {
  const KEY: &str;
}
