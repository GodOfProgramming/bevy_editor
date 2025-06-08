mod reflection;
pub mod ui;
pub mod xml;

use bevy::{
  asset::{AssetLoader, LoadContext, io},
  platform::collections::HashSet,
  prelude::*,
  reflect::{Reflectable, TypeRegistry},
  utils::TypeIdMap,
};
use derive_more::derive::From;
use itertools::{Either, Itertools};
use reflection::ReflectionError;
use std::{any::TypeId, borrow::Cow};
use ui::{
  AttrParams, Attribute, AttributeExtensions, Resources, SerializableAttribute,
  attrs::{self},
  elements,
  events::{
    BuiEvent, ClickEventType, EntityEvent, EventProducer, HoverEventType, LeaveEventType, UiEvents,
  },
};
use xml::Attr;

type GenericError = Box<dyn std::error::Error + Send + Sync>;

const EVENT_PREFIX: &str = "event";
const SELF: &str = "self";

const ON_CLICK_EVENT: &str = "onclick";
const ON_HOVER_EVENT: &str = "onhover";
const ON_LEAVE_EVENT: &str = "onleave";

const XML_SCOPE_SEPARATOR: &str = ".";
const RUST_SCOPE_SEPARATOR: &str = "::";

pub struct BuiPlugin {
  vtables: BuiResource,
  initializers: Vec<fn(&mut App)>,
  blacklist: SerializationBlacklist,
  overrides: SerializationOverrides,
}

impl Default for BuiPlugin {
  fn default() -> Self {
    Self::new()
  }
}

impl BuiPlugin {
  pub fn builder() -> BuiPluginBuilder {
    BuiPluginBuilder { inner: Self::new() }
  }

  fn new() -> Self {
    let mut this = Self {
      vtables: default(),
      initializers: default(),
      blacklist: default(),
      overrides: default(),
    };
    elements::register_all(&mut this);
    attrs::register_all(&mut this);
    this
  }

  pub fn register_element<E: Reflectable + FromReflect>(&mut self) -> &mut Self {
    self.register_reflect::<E>();
    self.initializers.push(|app| {
      app.register_type::<E>();
    });

    self
      .vtables
      .elements
      .insert(TypeId::of::<E>(), ElementVTable {});

    self
  }

  pub fn register_attr<A: Attribute + Reflectable + FromReflect>(&mut self) -> &mut Self {
    self.register_reflect::<A>();
    self.initializers.push(|app| {
      app.register_type::<A>();

      let world = app.world_mut();
      A::register_params(world);
    });

    let type_id = TypeId::of::<A>();

    self.vtables.attrs.insert(
      type_id,
      AttrVTable {
        insert: |world, entity, value: Box<dyn Reflect + 'static>| {
          let value = value.take::<A>().map_err(|value| {
            let value_type = value
              .get_represented_type_info()
              .map(|ti| ti.type_path())
              .unwrap_or("<Unknown Type>");
            let tp = A::get_type_registration().type_info().type_path();
            ReflectionError::invalid_cast(value_type, tp)
          })?;

          world.resource_scope(|world, mut params: Mut<AttrParams<A>>| -> Result {
            let params = params.get_mut(world);
            let bundle = value.construct(params)?;
            world.entity_mut(entity).insert(bundle);
            Ok(())
          })?;

          Ok(())
        },
      },
    );

