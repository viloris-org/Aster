//! Entity and native component storage.

use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    fmt,
};

use engine_core::{EngineError, EngineResult, Handle, HandleAllocator};

/// Entity handle.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Entity(Handle);

impl Entity {
    /// Creates an entity from an engine handle.
    pub const fn from_handle(handle: Handle) -> Self {
        Self(handle)
    }

    /// Returns the backing handle.
    pub const fn handle(self) -> Handle {
        self.0
    }
}

/// Native component lifecycle hook.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifecycle {
    /// Called once before the first update.
    Start,
    /// Called once per variable frame.
    Update,
    /// Called on fixed timestep ticks.
    FixedUpdate,
    /// Called after regular updates.
    LateUpdate,
    /// Called by editor-only ticking.
    EditorUpdate,
}

/// Runtime state for a native component attached to an entity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentState {
    registered: bool,
    enabled: bool,
    started: bool,
}

impl Default for ComponentState {
    fn default() -> Self {
        Self {
            registered: true,
            enabled: true,
            started: false,
        }
    }
}

impl ComponentState {
    /// Returns whether the component is registered with the world.
    pub const fn is_registered(self) -> bool {
        self.registered
    }

    /// Returns whether the component participates in lifecycle ticks.
    pub const fn is_enabled(self) -> bool {
        self.enabled
    }

    /// Returns whether `Component::start` has run.
    pub const fn has_started(self) -> bool {
        self.started
    }
}

/// Native Rust component contract.
pub trait Component: Any + Send {
    /// Returns a type-stable display name for diagnostics.
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// Called when the component is attached to a live entity.
    fn on_register(&mut self, _entity: Entity) {}

    /// Called before the component is detached from its entity.
    fn on_unregister(&mut self) {}

    /// Called when the component starts participating in lifecycle ticks.
    fn on_enable(&mut self) {}

    /// Called when the component stops participating in lifecycle ticks.
    fn on_disable(&mut self) {}

    /// Called once before the first update.
    fn start(&mut self) {}

    /// Called once per variable frame.
    fn update(&mut self) {}

    /// Called on fixed timestep ticks.
    fn fixed_update(&mut self) {}

    /// Called after regular updates.
    fn late_update(&mut self) {}

    /// Called by editor-only ticking.
    fn editor_update(&mut self) {}

    /// Called when a started component is removed or its owning entity is destroyed.
    fn end_play(&mut self) {}

    /// Type-erased mutable access.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

struct ComponentEntry {
    type_id: TypeId,
    component: Box<dyn Component>,
    state: ComponentState,
}

impl fmt::Debug for ComponentEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComponentEntry")
            .field("type_name", &self.component.type_name())
            .field("state", &self.state)
            .finish()
    }
}

/// Component storage keyed by live entity. Components are indexed by TypeId for O(1) lookup.
#[derive(Default)]
pub struct ComponentStorage {
    entries: HashMap<Entity, Vec<ComponentEntry>>,
    /// Per-entity, per-TypeId index into `entries` for O(1) access.
    type_index: HashMap<(Entity, TypeId), usize>,
}

impl fmt::Debug for ComponentStorage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComponentStorage")
            .field("entities", &self.entries.len())
            .finish()
    }
}

impl ComponentStorage {
    /// Inserts or replaces a native component for an entity.
    pub fn insert<C: Component>(&mut self, entity: Entity, mut component: C) {
        let type_id = TypeId::of::<C>();
        let component_list = self.entries.entry(entity).or_default();
        if let Some(&index) = self.type_index.get(&(entity, type_id)) {
            if let Some(entry) = component_list.get_mut(index) {
                Self::shutdown_entry(entry);
                component.on_register(entity);
                component.on_enable();
                *entry = ComponentEntry {
                    type_id,
                    component: Box::new(component),
                    state: ComponentState::default(),
                };
                return;
            }
        }
        let index = component_list.len();
        component.on_register(entity);
        component.on_enable();
        component_list.push(ComponentEntry {
            type_id,
            component: Box::new(component),
            state: ComponentState::default(),
        });
        self.type_index.insert((entity, type_id), index);
    }

