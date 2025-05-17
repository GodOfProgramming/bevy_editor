use cargo_metadata::Package;
use std::{
  env,
  error::Error,
  fs,
  path::{Path, PathBuf},
};
use walkdir::WalkDir;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub fn extract_structs(pkg: &Package) -> Result<Vec<syn::ItemStruct>> {
  let mut structs = Vec::new();

  let crate_path = find_crate_path(pkg)?;
  for_each_file(&crate_path, |syntax| {
    let s = syntax
      .items
      .into_iter()
      .filter_map(|item| {
        if let syn::Item::Struct(s) = item {
          Some(s)
        } else {
          None
        }
      })
      .collect::<Vec<_>>();
    structs.extend(s);
  })?;

  Ok(structs)
}

fn registry_path() -> Result<PathBuf> {
  let cargo_home = env::var("CARGO_HOME")
    .map(PathBuf::from)
    .or_else(|_| -> Result<PathBuf> {
      let home = env::var("HOME").map(PathBuf::from)?;
      Ok(home.join(".cargo"))
    })?;

  let reg_src = cargo_home.join("registry").join("src");

  Ok(reg_src)
}

fn find_crate_path(pkg: &Package) -> Result<PathBuf> {
  let registry_path = registry_path()?;

  let crate_dir = format!("{}-{}", &pkg.name, &pkg.version);
  let Some(crate_path) = fs::read_dir(&registry_path)?
    .filter_map(std::result::Result::ok)
    .find_map(|entry| {
      let path = entry.path();
      let package_path = path.join(&crate_dir);
      package_path.exists().then_some(package_path)
    })
  else {
    return Err("Crate not found in registry")?;
  };

  Ok(crate_path)
}

fn for_each_file<F>(crate_path: &Path, mut f: F) -> Result<()>
where
  F: FnMut(syn::File),
{
  for entry in WalkDir::new(crate_path)
    .into_iter()
    .filter_map(std::result::Result::ok)
    .filter(|e| e.path().extension().map(|ext| ext == "rs").unwrap_or(false))
  {
    let content = fs::read_to_string(entry.path())?;
    if let Ok(syntax) = syn::parse_file(&content) {
      (f)(syntax);
    }
  }

  Ok(())
}
