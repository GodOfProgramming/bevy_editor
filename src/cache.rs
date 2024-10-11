use bevy::prelude::*;
use rusqlite::Connection;
use std::{error::Error, path::Path, sync::Mutex};

type CacheError = Box<dyn Error>;

#[derive(Resource)]
pub struct Cache {
  db: Mutex<Connection>,
}

impl Cache {
  pub fn connect(db: impl AsRef<Path>) -> Result<Self, CacheError> {
    let mut cache = Self {
      db: Mutex::new(Connection::open(db)?),
    };

    cache.initial_setup()?;

    Ok(cache)
  }

  fn initial_setup(&mut self) -> Result<(), CacheError> {
    Ok(())
  }

  fn create_table<T>(&mut self) -> Result<(), CacheError>
  where
    T: Table,
  {
    let db = self.db.lock().unwrap();
    db.execute(T::CREATE_TABLE_SQL, ())?;
    Ok(())
  }
}

trait Table {
  const CREATE_TABLE_SQL: &str;
}
