//! Physics synchronization between ECS and the physics backend.

use std::collections::{HashMap, HashSet};

use engine_core::EntityId;
use engine_ecs::{ColliderComponentData, ComponentData, Scene};
use engine_physics::{
    BodyHandle, BodyKind, ColliderDesc, ColliderHandle, ColliderShape, PhysicsWorld, RigidbodyDesc,
};

/// Synchronizes ECS RigidbodyComponent/ColliderComponent with the physics backend.
///
/// PhysicsSync watches the Scene for entities with rigidbody and collider components,
/// creates corresponding physics bodies and colliders, and syncs transforms each fixed update.
pub struct PhysicsSync {
    /// Mapping from entity ID to (physics body handle, body kind).
    body_map: HashMap<EntityId, (BodyHandle, BodyKind)>,
    /// Mapping from (entity ID, collider index) to collider handle.
    collider_map: HashMap<(EntityId, usize), ColliderHandle>,
}

impl Default for PhysicsSync {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsSync {
    /// Creates a new PhysicsSync.
    pub fn new() -> Self {
        Self {
            body_map: HashMap::new(),
            collider_map: HashMap::new(),
        }
    }

    /// Synchronizes creation: creates physics bodies for entities with RigidbodyComponent
    /// that don't yet have a body handle, and creates colliders for ColliderComponents.
    pub fn sync_creation(
        &mut self,
        scene: &Scene,
        physics: &mut PhysicsWorld,
    ) -> engine_core::EngineResult<()> {
        for (entity, object) in scene.iter_objects() {
            // Find rigidbody component
            let rigidbody = object.components.iter().find_map(|c| {
                if let ComponentData::Rigidbody(rb) = c {
                    Some(rb)
                } else {
                    None
                }
            });

            let Some(rb_data) = rigidbody else {
                continue;
            };

            let body = match self.body_map.get(&object.id).copied() {
                Some((handle, _)) => handle,
                None => {
                    let local_transform = scene.transforms().local(entity).unwrap_or_default();
                    let body_kind = match rb_data.body_type.as_str() {
                        "static" => BodyKind::Static,
                        "kinematic" => BodyKind::Kinematic,
                        _ => BodyKind::Dynamic,
                    };
                    let desc = RigidbodyDesc {
                        transform: local_transform,
                        kind: body_kind,
                        linear_damping: rb_data.linear_damping,
                        angular_damping: rb_data.angular_damping,
                        gravity_scale: if rb_data.use_gravity { 1.0 } else { 0.0 },
                        ..RigidbodyDesc::default()
                    };
                    let new_body = physics.backend_mut().create_body(&desc)?;
                    self.body_map.insert(object.id, (new_body, body_kind));
                    new_body
                }
            };

            let collider_components = object.components.iter().filter_map(|c| {
                if let ComponentData::Collider(col) = c {
                    Some(col)
                } else {
                    None
                }
            });

            for (idx, collider_data) in collider_components.enumerate() {
                if !self.collider_map.contains_key(&(object.id, idx)) {
                    let (friction, restitution) =
                        physics_material_friction_restitution(&collider_data.physics_material);
                    let desc = ColliderDesc {
                        shape: collider_shape_from_data(collider_data),
                        friction,
                        restitution,
                        is_trigger: collider_data.is_trigger,
                        layer: object.layer,
                        mask: collider_data.mask,
                        friction_combine: engine_physics::CombineMode::Average,
                        restitution_combine: engine_physics::CombineMode::Average,
                        active_contact_events: false,
                    };
                    let collider_handle = physics.backend_mut().add_collider(body, &desc)?;
                    self.collider_map.insert((object.id, idx), collider_handle);
                }
            }
        }

        Ok(())
    }