    self
  }

  pub fn register_event<E: Reflectable + FromReflect + BuiEvent>(&mut self) -> &mut Self {
    self.register_reflect::<E>();
    self.initializers.push(|app| {
      app.register_type::<E>();
      app.add_event::<EntityEvent<E>>();

      let world = app.world_mut();
      let sys_id = world.register_system(
        |data: In<(Entity, Box<dyn Reflect>)>, mut writer: EventWriter<EntityEvent<E>>| -> Result {
          let (entity, reflect) = data.0;
          let event = reflect.take::<E>().map_err(|reflect| {
            let reflect_type = reflect
              .get_represented_type_info()
              .map(|ti| ti.type_path())
              .unwrap_or("<Unknown Type>");
            let tp = E::get_type_registration().type_info().type_path();
            ReflectionError::invalid_cast(reflect_type, tp)
          })?;

          writer.write(EntityEvent::new(entity, event));

          Ok(())
        },
      );

      if let Some(mut events) = app.world_mut().get_resource_mut::<UiEvents>() {
        events.add::<E>(sys_id)
      }
    });
    self.vtables.events.insert(
      TypeId::of::<E>(),
      EventVTable {
        create: |world, default_value| {
          let default_value = default_value
            .as_deref()
            .and_then(|default_value| E::from_reflect(default_value));
          Box::new(E::create(world, default_value)) as Box<dyn Reflect>
        },
      },
    );
    self
  }

  pub fn register_reflect<T: FromReflect>(&mut self) -> &mut Self {
    self.vtables.reflection.insert(
      TypeId::of::<T>(),
      ReflectionVTable {
        from_reflect: |partial| T::from_reflect(partial).map(|t| Box::new(t) as Box<dyn Reflect>),
      },
    );
    self
  }

  pub fn blacklist<T: 'static>(&mut self) -> &mut Self {
    self.blacklist.insert(TypeId::of::<T>());
    self
  }

  pub fn serialize_override<A>(&mut self) -> &mut Self
  where
    A: SerializableAttribute + 'static,
  {
    self.overrides.insert(
      TypeId::of::<A>(),
      Overrides {
        el: |input, world| {
          let attr = input
            .downcast_ref::<A>()
            .ok_or("Failed to downcast to concrete Attribute")?;

          let resources = A::Resources::from_world(world)
            .ok_or("Could not acquire all Attribute resources from world")?;
          let serialized = A::serialize(attr, resources)?;

          Ok(Box::new(serialized))
        },
        attr: |input, world| {
          let attr = input
            .downcast_ref::<A>()
            .ok_or("Failed to downcast to concrete Attribute")?;

          let resources = A::Resources::from_world(world)
            .ok_or("Could not acquire all Attribute resources from world")?;

          let serialized = A::serialize(attr, resources)?;

          let attr = attr
            .name_override()
            .map(|name_override| Attr::named(name_override).with_prefix(attr.prefix_override()));

          Ok((attr, Box::new(serialized)))
        },
      },
    );
    self
  }

  fn interaction_system(
    mut commands: Commands,
    ui_events: Res<UiEvents>,
    q_interactions: Query<
      (
        Entity,
        &Interaction,
        Option<&ClickEventType>,
        Option<&HoverEventType>,
        Option<&LeaveEventType>,
      ),
      (Changed<Interaction>, With<EventProducer>),
    >,
  ) -> Result {
    for (entity, interaction, maybe_click, maybe_hover, maybe_leave) in &q_interactions {
      match interaction {
        Interaction::Pressed => {
          if let Some(click_type) = maybe_click {
            Self::fire_untyped_event(
              &mut commands,
              entity,
              click_type.type_id(),
              click_type.initializer(),
              &ui_events,
            )?;
          }
        }
        Interaction::Hovered => {
          if let Some(hover_type) = maybe_hover {
            Self::fire_untyped_event(
              &mut commands,
              entity,
              hover_type.type_id(),
              hover_type.initializer(),
              &ui_events,
            )?;
          }
        }
        Interaction::None => {
          if let Some(leave_type) = maybe_leave {
            Self::fire_untyped_event(
              &mut commands,
              entity,
              leave_type.type_id(),
              leave_type.initializer(),
              &ui_events,
            )?;
          }
        }
      }
    }

    Ok(())
  }

  fn fire_untyped_event(
    commands: &mut Commands,
    entity: Entity,
    event_type: TypeId,
    default_value: Option<&dyn Reflect>,
    ui_events: &UiEvents,
  ) -> Result {
    if let Some(sys) = ui_events.get(&event_type.clone()).cloned() {
      let default_value = default_value
        .map(|reflect| reflect.reflect_clone())
        .transpose()?;
      commands.queue(move |world: &mut World| -> Result {
        world.resource_scope(|world, vtables: Mut<BuiResource>| {
          if let Some(vtable) = vtables.events.get(&event_type) {
            let event = (vtable.create)(world, default_value);
            world.run_system_with(sys, (entity, event))??;
          }
          Ok(())
        })
      });
    }

    Ok(())
  }
}

impl Plugin for BuiPlugin {
  fn build(&self, app: &mut App) {
    app
      .register_type::<PrimaryType>()
      .register_asset_loader(BuiLoader)
      .init_asset::<Bui>()
      .init_resource::<UiEvents>()
      .insert_resource(self.vtables.clone())
      .insert_resource(self.blacklist.clone())
      .insert_resource(self.overrides.clone())
      .add_systems(Update, Self::interaction_system);

    for init in &self.initializers {
      (init)(app);
    }
  }
}

