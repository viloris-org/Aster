//! Minimal entity storage.

use engine_core::{EngineResult, Handle, HandleAllocator};

/// Entity handle.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

/// Minimal ECS world with entity lifetime tracking.
#[derive(Clone, Debug, Default)]
pub struct World {
    allocator: HandleAllocator,
}

impl World {
    /// Spawns an empty entity.
    pub fn spawn(&mut self) -> EngineResult<Entity> {
        self.allocator.allocate().map(Entity)
    }

    /// Destroys a live entity.
    pub fn despawn(&mut self, entity: Entity) -> EngineResult<()> {
        self.allocator.free(entity.handle())
    }

    /// Returns whether an entity is currently live.
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.allocator.is_live(entity.handle())
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
}