    /// Synchronizes destruction: removes physics bodies for entities that have been destroyed.
    /// Returns entity IDs that were destroyed.
    pub fn sync_destruction(
        &mut self,
        scene: &Scene,
        physics: &mut PhysicsWorld,
    ) -> engine_core::EngineResult<Vec<EntityId>> {
        // Collect entity IDs that exist in the scene
        let active_entities: HashSet<_> = scene.iter_objects().map(|(_, obj)| obj.id).collect();

        let mut destroyed = Vec::new();

        // Find bodies whose entities no longer exist
        let bodies_to_remove: Vec<_> = self
            .body_map
            .iter()
            .filter(|(eid, _)| !active_entities.contains(eid))
            .map(|(eid, (handle, _))| (*eid, *handle))
            .collect();

        for (eid, handle) in bodies_to_remove {
            let colliders_to_remove: Vec<_> = self
                .collider_map
                .iter()
                .filter(|((entity_id, _), _)| *entity_id == eid)
                .map(|(_, collider)| *collider)
                .collect();

            for collider_handle in colliders_to_remove {
                let _ = physics.backend_mut().remove_collider(collider_handle);
            }
            self.collider_map
                .retain(|(entity_id, _), _| *entity_id != eid);

            let _ = physics.backend_mut().destroy_body(handle);
            self.body_map.remove(&eid);
            destroyed.push(eid);
        }

        Ok(destroyed)
    }

    /// Synchronizes transforms from ECS to physics (scene → physics).
    pub fn sync_transforms_to_physics(
        &self,
        scene: &Scene,
        physics: &mut PhysicsWorld,
    ) -> engine_core::EngineResult<()> {
        for (eid, (body_handle, _)) in &self.body_map {
            if let Some(entity) = scene.find_by_id(*eid) {
                if let Some(local_transform) = scene.transforms().local(entity) {
                    physics
                        .backend_mut()
                        .set_body_transform(*body_handle, local_transform)?;
                }
            }
        }
        Ok(())
    }

    /// Synchronizes transforms from physics to ECS (physics → scene).
    /// Only syncs dynamic and kinematic bodies — static bodies don't move.
    pub fn sync_transforms_from_physics(
        &self,
        scene: &mut Scene,
        physics: &mut PhysicsWorld,
    ) -> engine_core::EngineResult<()> {
        for (eid, (body_handle, body_kind)) in &self.body_map {
            if *body_kind == BodyKind::Static {
                continue;
            }
            if let Some(entity) = scene.find_by_id(*eid) {
                if let Ok(transform) = physics.backend().body_transform(*body_handle) {
                    scene.transforms_mut().set_local(entity, transform);
                }
            }
        }
        Ok(())
    }

    /// Clears all physics bindings.
    pub fn clear(&mut self) {
        self.body_map.clear();
        self.collider_map.clear();
    }

    /// Returns the number of physics bodies managed by this sync.
    pub fn body_count(&self) -> usize {
        self.body_map.len()
    }

    /// Returns the number of physics colliders managed by this sync.
    pub fn collider_count(&self) -> usize {
        self.collider_map.len()
    }
}

/// Helper to get friction/restitution from physics material name.
fn physics_material_friction_restitution(material: &str) -> (f32, f32) {
    match material {
        "metal" => (0.3, 0.5),
        "ice" => (0.05, 0.1),
        "rubber" => (0.9, 0.8),
        _ => (0.5, 0.0), // default
    }
}

/// Helper to convert ColliderComponentData to ColliderShape.
fn collider_shape_from_data(data: &ColliderComponentData) -> ColliderShape {
    let half = data.size * 0.5;
    match data.shape.as_str() {
        "sphere" => ColliderShape::Sphere {
            radius: half.x.max(half.y).max(half.z),
        },
        "capsule" => ColliderShape::Capsule {
            half_height: half.y,
            radius: half.x.max(half.z),
        },
        _ => ColliderShape::Box { half_extents: half },
    }
}

#[cfg(feature = "physics")]
#[cfg(test)]
mod tests {
    use super::*;
    use engine_ecs::{ColliderComponentData, ComponentData, RigidbodyComponentData};
    use engine_physics::{PhysicsWorld, SimplePhysicsBackend};

    #[test]
    fn physics_sync_creates_body_for_entity_with_rigidbody() {
        let mut scene = Scene::new();
        let entity = scene.create_object("TestObject").unwrap();
        scene
            .upsert_component(
                entity,
                ComponentData::Rigidbody(RigidbodyComponentData::default()),
            )
            .unwrap();

        let mut sync = PhysicsSync::new();
        let mut world = PhysicsWorld::new(SimplePhysicsBackend::new());

        sync.sync_creation(&scene, &mut world).unwrap();

        assert_eq!(sync.body_count(), 1);
    }