pub struct BuiPluginBuilder {
  inner: BuiPlugin,
}

impl BuiPluginBuilder {
  pub fn register_element<E: Reflectable + FromReflect>(mut self) -> Self {
    self.inner.register_element::<E>();
    self
  }

  pub fn register_attr<A: Attribute + Reflectable + FromReflect>(mut self) -> Self {
    self.inner.register_attr::<A>();
    self
  }

  pub fn register_event<E: Reflectable + FromReflect + BuiEvent>(mut self) -> Self {
    self.inner.register_event::<E>();
    self
  }

  pub fn blacklist<T: 'static>(mut self) -> Self {
    self.inner.blacklist::<T>();
    self
  }

  pub fn serialize_override<O>(mut self) -> Self
  where
    O: SerializableAttribute + 'static,
  {
    self.inner.serialize_override::<O>();
    self
  }

  pub fn build(self) -> BuiPlugin {
    self.inner
  }
}

#[derive(thiserror::Error, Debug)]
enum BuiError {
  #[error("Type {0} was not registered as an Attribute")]
  UnregisteredAttribute(String),
  #[error("Event {0} is not valid for use")]
  UnregisteredEvent(String),
}

impl BuiError {
  fn unregistered_attribute(t: impl Into<String>) -> Self {
    Self::UnregisteredAttribute(t.into())
  }

  fn unregistered_event(t: impl Into<String>) -> Self {
    Self::UnregisteredEvent(t.into())
  }
}

#[derive(Default, Resource, Clone)]
pub struct BuiResource {
  elements: TypeIdMap<ElementVTable>,
  attrs: TypeIdMap<AttrVTable>,
  events: TypeIdMap<EventVTable>,
  reflection: TypeIdMap<ReflectionVTable>,
}

#[derive(Clone)]
struct ElementVTable {}

#[derive(Clone)]
struct AttrVTable {
  insert: fn(world: &mut World, entity: Entity, value: Box<dyn Reflect>) -> Result,
}

type EventCreatorFn = fn(&mut World, Option<Box<dyn Reflect>>) -> Box<dyn Reflect>;

#[derive(Clone)]
struct EventVTable {
  create: EventCreatorFn,
}

#[derive(Clone)]
struct ReflectionVTable {
  from_reflect: fn(&dyn PartialReflect) -> Option<Box<dyn Reflect>>,
}

#[derive(Resource, Default, Deref, DerefMut, Clone)]
struct SerializationBlacklist(HashSet<TypeId>);

type ElOverrideFn = fn(&dyn Reflect, &World) -> Result<Box<dyn Reflect>>;
type AttrOverrideFn = fn(&dyn Reflect, &World) -> Result<(Option<Attr>, Box<dyn Reflect>)>;

#[derive(Clone)]
struct Overrides {
  el: ElOverrideFn,
  attr: AttrOverrideFn,
}

#[derive(Resource, Default, Deref, DerefMut, Clone)]
struct SerializationOverrides(TypeIdMap<Overrides>);

#[derive(Asset, Reflect, Default)]
pub struct Bui {
  #[reflect(ignore)]
  node: xml::Node,
}

impl Bui {
  pub fn parse(ui_xml: &str) -> Result<Self, xml::ParseError> {
    xml::parse(ui_xml).map(|node| Self { node })
  }

  pub fn spawn(
    &self,
    entity_manager: &mut impl EntityManager,
    bui_resource: &BuiResource,
    type_registry: &TypeRegistry,
  ) -> Result<Entity> {
    spawn_node(&self.node, entity_manager, bui_resource, type_registry)
  }

  pub fn serialize(entity: Entity, world: &World) -> Result<Self> {
    let blacklist = world
      .get_resource::<SerializationBlacklist>()
      .map(|bl| &**bl);
    let overrides = world
      .get_resource::<SerializationOverrides>()
      .map(|ovrds| &**ovrds);

    xml::Node::serialize(entity, world, blacklist, overrides).map(|node| Self { node })
  }

  pub fn try_into_string(&self) -> Result<String> {
    self.try_into()
  }
}

impl TryInto<String> for &Bui {
  type Error = BevyError;
  fn try_into(self) -> Result<String> {
    (&self.node).try_into()
  }
}

struct BuiLoader;

