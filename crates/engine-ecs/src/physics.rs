//! Physics scene components: rigidbody and collider lifecycle integration.

use std::any::Any;

use engine_physics::{BodyHandle, BodyKind, ColliderDesc, ColliderHandle, RigidbodyDesc};

use crate::world::Component;

/// Scene component that owns a physics rigidbody handle.
///
/// The component stores the creation parameters and the live handle assigned by
/// the backend. Systems are responsible for calling the backend; this component
/// only carries the data.
#[derive(Debug)]
pub struct RigidbodyComponent {
    /// Parameters used to create the body.
    pub desc: RigidbodyDesc,
    /// Live handle assigned by the physics backend, if spawned.
    pub handle: Option<BodyHandle>,
}

impl RigidbodyComponent {
    /// Creates a dynamic rigidbody component with default parameters.
    pub fn dynamic() -> Self {
        Self {
            desc: RigidbodyDesc::default(),
            handle: None,
        }
    }

    /// Creates a static rigidbody component.
    pub fn static_body() -> Self {
        Self {
            desc: RigidbodyDesc {
                kind: BodyKind::Static,
                ..RigidbodyDesc::default()
            },
            handle: None,
        }
    }
}

impl Component for RigidbodyComponent {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Scene component that owns a physics collider handle.
///
/// Attach alongside a [`RigidbodyComponent`] on the same entity. The collider
/// is attached to the body identified by `body_handle` once both are spawned.
#[derive(Debug)]
pub struct ColliderComponent {
    /// Parameters used to create the collider.
    pub desc: ColliderDesc,
    /// Live handle assigned by the physics backend, if spawned.
    pub handle: Option<ColliderHandle>,
}

impl ColliderComponent {
    /// Creates a box collider component with default parameters.
    pub fn new(desc: ColliderDesc) -> Self {
        Self { desc, handle: None }
    }
}

impl Component for ColliderComponent {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn rigidbody_component_attaches_to_entity() {
        let mut world = World::default();
        let entity = world.spawn().unwrap();
        world
            .insert_component(entity, RigidbodyComponent::dynamic())
            .unwrap();
        let rb = world.component_mut::<RigidbodyComponent>(entity).unwrap();
        assert!(rb.handle.is_none());
        assert_eq!(rb.desc.kind, BodyKind::Dynamic);
    }

    #[test]
    fn collider_component_attaches_to_entity() {
        let mut world = World::default();
        let entity = world.spawn().unwrap();
        world
            .insert_component(entity, ColliderComponent::new(ColliderDesc::default()))
            .unwrap();
        let col = world.component_mut::<ColliderComponent>(entity).unwrap();
        assert!(col.handle.is_none());
        assert!(!col.desc.is_trigger);
    }

    #[test]
    fn static_rigidbody_has_correct_kind() {
        let rb = RigidbodyComponent::static_body();
        assert_eq!(rb.desc.kind, BodyKind::Static);
    }
}
