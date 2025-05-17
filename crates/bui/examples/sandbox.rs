use bevy::prelude::*;
use syn::{ItemStruct, Visibility};

fn main() -> Result {
  let packages = ["bevy_ui", "bevy_text", "bevy_color"];

  let metadata = cargo_metadata::MetadataCommand::new().exec()?;

  let packages = metadata
    .packages
    .into_iter()
    .filter(|pkg| packages.contains(&pkg.name.as_str()));

  let mut lines = Vec::new();
  for pkg in packages {
    let mut structs = type_extractor::extract_structs(&pkg).map_err(|e| e.to_string())?;
    structs.sort_by(|a, b| a.ident.cmp(&b.ident));
    for s in structs {
      if is_valid_struct(&s) {
        let ident = s.ident;
        let fn_call = quote::quote! {
          plugin.register_attr::<#ident>();
        };
        lines.push(fn_call);
      }
    }
  }

  let attr_registration = quote::quote! {
    pub fn register_all(plugin: &mut crate::BuiPlugin) {
      #(#lines)*
    }
  }
  .to_string();

  println!("{attr_registration}");

  Ok(())
}

fn is_valid_struct(item: &ItemStruct) -> bool {
  if !matches!(item.vis, Visibility::Public(_)) || !item.generics.params.is_empty() {
    return false;
  }

  is_reflect_component(item)
}

fn is_reflect_component(item: &ItemStruct) -> bool {
  let mut has_component = false;
  let mut has_reflect = false;

  for attr in &item.attrs {
    if attr.path().is_ident("derive") {
      attr
        .parse_nested_meta(|meta_list| {
          if let Some(i) = meta_list.path.get_ident().map(|i| i.to_string()) {
            match i.as_str() {
              "Component" => has_component = true,
              "Reflect" => has_reflect = true,
              _ => (),
            }
          }
          Ok(())
        })
        .ok();
    }
  }

  has_component && has_reflect
}
