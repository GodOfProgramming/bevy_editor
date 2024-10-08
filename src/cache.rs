use rusqlite::Connection;
use std::{error::Error, path::Path};

type CacheError = Box<dyn Error>;

pub struct Cache {
  db: Connection,
}

impl Cache {
  pub fn connect(db: impl AsRef<Path>) -> Result<Self, CacheError> {
    let mut cache = Self {
      db: Connection::open(db)?,
    };

    cache.initial_setup()?;

    Ok(cache)
  }

  pub fn component_prefab(&self, name: &str) -> Result<Option<String>, CacheError> {
    Components::get(&self.db, name)
  }

  pub fn register_type_prefab(&self, type_path: &str, prefab: impl ToString) {
    Components::insert(&self.db, type_path, prefab.to_string()).unwrap();
  }

  fn initial_setup(&mut self) -> Result<(), CacheError> {
    self.create_table::<Components>()?;
    Ok(())
  }

  fn create_table<T>(&mut self) -> Result<(), CacheError>
  where
    T: Table,
  {
    self.db.execute(T::CREATE_TABLE_SQL, ())?;
    Ok(())
  }
}

trait Table {
  const CREATE_TABLE_SQL: &str;
}

struct Components;

impl Components {
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
