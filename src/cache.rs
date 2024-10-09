use bevy::prelude::*;
use rusqlite::Connection;
use std::{any::TypeId, error::Error, path::Path, sync::Mutex};

use crate::Prefab;

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

  pub fn list_prefabs(&self) -> Result<Vec<Prefab>, CacheError> {
    let db = self.db.lock().unwrap();
    Components::get_all(&db)
  }

  pub fn component_prefab(&self, name: &str) -> Result<Option<String>, CacheError> {
    let db = self.db.lock().unwrap();
    Components::get(&db, name)
  }

  pub fn register_type_prefab(&self, type_path: &str, prefab: impl ToString) {
    let db = self.db.lock().unwrap();
    Components::insert(&db, type_path, prefab.to_string()).unwrap();
  }

  fn initial_setup(&mut self) -> Result<(), CacheError> {
    self.create_table::<Components>()?;
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

struct Components;

impl Components {
  fn get_all(db: &Connection) -> Result<Vec<Prefab>, CacheError> {
    const SQL: &str = include_str!("sql/component_get_all.sql");
    let mut stmt = db.prepare(SQL)?;
    let iter = stmt.query_map((), |row| {
      Ok(Prefab {
        datatype: row.get(0)?,
        ron_repr: row.get(1)?,
      })
    })?;

    Ok(iter.map(|x| x.unwrap()).collect())
  }

  fn get(db: &Connection, name: impl AsRef<str>) -> Result<Option<String>, CacheError> {
    const SQL: &str = include_str!("sql/component_get.sql");
    let prefab = db
      .query_row(SQL, [name.as_ref()], |row| row.get(0))
      .map(Some)
      .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        e => Err(e),
      })?;
    Ok(prefab)
  }

  fn insert(
    db: &Connection,
    name: impl AsRef<str>,
    prefab: impl AsRef<str>,
  ) -> Result<(), CacheError> {
    const SQL: &str = include_str!("sql/component_insert.sql");
    db.execute(SQL, (name.as_ref(), prefab.as_ref()))?;
    Ok(())
  }
}

impl Table for Components {
  const CREATE_TABLE_SQL: &str = include_str!("sql/component_table.sql");
}
