use crate::util::sorted_keys;
use bevy::{prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn cache_path() -> PathBuf {
  const FILE: &str = concat!(env!("CARGO_PKG_NAME"), ".cache.json");
  std::env::current_exe()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf()
    .join(FILE)
}

#[derive(Default, Resource, Serialize, Deserialize, Debug)]
pub struct Cache(#[serde(serialize_with = "sorted_keys")] HashMap<String, serde_json::Value>);

impl Cache {
  pub fn load_or_default() -> Self {
    let cache_path = cache_path();
    println!("Loading cache from: {}", cache_path.display());

    match std::fs::read_to_string(cache_path).map(|data| serde_json::from_str(&data)) {
      Ok(Ok(cache)) => cache,
      Ok(Err(err)) => {
        eprintln!("Error deserializing during initial load: {err}");
        Self::default()
      }
      Err(err) => {
        eprintln!("Error loading cache from disk: {err}");
        Self::default()
      }
    }
  }

  pub fn save(&self) {
    let cache_path = cache_path();
    info!("Saving cache to: {}", cache_path.display());

    match serde_json::to_string_pretty(self).map(|data| std::fs::write(cache_path, data)) {
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

  pub fn store<S>(&mut self, saveable: &S)
  where
    S: Saveable,
  {
    match serde_json::to_value(saveable) {
      Ok(value) => {
        self.0.insert(S::KEY.to_string(), value);
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
    match self
      .0
      .get(S::KEY)
      .map(|v| serde_json::from_value(v.clone()))?
    {
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
