//! Transform hierarchy storage.

use std::collections::HashMap;

use engine_core::{math::Transform, EngineError, EngineResult};

use crate::Entity;

/// Parent/child transform hierarchy.
#[derive(Clone, Debug, Default)]
pub struct TransformHierarchy {
    locals: HashMap<Entity, Transform>,
    parents: HashMap<Entity, Entity>,
    children: HashMap<Entity, Vec<Entity>>,
}

impl TransformHierarchy {
    /// Sets or replaces the local transform for an entity.
    pub fn set_local(&mut self, entity: Entity, transform: Transform) {
        self.locals.insert(entity, transform);
    }

    /// Returns the local transform if present.
    pub fn local(&self, entity: Entity) -> Option<Transform> {
        self.locals.get(&entity).copied()
    }

    /// Sets a parent relationship. Parent and child must be distinct and acyclic.
    pub fn set_parent(&mut self, child: Entity, parent: Entity) -> EngineResult<()> {
        if child == parent {
            return Err(EngineError::other("entity cannot parent itself"));
        }
        if self.is_descendant(parent, child) {
            return Err(EngineError::other("transform hierarchy cycle rejected"));
        }
        self.clear_parent(child);
        self.parents.insert(child, parent);
        self.children.entry(parent).or_default().push(child);
        Ok(())
    }

    /// Clears any parent relationship for an entity.
    pub fn clear_parent(&mut self, child: Entity) {
        if let Some(parent) = self.parents.remove(&child) {
            if let Some(children) = self.children.get_mut(&parent) {
                children.retain(|candidate| *candidate != child);
            }
        }
    }

    /// Returns the parent for an entity.
    pub fn parent(&self, child: Entity) -> Option<Entity> {
        self.parents.get(&child).copied()
    }

    /// Returns a copy of the current children list.
    pub fn children(&self, parent: Entity) -> Vec<Entity> {
        self.children.get(&parent).cloned().unwrap_or_default()
    }

    fn is_descendant(&self, entity: Entity, possible_ancestor: Entity) -> bool {
        let mut current = Some(entity);
        while let Some(candidate) = current {
            if candidate == possible_ancestor {
                return true;
            }
            current = self.parent(candidate);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use engine_core::HandleAllocator;

    use super::*;

    fn entity(allocator: &mut HandleAllocator) -> Entity {
        Entity::from_handle(allocator.allocate().unwrap())
    }

    #[test]
    fn rejects_cycles() {
        let mut allocator = HandleAllocator::default();
        let root = entity(&mut allocator);
        let child = entity(&mut allocator);
        let mut hierarchy = TransformHierarchy::default();

        hierarchy.set_parent(child, root).unwrap();
        assert!(hierarchy.set_parent(root, child).is_err());
    }
}