impl AssetLoader for BuiLoader {
  type Asset = Bui;

  type Settings = ();

  type Error = GenericError;

  async fn load(
    &self,
    reader: &mut dyn io::Reader,
    _settings: &Self::Settings,
    _load_context: &mut LoadContext<'_>,
  ) -> Result<Self::Asset, Self::Error> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    let xml = String::from_utf8(bytes)?;
    let bui = Bui::parse(&xml)?;
    Ok(bui)
  }

  fn extensions(&self) -> &[&str] {
    &[".bui.xml"]
  }
}

#[derive(Component, From, Reflect)]
pub struct PrimaryType(TypeId);

impl Default for PrimaryType {
  fn default() -> Self {
    Self::new::<()>()
  }
}

impl PrimaryType {
  pub fn new<T: 'static>() -> Self {
    Self(TypeId::of::<T>())
  }

  pub fn type_id(&self) -> TypeId {
    self.0
  }
}

fn spawn_node(
  node: &xml::Node,
  entity_manager: &mut impl EntityManager,
  bui_resource: &BuiResource,
  type_registry: &TypeRegistry,
) -> Result<Entity> {
  match node {
    xml::Node::Tag(tag) => spawn_tag(tag, entity_manager, bui_resource, type_registry),
    xml::Node::Text(text) => Ok(spawn_text(text, entity_manager)),
  }
}

fn spawn_tag(
  tag: &xml::Tag,
  entity_manager: &mut impl EntityManager,
  bui_resource: &BuiResource,
  type_registry: &TypeRegistry,
) -> Result<Entity> {
  let name = deserialize_name(tag.name());

  let registration = reflection::get_type_registration_from_name(&name, type_registry)?;
  let reflect_component_data = registration
    .data::<ReflectComponent>()
    .ok_or_else(|| ReflectionError::missing_type_data(&name, stringify!(ReflectComponent)))?;

  let mut reflected_component_instance = tag.attr(Attr::named(SELF)).map_or_else(
    || -> Result<Box<dyn Reflect>> {
      // use reflect default if there is no self attrib
      let reflect = registration
        .data::<ReflectDefault>()
        .map(|rd| rd.default())
        .ok_or_else(|| ReflectionError::missing_type_data(&name, stringify!(ReflectDefault)))?;

      Ok(reflect)
    },
    |data: &str| -> Result<Box<dyn Reflect>> {
      let reflect =
        reflection::deserialize_reflect(type_registry, registration, data, bui_resource)?;

      Ok(reflect)
    },
  )?;

  // after creation, any attrs that use self. are set on the new struct
  let (fields, rest): (Vec<_>, Vec<_>) = tag
    .attr_iter()
    .filter(|(k, _)| k.to_string() != SELF)
    .partition_map(|(k, v)| {
      k.prefix()
        .and_then(|prefix| (prefix == SELF).then_some(Either::Left((k.name(), v))))
        .unwrap_or(Either::Right((k, v)))
    });
  reflection::patch_struct_with_map(fields, &mut *reflected_component_instance, type_registry)?;

  let (events, components): (Vec<_>, Vec<_>) = rest.into_iter().partition_map(|(k, v)| {
    k.prefix()
      .and_then(|prefix| (prefix == EVENT_PREFIX).then_some(Either::Left((k.name(), v))))
      .unwrap_or(Either::Right((k, v)))
  });

  let entity = entity_manager.spawn(PrimaryType::from(registration.type_id()));

  entity_manager.insert_reflect(
    entity,
    reflect_component_data,
    reflected_component_instance,
    type_registry,
  );

  // then add all children
  let children =
    create_child_entities(tag.children(), entity_manager, bui_resource, type_registry)?;
  entity_manager.add_children(entity, children);

  // add all events
  if let Err(err) = bind_events(events, entity, entity_manager, type_registry, bui_resource) {
    entity_manager.despawn(entity);
    return Err(err);
  }

  // the world is free again and now the attributes can be created
  for (attr, value) in components {
    if let Err(err) = insert_attribute(
      entity,
      attr,
      value,
      entity_manager,
      bui_resource,
      type_registry,
    ) {
      entity_manager.despawn(entity);
      return Err(err);
    }
  }

  Ok(entity)
}

fn spawn_text(text: &str, entity_manager: &mut impl EntityManager) -> Entity {
  entity_manager.spawn((Text::new(text.to_string()), PrimaryType::new::<Text>()))
}

