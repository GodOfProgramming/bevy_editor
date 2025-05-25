use std::{error::Error, path::PathBuf};
use syn::{ItemStruct, Visibility};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
  println!("cargo::rerun-if-changed=build.rs");
  println!("cargo::rerun-if-changed=Cargo.toml");

  let packages = ["bevy_ui", "bevy_text", "bevy_color"];

  let metadata = cargo_metadata::MetadataCommand::new()
    .exec()
    .map_err(|e| format!("Failed to execute metadata command: {e}"))?;

  let packages = metadata
    .packages
    .into_iter()
    .filter(|pkg| packages.contains(&pkg.name.as_str()));

  let mut structs = Vec::new();
  for pkg in packages {
    let extracted = type_extractor::extract_structs(&pkg)
      .map_err(|e| format!("Failed to query structs for {}: {e}", pkg.name))?;
    structs.extend(extracted);
  }

  structs.sort_by(|a, b| a.ident.cmp(&b.ident));

  let mut lines = Vec::new();

  for s in structs {
    if is_valid_struct(&s) {
      let ident = s.ident;
      let fn_call = quote::quote! {
        plugin.register_attr::<#ident>();
      };
      lines.push(fn_call);
    }
  }

  let attr_registration = quote::quote! {
    use bevy::{ prelude::*, text::*, ui::{experimental::*, widget::*, *} };

    pub fn register_all(plugin: &mut crate::BuiPlugin) {
      #(#lines)*
    }
  }
  .to_string();

  let generated_path = PathBuf::from("src").join("ui").join("generated");

  std::fs::create_dir_all(&generated_path)?;

  std::fs::write(generated_path.join("attrs.rs"), attr_registration)
    .map_err(|e| format!("Unable to write generated file: {e}"))?;

  Ok(())
}

fn is_valid_struct(item: &ItemStruct) -> bool {
  if !matches!(item.vis, Visibility::Public(_)) || !item.generics.params.is_empty() {
    return false;
  }

  is_reflect_component(item)
}

fn is_reflect_component(item: &ItemStruct) -> bool {
  const DERIVE: &str = "derive";
  const COMPONENT: &str = "Component";
  const REFLECT: &str = "Reflect";
  const CLONE: &str = "Clone";

  let mut has_component = false;
  let mut has_reflect = false;
  let mut has_clone = false;

  for attr in &item.attrs {
    if attr.path().is_ident(DERIVE) {
      attr
        .parse_nested_meta(|meta_list| {
          if let Some(i) = meta_list.path.get_ident().map(|i| i.to_string()) {
            match i.as_str() {
              COMPONENT => has_component = true,
              REFLECT => has_reflect = true,
              CLONE => has_clone = true,
              _ => (),
            }
          }
          Ok(())
        })
        .ok();
    }
  }

  has_component && has_reflect && has_clone
}