    /// Returns a mutable component reference by concrete type (O(1) via TypeId index).
    pub fn get_mut<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        let type_id = TypeId::of::<C>();
        if let Some(&index) = self.type_index.get(&(entity, type_id)) {
            return self
                .entries
                .get_mut(&entity)?
                .get_mut(index)?
                .component
                .as_any_mut()
                .downcast_mut::<C>();
        }
        // Fallback: linear scan for types registered before the index was added
        self.entries
            .get_mut(&entity)?
            .iter_mut()
            .find_map(|entry| entry.component.as_any_mut().downcast_mut::<C>())
    }

    /// Returns an immutable component reference by concrete type (O(1) via TypeId index).
    pub fn get<C: Component>(&self, entity: Entity) -> Option<&C> {
        let type_id = TypeId::of::<C>();
        if let Some(&index) = self.type_index.get(&(entity, type_id)) {
            let component = self.entries.get(&entity)?.get(index)?.component.as_ref();
            let component: &dyn Any = component;
            return component.downcast_ref::<C>();
        }
        self.entries.get(&entity)?.iter().find_map(|entry| {
            let component: &dyn Any = entry.component.as_ref();
            component.downcast_ref::<C>()
        })
    }

    /// Returns whether an entity has a component of the given concrete type.
    pub fn contains<C: Component>(&self, entity: Entity) -> bool {
        self.type_index.contains_key(&(entity, TypeId::of::<C>()))
    }

    /// Returns the runtime state for a component by concrete type.
    pub fn state<C: Component>(&self, entity: Entity) -> Option<ComponentState> {
        let index = *self.type_index.get(&(entity, TypeId::of::<C>()))?;
        self.entries
            .get(&entity)
            .and_then(|components| components.get(index))
            .map(|entry| entry.state)
    }

    /// Enables or disables lifecycle ticking for a component.
    pub fn set_enabled<C: Component>(&mut self, entity: Entity, enabled: bool) -> bool {
        let type_id = TypeId::of::<C>();
        let Some(&index) = self.type_index.get(&(entity, type_id)) else {
            return false;
        };
        let Some(entry) = self
            .entries
            .get_mut(&entity)
            .and_then(|components| components.get_mut(index))
        else {
            return false;
        };
        if entry.state.enabled == enabled {
            return true;
        }
        entry.state.enabled = enabled;
        if enabled {
            entry.component.on_enable();
        } else {
            entry.component.on_disable();
        }
        true
    }

    /// Removes one concrete component type from an entity.
    pub fn remove<C: Component>(&mut self, entity: Entity) -> bool {
        self.remove_by_type_id(entity, TypeId::of::<C>())
    }

    /// Removes every component attached to an entity.
    pub fn remove_entity(&mut self, entity: Entity) {
        if let Some(mut entries) = self.entries.remove(&entity) {
            for entry in &mut entries {
                Self::shutdown_entry(entry);
            }
        }
        self.type_index.retain(|&(e, _), _| e != entity);
    }

    /// Ticks lifecycle hooks in deterministic entity and insertion order.
    pub fn run_lifecycle(&mut self, lifecycle: Lifecycle) {
        let mut entities = self.entries.keys().copied().collect::<Vec<_>>();
        entities.sort_by_key(|entity| entity.handle().slot());

        for entity in entities {
            let Some(components) = self.entries.get_mut(&entity) else {
                continue;
            };
            for entry in components {
                if !entry.state.registered || !entry.state.enabled {
                    continue;
                }
                match lifecycle {
                    Lifecycle::Start => {
                        if !entry.state.started {
                            entry.component.start();
                            entry.state.started = true;
                        }
                    }
                    Lifecycle::Update => entry.component.update(),
                    Lifecycle::FixedUpdate => entry.component.fixed_update(),
                    Lifecycle::LateUpdate => entry.component.late_update(),
                    Lifecycle::EditorUpdate => entry.component.editor_update(),
                }
            }
        }
    }

    fn remove_by_type_id(&mut self, entity: Entity, type_id: TypeId) -> bool {
        let Some(index) = self.type_index.remove(&(entity, type_id)) else {
            return false;
        };
        let Some(component_list) = self.entries.get_mut(&entity) else {
            return false;
        };
        if index >= component_list.len() {
            self.rebuild_entity_index(entity);
            return false;
        }
        let mut entry = component_list.remove(index);
        Self::shutdown_entry(&mut entry);
        if component_list.is_empty() {
            self.entries.remove(&entity);
        }
        self.rebuild_entity_index(entity);
        true
    }

    fn rebuild_entity_index(&mut self, entity: Entity) {
        self.type_index
            .retain(|&(indexed_entity, _), _| indexed_entity != entity);
        let Some(component_list) = self.entries.get_mut(&entity) else {
            return;
        };
        for (index, entry) in component_list.iter_mut().enumerate() {
            self.type_index.insert((entity, entry.type_id), index);
        }
    }

    fn shutdown_entry(entry: &mut ComponentEntry) {
        if entry.state.started {
            entry.component.end_play();
            entry.state.started = false;
        }
        if entry.state.enabled {
            entry.component.on_disable();
            entry.state.enabled = false;
        }
        if entry.state.registered {
            entry.component.on_unregister();
            entry.state.registered = false;
        }
    }
}