    #[test]
    fn physics_sync_creates_collider_for_entity_with_collider() {
        let mut scene = Scene::new();
        let entity = scene.create_object("TestObject").unwrap();
        scene
            .upsert_component(
                entity,
                ComponentData::Rigidbody(RigidbodyComponentData::default()),
            )
            .unwrap();
        scene
            .upsert_component(
                entity,
                ComponentData::Collider(ColliderComponentData::default()),
            )
            .unwrap();

        let mut sync = PhysicsSync::new();
        let mut world = PhysicsWorld::new(SimplePhysicsBackend::new());

        sync.sync_creation(&scene, &mut world).unwrap();

        assert_eq!(sync.body_count(), 1);
        assert_eq!(sync.collider_count(), 1);
    }

    #[test]
    fn physics_sync_removes_body_when_entity_destroyed() {
        let mut scene = Scene::new();
        let entity = scene.create_object("TestObject").unwrap();
        let object_id = scene.object(entity).unwrap().id;
        scene
            .upsert_component(
                entity,
                ComponentData::Rigidbody(RigidbodyComponentData::default()),
            )
            .unwrap();

        let mut sync = PhysicsSync::new();
        let mut world = PhysicsWorld::new(SimplePhysicsBackend::new());

        sync.sync_creation(&scene, &mut world).unwrap();
        assert_eq!(sync.body_count(), 1);

        // Destroy the entity
        scene.destroy_deferred(entity).unwrap();
        scene.process_deferred_destroy().unwrap();

        // Sync destruction
        let destroyed = sync.sync_destruction(&scene, &mut world).unwrap();
        assert!(destroyed.contains(&object_id));
        assert_eq!(sync.body_count(), 0);
    }

    #[test]
    fn physics_sync_skips_entities_without_rigidbody() {
        let mut scene = Scene::new();
        let _entity = scene.create_object("TestObject").unwrap();
        // No RigidbodyComponent

        let mut sync = PhysicsSync::new();
        let mut world = PhysicsWorld::new(SimplePhysicsBackend::new());

        sync.sync_creation(&scene, &mut world).unwrap();

        assert_eq!(sync.body_count(), 0);
    }

    #[test]
    fn physics_sync_transform_writeback_updates_dynamic_body() {
        let mut scene = Scene::new();
        let entity = scene.create_object("FallingObject").unwrap();
        scene
            .upsert_component(
                entity,
                ComponentData::Rigidbody(RigidbodyComponentData {
                    body_type: "dynamic".to_string(),
                    use_gravity: true,
                    ..RigidbodyComponentData::default()
                }),
            )
            .unwrap();

        let mut sync = PhysicsSync::new();
        let mut world = PhysicsWorld::new(SimplePhysicsBackend::new());

        sync.sync_creation(&scene, &mut world).unwrap();

        // Step physics (dynamic body falls due to gravity)
        world.fixed_update(1.0 / 60.0);

        // Write back transforms
        sync.sync_transforms_from_physics(&mut scene, &mut world)
            .unwrap();

        // Verify the entity's transform position changed (y should decrease due to gravity)
        let transform = scene.transforms().local(entity).unwrap();
        assert!(
            transform.translation.y < 0.0,
            "dynamic body should have fallen below origin, got y={}",
            transform.translation.y
        );
    }

    #[test]
    fn physics_sync_transform_writeback_skips_static_body() {
        let mut scene = Scene::new();
        let entity = scene.create_object("StaticObject").unwrap();
        scene
            .upsert_component(
                entity,
                ComponentData::Rigidbody(RigidbodyComponentData {
                    body_type: "static".to_string(),
                    ..RigidbodyComponentData::default()
                }),
            )
            .unwrap();

        let mut sync = PhysicsSync::new();
        let mut world = PhysicsWorld::new(SimplePhysicsBackend::new());

        sync.sync_creation(&scene, &mut world).unwrap();

        // Step physics
        world.fixed_update(1.0 / 60.0);

        // Write back transforms — static body should NOT be updated
        sync.sync_transforms_from_physics(&mut scene, &mut world)
            .unwrap();

        // Verify the entity's transform position is still at origin (default)
        let transform = scene.transforms().local(entity).unwrap();
        assert_eq!(transform.translation.y, 0.0, "static body should not move");
    }
}
