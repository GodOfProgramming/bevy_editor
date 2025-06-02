use bevy::platform::collections::{Equivalent, HashMap};
use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::hash::Hash;

pub struct Vfs<T> {
  inner: HashMap<VfsPath, VfsDir<T>>,
}

impl<T> Vfs<T> {
  pub fn new() -> Self {
    Self { inner: default() }
  }

  pub fn create(&mut self, path: impl Into<VfsPath>) -> &mut VfsDir<T> {
    let path = path.into();
    if let Some((parent_ref, basename)) = path.parent_ref().zip(path.basename()) {
      if self.get_dir(&parent_ref).is_none() {
        self.create(parent_ref).add_dir(basename);
      }
    }
    self.inner.entry(path).or_default()
  }

  pub fn get_dir<P>(&self, path: P) -> Option<&VfsDir<T>>
  where
    P: Eq + Hash + Equivalent<VfsPath>,
  {
    self.inner.get(&path)
  }

  pub fn get_node<P>(&self, path: P) -> Option<&VfsNode<T>>
  where
    P: AsRef<VfsPath>,
  {
    let path_ref = path.as_ref();
    path_ref
      .parent_ref()
      .zip(path_ref.basename())
      .and_then(|(parent, item)| self.inner.get(&parent).and_then(|dir| dir.get(item)))
  }
}

#[derive(PartialEq, Eq, Hash)]
pub struct VfsPath<T = String>(Vec<T>)
where
  T: Eq + Hash;

impl VfsPath {
  pub fn parent(&self) -> Option<Self> {
    match self.0.len() {
      0 => None,
      1 => Some(Self(Vec::new())),
      n => {
        let slice = &self.0[0..n - 1];
        Some(Self(Vec::from(slice)))
      }
    }
  }

  pub fn parent_ref(&self) -> Option<VfsPath<&str>> {
    match self.0.len() {
      0 => None,
      1 => Some(VfsPath(Vec::new())),
      n => {
        let refs = self.0[0..n - 1]
          .iter()
          .map(String::as_str)
          .collect::<Vec<_>>();
        Some(VfsPath(refs))
      }
    }
  }
}

impl AsRef<VfsPath> for VfsPath {
  fn as_ref(&self) -> &VfsPath {
    self
  }
}

impl<S> VfsPath<S>
where
  S: Eq + Hash + AsRef<str>,
{
  pub fn basename(&self) -> Option<&str> {
    if !self.0.is_empty() {
      self.0.last().map(S::as_ref)
    } else {
      None
    }
  }
}

impl From<VfsPath<&str>> for VfsPath {
  fn from(value: VfsPath<&str>) -> Self {
    Self(value.0.into_iter().map(String::from).collect())
  }
}

impl<const N: usize> From<[&str; N]> for VfsPath {
  fn from(value: [&str; N]) -> Self {
    Self(Vec::from(value.map(str::to_string)))
  }
}

impl<'s, const N: usize> From<[&'s str; N]> for VfsPath<&'s str> {
  fn from(value: [&'s str; N]) -> Self {
    Self(Vec::from(value))
  }
}

impl<const N: usize> Equivalent<VfsPath> for [&str; N] {
  fn equivalent(&self, key: &VfsPath) -> bool {
    key.0.as_slice() == self
  }
}

impl Equivalent<VfsPath> for VfsPath<&str> {
  fn equivalent(&self, key: &VfsPath) -> bool {
    self.0 == key.0
  }
}

impl Equivalent<VfsPath> for &VfsPath<&str> {
  fn equivalent(&self, key: &VfsPath) -> bool {
    self.0 == key.0
  }
}

pub enum VfsNode<T> {
  Dir(String),
  Item { name: String, value: T },
}

impl<T> PartialEq for VfsNode<T> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (VfsNode::Dir(name), VfsNode::Dir(other_name)) => name.eq(other_name),
      (
        VfsNode::Dir(dir_name),
        VfsNode::Item {
          name: item_name, ..
        },
      ) => dir_name.eq(item_name),
      (
        VfsNode::Item {
          name: item_name, ..
        },
        VfsNode::Dir(dir_name),
      ) => item_name.eq(dir_name),
      (
        VfsNode::Item { name, .. },
        VfsNode::Item {
          name: other_name, ..
        },
      ) => name.eq(other_name),
    }
  }
}

impl<T> Eq for VfsNode<T> {}

impl<T> PartialOrd for VfsNode<T> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<T> Ord for VfsNode<T> {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      (VfsNode::Dir(name), VfsNode::Dir(other_name)) => name.cmp(other_name),
      (VfsNode::Dir(..), VfsNode::Item { .. }) => Ordering::Less,
      (VfsNode::Item { .. }, VfsNode::Dir(..)) => Ordering::Greater,
      (
        VfsNode::Item { name, .. },
        VfsNode::Item {
          name: other_name, ..
        },
      ) => name.cmp(other_name),
    }
  }
}

pub struct VfsDir<T> {
  nodes: BTreeSet<VfsNode<T>>,
}

impl<T> Default for VfsDir<T> {
  fn default() -> Self {
    Self { nodes: default() }
  }
}

impl<T> VfsDir<T> {
  pub fn add(&mut self, node: impl Into<VfsNode<T>>) {
    self.nodes.insert(node.into());
  }

  pub fn add_dir(&mut self, name: impl Into<String>) {
    self.add(VfsNode::Dir(name.into()));
  }

  pub fn add_item(&mut self, name: impl Into<String>, item: T) {
    self.add(VfsNode::Item {
      name: name.into(),
      value: item,
    });
  }

  pub fn get(&self, item_name: &str) -> Option<&VfsNode<T>> {
    self.nodes.iter().find(|n| match n {
      VfsNode::Dir(name) | VfsNode::Item { name, .. } => name == item_name,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::{Vfs, VfsNode, VfsPath};

  #[test]
  fn vfs_add_dir() {
    let mut vfs = Vfs::new();

    vfs.create(["dir"]).add_item("item", 1);

    let VfsNode::Item { value, .. } = vfs.get_node(VfsPath::from(["dir", "item"])).unwrap() else {
      panic!("Item not an item");
    };

    assert_eq!(*value, 1);

    let root = vfs.get_dir([]).unwrap();

    let VfsNode::Dir(dir) = root.get("dir").unwrap() else {
      panic!("Dir is not a dir")
    };

    assert_eq!(dir, "dir");
  }
}