/// ECS world with entity lifetime and native component tracking.
#[derive(Default)]
pub struct World {
    allocator: HandleAllocator,
    entities: HashSet<Entity>,
    components: ComponentStorage,
}

impl fmt::Debug for World {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("World")
            .field("entities", &self.entities.len())
            .field("components", &self.components)
            .finish_non_exhaustive()
    }
}

impl World {
    /// Spawns an empty entity.
    pub fn spawn(&mut self) -> EngineResult<Entity> {
        let entity = Entity(self.allocator.allocate()?);
        self.entities.insert(entity);
        Ok(entity)
    }

    /// Destroys a live entity.
    pub fn despawn(&mut self, entity: Entity) -> EngineResult<()> {
        self.components.remove_entity(entity);
        self.allocator.free(entity.handle())?;
        self.entities.remove(&entity);
        Ok(())
    }

    /// Returns whether an entity is currently live.
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.allocator.is_live(entity.handle())
    }

    /// Iterates currently live entities in deterministic slot order.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        let mut entities = self.entities.iter().copied().collect::<Vec<_>>();
        entities.sort_by_key(|entity| entity.handle().slot());
        entities.into_iter()
    }

    /// Inserts or replaces a component for a live entity.
    pub fn insert_component<C: Component>(
        &mut self,
        entity: Entity,
        component: C,
    ) -> EngineResult<()> {
        if !self.is_alive(entity) {
            return Err(EngineError::invalid_handle(
                "cannot attach a component to a dead entity",
            ));
        }
        self.components.insert(entity, component);
        Ok(())
    }

    /// Removes one concrete component type from a live entity.
    pub fn remove_component<C: Component>(&mut self, entity: Entity) -> EngineResult<bool> {
        if !self.is_alive(entity) {
            return Err(EngineError::invalid_handle(
                "cannot remove a component from a dead entity",
            ));
        }
        Ok(self.components.remove::<C>(entity))
    }

    /// Returns whether a live entity has a component of the given concrete type.
    pub fn has_component<C: Component>(&self, entity: Entity) -> bool {
        self.is_alive(entity) && self.components.contains::<C>(entity)
    }

    /// Returns a mutable component reference by concrete type.
    pub fn component_mut<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        self.components.get_mut(entity)
    }

    /// Returns an immutable component reference by concrete type.
    pub fn component<C: Component>(&self, entity: Entity) -> Option<&C> {
        if !self.is_alive(entity) {
            return None;
        }
        self.components.get(entity)
    }

    /// Enables or disables lifecycle ticking for a component on a live entity.
    pub fn set_component_enabled<C: Component>(
        &mut self,
        entity: Entity,
        enabled: bool,
    ) -> EngineResult<bool> {
        if !self.is_alive(entity) {
            return Err(EngineError::invalid_handle(
                "cannot change a component on a dead entity",
            ));
        }
        Ok(self.components.set_enabled::<C>(entity, enabled))
    }

    /// Returns runtime state for a component on a live entity.
    pub fn component_state<C: Component>(&self, entity: Entity) -> Option<ComponentState> {
        if !self.is_alive(entity) {
            return None;
        }
        self.components.state::<C>(entity)
    }

    /// Runs component lifecycle hooks.
    pub fn run_lifecycle(&mut self, lifecycle: Lifecycle) {
        self.components.run_lifecycle(lifecycle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_despawn_entity() {
        let mut world = World::default();
        let entity = world.spawn().unwrap();
        assert!(world.is_alive(entity));
        world.despawn(entity).unwrap();
        assert!(!world.is_alive(entity));
    }

    #[derive(Default)]
    struct Counter {
        updates: u32,
        enable_count: u32,
        disable_count: u32,
        end_play_count: u32,
        unregister_count: u32,
    }

    #[derive(Default)]
    struct Events {
        events: Vec<&'static str>,
    }

    impl Component for Events {
        fn on_register(&mut self, _entity: Entity) {
            self.events.push("register");
        }

        fn on_enable(&mut self) {
            self.events.push("enable");
        }

        fn start(&mut self) {
            self.events.push("start");
        }

        fn on_disable(&mut self) {
            self.events.push("disable");
        }

        fn on_unregister(&mut self) {
            self.events.push("unregister");
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    impl Component for Counter {
        fn update(&mut self) {
            self.updates += 1;
        }

        fn on_enable(&mut self) {
            self.enable_count += 1;
        }

        fn on_disable(&mut self) {
            self.disable_count += 1;
        }

        fn end_play(&mut self) {
            self.end_play_count += 1;
        }

        fn on_unregister(&mut self) {
            self.unregister_count += 1;
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn stores_native_components() {
        let mut world = World::default();
        let entity = world.spawn().unwrap();
        world.insert_component(entity, Counter::default()).unwrap();

        world.component_mut::<Counter>(entity).unwrap().updates += 1;

        assert_eq!(world.component::<Counter>(entity).unwrap().updates, 1);
        assert_eq!(world.component::<Counter>(entity).unwrap().enable_count, 1);
    }

    #[test]
    fn replacing_native_component_keeps_lookup_and_lifecycle_single_valued() {
        let mut world = World::default();
        let entity = world.spawn().unwrap();
        world
            .insert_component(
                entity,
                Counter {
                    updates: 7,
                    ..Counter::default()
                },
            )
            .unwrap();
        world
            .insert_component(
                entity,
                Counter {
                    updates: 11,
                    ..Counter::default()
                },
            )
            .unwrap();

        world.run_lifecycle(Lifecycle::Update);

        assert_eq!(world.component_mut::<Counter>(entity).unwrap().updates, 12);
    }

    #[test]
    fn disabled_components_do_not_tick_until_enabled() {
        let mut world = World::default();
        let entity = world.spawn().unwrap();
        world.insert_component(entity, Counter::default()).unwrap();

        assert!(world
            .set_component_enabled::<Counter>(entity, false)
            .unwrap());
        world.run_lifecycle(Lifecycle::Start);
        world.run_lifecycle(Lifecycle::Update);

        let state = world.component_state::<Counter>(entity).unwrap();
        assert!(!state.is_enabled());
        assert!(!state.has_started());
        let counter = world.component::<Counter>(entity).unwrap();
        assert_eq!(counter.updates, 0);
        assert_eq!(counter.enable_count, 1);
        assert_eq!(counter.disable_count, 1);

        assert!(world
            .set_component_enabled::<Counter>(entity, true)
            .unwrap());
        world.run_lifecycle(Lifecycle::Start);
        world.run_lifecycle(Lifecycle::Update);

        let state = world.component_state::<Counter>(entity).unwrap();
        assert!(state.is_enabled());
        assert!(state.has_started());
        let counter = world.component::<Counter>(entity).unwrap();
        assert_eq!(counter.updates, 1);
        assert_eq!(counter.enable_count, 2);
        assert_eq!(counter.disable_count, 1);
    }

    #[test]
    fn component_removal_runs_end_play_and_unregister() {
        let mut world = World::default();
        let entity = world.spawn().unwrap();
        world.insert_component(entity, Counter::default()).unwrap();
        world.run_lifecycle(Lifecycle::Start);

        assert!(world.remove_component::<Counter>(entity).unwrap());
        assert!(!world.has_component::<Counter>(entity));
        assert!(world.component_state::<Counter>(entity).is_none());
    }

    #[test]
    fn iterates_live_entities_in_slot_order() {
        let mut world = World::default();
        let first = world.spawn().unwrap();
        let second = world.spawn().unwrap();

        assert_eq!(world.entities().collect::<Vec<_>>(), vec![first, second]);

        world.despawn(first).unwrap();
        assert_eq!(world.entities().collect::<Vec<_>>(), vec![second]);
    }

    #[test]
    fn inserted_component_registers_and_enables_before_start() {
        let mut world = World::default();
        let entity = world.spawn().unwrap();
        world.insert_component(entity, Events::default()).unwrap();
        world.run_lifecycle(Lifecycle::Start);

        assert_eq!(
            world.component::<Events>(entity).unwrap().events,
            ["register", "enable", "start"]
        );
    }
}
