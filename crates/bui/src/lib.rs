mod reflection;
pub mod ui;
pub mod xml;

use bevy::{
  platform::collections::HashSet,
  prelude::*,
  reflect::{Reflectable, TypeRegistry},
  utils::TypeIdMap,
};
use derive_more::derive::From;
use itertools::{Either, Itertools};
use std::any::TypeId;
use ui::{
  AttrParams, Attribute, AttributeExtensions, Resources, SerializableAttribute,
  attrs::{self},
  elements,
  events::{ClickEventType, EventProducer, HoverEventType, LeaveEventType, UiEvent, UiEvents},
};
use xml::Attr;

const EVENT_PREFIX: &str = "event";
const SELF_PREFIX: &str = "self";

pub struct BuiPlugin {
  vtables: UiVTables,
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

    self.vtables.attrs.insert(
      TypeId::of::<A>(),
      AttrVTable {
        insert: |world, entity, value: Box<dyn Reflect + 'static>| {
          let value = value.take::<A>().map_err(|_| {
            let tp = A::get_type_registration().type_info().type_path();
            format!("Could not downcast {tp} to its underlying type")
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

  pub fn register_event<E: Reflectable + FromReflect + FromWorld>(&mut self) -> &mut Self {
    self.register_reflect::<E>();
    self.initializers.push(|app| {
      app.register_type::<E>();
      app.add_event::<UiEvent<E>>();

      let world = app.world_mut();
      let sys_id = world.register_system(
        |data: In<(Entity, Box<dyn Reflect>)>, mut writer: EventWriter<UiEvent<E>>| -> Result {
          let (entity, reflect) = &*data;
          let event = E::from_reflect(&**reflect).ok_or_else(|| {
            let registration = E::get_type_registration();
            let tp = registration.type_info().type_path();
            if let Some(ti) = reflect.get_represented_type_info() {
              format!("Could not make {tp} from {}", ti.type_path())
            } else {
              format!("Could not make {tp} from Reflect")
            }
          })?;

          writer.write(UiEvent::new(*entity, event));

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
        create: |world| Box::new(E::from_world(world)) as Box<dyn Reflect>,
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
          let attr = input.downcast_ref::<A>().ok_or("")?;

          let resources = A::Resources::from_world(world).ok_or("")?;
          let serialized = A::serialize(attr, resources)?;

          Ok(Box::new(serialized))
        },
        attr: |input, world| {
          let attr = input.downcast_ref::<A>().ok_or("")?;

          let resources = A::Resources::from_world(world).ok_or("")?;
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
  ) {
    for (entity, interaction, maybe_click, maybe_hover, maybe_leave) in &q_interactions {
      match interaction {
        Interaction::Pressed => {
          if let Some(click_type) = maybe_click {
            Self::fire_untyped_event(&mut commands, entity, **click_type, &ui_events);
          }
        }
        Interaction::Hovered => {
          if let Some(hover_type) = maybe_hover {
            Self::fire_untyped_event(&mut commands, entity, **hover_type, &ui_events);
          }
        }
        Interaction::None => {
          if let Some(leave_type) = maybe_leave {
            Self::fire_untyped_event(&mut commands, entity, **leave_type, &ui_events);
          }
        }
      }
    }
  }

  fn fire_untyped_event(
    commands: &mut Commands,
    entity: Entity,
    event_type: TypeId,
    ui_events: &UiEvents,
  ) {
    if let Some(sys) = ui_events.get(&event_type.clone()).cloned() {
      commands.queue(move |world: &mut World| -> Result {
        world.resource_scope(|world, vtables: Mut<UiVTables>| {
          if let Some(vtable) = vtables.events.get(&event_type) {
            let event = (vtable.create)(world);
            world.run_system_with(sys, (entity, event))??;
          }
          Ok(())
        })
      });
    }
  }
}

impl Plugin for BuiPlugin {
  fn build(&self, app: &mut App) {
    app.init_resource::<UiEvents>();
    app.insert_resource(self.vtables.clone());
    app.insert_resource(self.blacklist.clone());
    app.insert_resource(self.overrides.clone());

    for init in &self.initializers {
      (init)(app);
    }

    app.add_systems(Update, Self::interaction_system);
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

  pub fn register_event<E: Reflectable + FromReflect + FromWorld>(mut self) -> Self {
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

#[derive(Default, Resource, Clone)]
struct UiVTables {
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

#[derive(Clone)]
struct EventVTable {
  create: fn(&mut World) -> Box<dyn Reflect>,
}

#[derive(Clone)]
struct ReflectionVTable {
  from_reflect: fn(&dyn PartialReflect) -> Option<Box<dyn Reflect>>,
}

#[derive(Resource, Default, Deref, DerefMut, Clone)]
struct SerializationBlacklist(HashSet<TypeId>);

type ElOverrideFn = fn(&dyn Reflect, &World) -> Result<(Box<dyn Reflect>)>;
type AttrOverrideFn = fn(&dyn Reflect, &World) -> Result<(Option<Attr>, Box<dyn Reflect>)>;

#[derive(Clone)]
struct Overrides {
  el: ElOverrideFn,
  attr: AttrOverrideFn,
}

#[derive(Resource, Default, Deref, DerefMut, Clone)]
struct SerializationOverrides(TypeIdMap<Overrides>);

pub struct Bui {
  node: xml::Node,
}

impl Bui {
  pub fn parse_all(ui_xml: &str) -> Result<Vec<Self>, xml::ParseError> {
    xml::parse(ui_xml).map(|nodes| nodes.into_iter().map(|node| Self { node }).collect())
  }

  pub fn spawn(&self, world: &mut World) -> Result<Entity> {
    spawn_node(&self.node, world)
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
}

impl TryInto<String> for &Bui {
  type Error = BevyError;
  fn try_into(self) -> Result<String> {
    (&self.node).try_into()
  }
}

#[derive(Bundle)]
pub struct BuiPrime<T>
where
  T: Component,
{
  inner: T,
  primary_type: PrimaryType,
}

impl<T> BuiPrime<T>
where
  T: Component,
{
  pub fn new(component: T) -> Self {
    Self {
      inner: component,
      primary_type: PrimaryType::new::<T>(),
    }
  }
}

impl<T> Default for BuiPrime<T>
where
  T: Component + Default,
{
  fn default() -> Self {
    Self {
      inner: T::default(),
      primary_type: PrimaryType::new::<T>(),
    }
  }
}

#[derive(Component, From)]
struct PrimaryType(TypeId);

impl PrimaryType {
  fn new<T: 'static>() -> Self {
    Self(TypeId::of::<T>())
  }

  fn type_id(&self) -> TypeId {
    self.0
  }
}

fn spawn_node(node: &xml::Node, world: &mut World) -> Result<Entity> {
  match node {
    xml::Node::Tag(tag) => {
      let type_registry = world.resource::<AppTypeRegistry>().clone();
      let type_registry = type_registry.read();
      spawn_tag(tag, world, &type_registry)
    }
    xml::Node::Text(text) => Ok(spawn_text(text, world)),
  }
}

fn spawn_tag(tag: &xml::Tag, world: &mut World, type_registry: &TypeRegistry) -> Result<Entity> {
  let name = template_to_mod_path(tag.name());

  let registration = reflection::get_type_registration_from_name(&name, type_registry)?;
  let reflect_component = registration
    .data::<ReflectComponent>()
    .ok_or_else(|| format!("Type {name} does not have ReflectComponent"))?;

  let mut reflect = tag.attr(Attr::named("self")).map_or_else(
    || -> Result<Box<dyn Reflect>> {
      // use reflect default if there is no self attrib
      let reflect = registration
        .data::<ReflectDefault>()
        .map(|rd| rd.default())
        .ok_or_else(|| format!("Type {name} does not have ReflectDefault"))?;

      Ok(reflect)
    },
    |data: &str| -> Result<Box<dyn Reflect>> {
      let reflect = world.resource_scope(|_, vtables: Mut<UiVTables>| {
        reflection::deserialize_reflect(type_registry, registration, data, &vtables)
      })?;

      Ok(reflect)
    },
  )?;

  // after creation, any attrs that use self. are set on the new struct
  let (fields, rest): (Vec<_>, Vec<_>) = tag
    .attr_iter()
    .filter(|(k, _)| k.to_string() != SELF_PREFIX)
    .partition_map(|(k, v)| {
      k.prefix()
        .and_then(|prefix| (prefix == SELF_PREFIX).then_some(Either::Left((k.name(), v))))
        .unwrap_or(Either::Right((k, v)))
    });
  reflection::patch_struct_with_map(fields, &mut *reflect, type_registry)?;

  let (events, components): (Vec<_>, Vec<_>) = rest.into_iter().partition_map(|(k, v)| {
    k.prefix()
      .and_then(|prefix| (prefix == EVENT_PREFIX).then_some(Either::Left((k.name(), v))))
      .unwrap_or(Either::Right((k, v)))
  });

  // create children first, as they need the world
  let children = create_child_entities(tag.children(), world)?;

  // then the actual entity for this element
  let mut entity = world.spawn(PrimaryType::from(registration.type_id()));

  // add the reflected value to this entity
  reflect_component.insert(&mut entity, &*reflect, type_registry);

  // then add all children
  entity.add_children(&children);

  // add all events
  if let Err(err) = bind_events(events, &mut entity, type_registry) {
    let id = entity.id();
    world.despawn(id);
    return Err(err);
  }

  let entity = entity.id();

  // the world is free again and now the attributes can be created
  for (attr, value) in components {
    if let Err(err) = insert_attribute(attr, value, world, entity, type_registry) {
      world.despawn(entity);
      return Err(err);
    }
  }

  Ok(entity)
}

fn spawn_text(text: &str, world: &mut World) -> Entity {
  world
    .spawn((Text::new(text.to_string()), PrimaryType::new::<Text>()))
    .id()
}

fn create_child_entities<'c>(
  child_elements: impl IntoIterator<Item = &'c xml::Node>,
  world: &mut World,
) -> Result<Vec<Entity>> {
  let mut children = Vec::new();

  for child in child_elements {
    match spawn_node(child, world) {
      Ok(child) => children.push(child),
      Err(err) => {
        for child in children {
          world.despawn(child);
        }
        return Err(err);
      }
    };
  }

  Ok(children)
}

fn insert_attribute(
  attr: &xml::Attr,
  value: &str,
  world: &mut World,
  entity: Entity,
  type_registry: &TypeRegistry,
) -> Result {
  world.resource_scope(|world, vtables: Mut<UiVTables>| -> Result {
    let name = template_to_mod_path(attr.to_string());
    let reg = reflection::get_type_registration_from_name(&name, type_registry)?;
    let reflect = reflection::deserialize_reflect(type_registry, reg, value, &vtables)?;

    match vtables.attrs.get(&reg.type_id()) {
      Some(fns) => {
        (fns.insert)(world, entity, reflect)?;
      }
      None => {
        Err(format!("Type {attr} was not registered as an attribute"))?;
      }
    }

    Ok(())
  })
}

fn bind_events<K, V>(
  events: impl IntoIterator<Item = (K, V)>,
  entity: &mut EntityWorldMut,
  type_registry: &TypeRegistry,
) -> Result
where
  K: AsRef<str>,
  V: AsRef<str>,
{
  for (k, v) in events {
    let event_name = k.as_ref();
    let event_type = template_to_mod_path(v);

    match event_name {
      "onclick" => {
        let t = reflection::get_type_registration_from_name(&event_type, type_registry)?;
        entity.insert((EventProducer, ClickEventType::new(t.type_id())));
      }
      "onhover" => {
        let t = reflection::get_type_registration_from_name(&event_type, type_registry)?;
        entity.insert((EventProducer, HoverEventType::new(t.type_id())));
      }
      "onleave" => {
        let t = reflection::get_type_registration_from_name(&event_type, type_registry)?;
        entity.insert((EventProducer, LeaveEventType::new(t.type_id())));
      }
      evt => return Err(format!("Invalid event type {evt}"))?,
    }
  }

  Ok(())
}

fn template_to_mod_path(s: impl AsRef<str>) -> String {
  // replace . with :: so full path lookup works
  s.as_ref().replace(".", "::")
}

fn mod_path_to_template(s: impl AsRef<str>) -> String {
  // replace :: with . so it's xml compatible
  s.as_ref().replace("::", ".")
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

#[cfg(test)]
mod tests {
  use crate::Bui;
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
    {
      let app_type_registry = AppTypeRegistry::default();
      {
        let mut type_registry = app_type_registry.write();
        type_registry.register::<Example>();
      }
      world.insert_resource(app_type_registry);
    }

    let uis = Bui::parse_all(EXAMPLE_UI).unwrap();
    let ui = uis.first().unwrap();

    let entity = ui.spawn(&mut world).unwrap();

    let example_component = world.get::<Example>(entity).unwrap();

    assert_that(&example_component.field1).is_equal_to(123);
    assert_that(&example_component.field2.as_str()).is_equal_to("some text");
  }
}