fn create_child_entities<'c>(
  child_elements: impl IntoIterator<Item = &'c xml::Node>,
  entity_manager: &mut impl EntityManager,
  bui_resource: &BuiResource,
  type_registry: &TypeRegistry,
) -> Result<Vec<Entity>> {
  let mut children = Vec::new();

  for child in child_elements {
    match spawn_node(child, entity_manager, bui_resource, type_registry) {
      Ok(child) => children.push(child),
      Err(err) => {
        for child in children {
          entity_manager.despawn(child);
        }
        return Err(err);
      }
    };
  }

  Ok(children)
}

fn insert_attribute(
  entity: Entity,
  attr: &xml::Attr,
  value: &str,
  entity_manager: &mut impl EntityManager,
  bui_resource: &BuiResource,
  type_registry: &TypeRegistry,
) -> Result {
  let name = deserialize_name(attr.to_string());
  let reg = reflection::get_type_registration_from_name(&name, type_registry)?;

  if let Some(reflect_component_data) = reg.data::<ReflectComponent>() {
    let reflected_component_instance =
      reflection::deserialize_reflect(type_registry, reg, value, bui_resource)?;

    entity_manager.insert_reflect(
      entity,
      reflect_component_data,
      reflected_component_instance,
      type_registry,
    );
  } else if let Some(fns) = bui_resource.attrs.get(&reg.type_id()) {
    let reflected_component_instance =
      reflection::deserialize_reflect(type_registry, reg, value, bui_resource)?;

    let insert = fns.insert;
    entity_manager.with_world(move |world| {
      if let Err(e) = (insert)(world, entity, reflected_component_instance) {
        error!("{e}");
      }
    });
  } else {
    Err(BuiError::unregistered_attribute(attr))?;
  }

  Ok(())
}

fn bind_events<K, V>(
  events: impl IntoIterator<Item = (K, V)>,
  entity: Entity,
  entity_manager: &mut impl EntityManager,
  type_registry: &TypeRegistry,
  bui_res: &BuiResource,
) -> Result
where
  K: AsRef<str>,
  V: AsRef<str>,
{
  for (k, v) in events {
    let event_name = k.as_ref();

    // parse the type name and possible default value
    let type_name = v.as_ref();
    let (type_name, type_value) = if let Some((type_name, type_value)) = type_name.split_once('=') {
      (type_name, Some(type_value))
    } else {
      (type_name, None)
    };

    let type_name = if type_name.starts_with('"') {
      Cow::Borrowed(type_name)
    } else {
      Cow::Owned(format!("\"{type_name}\""))
    };

    let type_name = ron::de::from_str::<String>(&type_name)?;
    let event_type = deserialize_name(type_name);

    let type_value = type_value
      .map(|type_value| {
        let registration = reflection::get_type_registration_from_name(&event_type, type_registry)?;
        reflection::deserialize_reflect(type_registry, registration, type_value, bui_res)
      })
      .transpose()?;

    match event_name {
      ON_CLICK_EVENT => {
        let t = reflection::get_type_registration_from_name(&event_type, type_registry)?;

        entity_manager.insert(
          entity,
          (EventProducer, ClickEventType::new(t.type_id(), type_value)),
        );
      }
      ON_HOVER_EVENT => {
        let t = reflection::get_type_registration_from_name(&event_type, type_registry)?;
        entity_manager.insert(
          entity,
          (EventProducer, HoverEventType::new(t.type_id(), type_value)),
        );
      }
      ON_LEAVE_EVENT => {
        let t = reflection::get_type_registration_from_name(&event_type, type_registry)?;
        entity_manager.insert(
          entity,
          (EventProducer, LeaveEventType::new(t.type_id(), type_value)),
        );
      }
      evt => return Err(BuiError::unregistered_event(evt))?,
    }
  }

  Ok(())
}

fn deserialize_name(s: impl AsRef<str>) -> String {
  s.as_ref()
    .replace(XML_SCOPE_SEPARATOR, RUST_SCOPE_SEPARATOR)
}

fn serialize_name(s: impl AsRef<str>) -> String {
  s.as_ref()
    .replace(RUST_SCOPE_SEPARATOR, XML_SCOPE_SEPARATOR)
}

fn result_string<O, E>(result: &Result<O, E>) -> &str
where
  O: AsRef<str>,
  E: AsRef<str>,
{
  match result {
    Ok(s) => s.as_ref(),
    Err(s) => s.as_ref(),
  }
}

pub trait EntityManager {
  fn spawn(&mut self, bundle: impl Bundle) -> Entity;

  fn despawn(&mut self, entity: Entity);

  fn add_children(&mut self, entity: Entity, children: impl AsRef<[Entity]>);

  fn insert(&mut self, entity: Entity, bundle: impl Bundle);

  fn insert_reflect(
    &mut self,
    entity: Entity,
    reflect_component_data: &ReflectComponent,
    reflected_instance: Box<dyn Reflect>,
    type_registry: &TypeRegistry,
  );

  fn with_world(&mut self, f: impl FnOnce(&mut World) + Send + 'static);
}

impl EntityManager for Commands<'_, '_> {
  fn spawn(&mut self, bundle: impl Bundle) -> Entity {
    self.spawn(bundle).id()
  }

  fn despawn(&mut self, entity: Entity) {
    self.entity(entity).despawn();
  }

  fn add_children(&mut self, entity: Entity, children: impl AsRef<[Entity]>) {
    self.entity(entity).add_children(children.as_ref());
  }

  fn insert(&mut self, entity: Entity, bundle: impl Bundle) {
    self.entity(entity).insert(bundle);
  }

  fn insert_reflect(
    &mut self,
    entity: Entity,
    _reflect_component_data: &ReflectComponent,
    reflected_instance: Box<dyn Reflect>,
    _type_registry: &TypeRegistry,
  ) {
    let Some(ti) = reflected_instance.get_represented_type_info() else {
      return;
    };
    let type_id = ti.type_id();

    self.with_world(move |world| {
      world.resource_scope(move |world, bui_resource: Mut<BuiResource>| {
        let Some(avt) = bui_resource.attrs.get(&type_id) else {
          return;
        };

        if let Err(err) = (avt.insert)(world, entity, reflected_instance) {
          error!("{err}");
        }
      });
    });
  }

  fn with_world(&mut self, f: impl FnOnce(&mut World) + Send + 'static) {
    self.queue(f);
  }
}

impl EntityManager for World {
  fn spawn(&mut self, bundle: impl Bundle) -> Entity {
    self.spawn(bundle).id()
  }

  fn despawn(&mut self, entity: Entity) {
    self.entity_mut(entity).despawn();
  }

  fn add_children(&mut self, entity: Entity, children: impl AsRef<[Entity]>) {
    self.entity_mut(entity).add_children(children.as_ref());
  }

  fn insert(&mut self, entity: Entity, bundle: impl Bundle) {
    self.entity_mut(entity).insert(bundle);
  }

  fn insert_reflect(
    &mut self,
    entity: Entity,
    reflect_component_data: &ReflectComponent,
    reflected_instance: Box<dyn Reflect>,
    type_registry: &TypeRegistry,
  ) {
    let mut entity = self.entity_mut(entity);
    reflect_component_data.insert(&mut entity, &*reflected_instance, type_registry);
  }

  fn with_world(&mut self, f: impl FnOnce(&mut World) + Send + 'static) {
    (f)(self)
  }
}

#[cfg(test)]
mod tests {
  use crate::{Bui, BuiResource};
  use bevy::prelude::*;
  use speculoos::prelude::*;

  #[test]
  fn construct_type_by_xml() {
    const EXAMPLE_UI: &str = include_str!("../test/example_ui.xml");

    #[derive(Default, Component, Reflect)]
    #[reflect(Component)]
    #[reflect(Default)]
    struct Example {
      field1: i32,
      field2: String,
    }

    let mut world = World::default();
    world.init_resource::<BuiResource>();
    {
      let app_type_registry = AppTypeRegistry::default();
      {
        let mut type_registry = app_type_registry.write();
        type_registry.register::<Example>();
      }
      world.insert_resource(app_type_registry);
    }

    let ui = Bui::parse(EXAMPLE_UI).unwrap();

    let entity = world.resource_scope(|world, app_type_registry: Mut<AppTypeRegistry>| {
      world.resource_scope(|world, bui_resource: Mut<BuiResource>| {
        let type_registry = app_type_registry.read();
        ui.spawn(world, &bui_resource, &type_registry).unwrap()
      })
    });

    let example_component = world.get::<Example>(entity).unwrap();

    assert_that(&example_component.field1).is_equal_to(123);
    assert_that(&example_component.field2.as_str()).is_equal_to("some text");
  }
}
