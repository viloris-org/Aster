#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Physics abstraction and null backend for the Aster engine.
//!
//! The null backend compiles everywhere and satisfies the trait contract without
//! linking any physics library. A real backend (Rapier, Jolt, …) replaces it by
//! implementing [`PhysicsBackend`] and registering it at startup.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use engine_core::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};

pub use engine_core::math::{Quat, Transform, Vec3};

// ── Primitive types ──────────────────────────────────────────────────────────

/// Opaque handle to a physics body.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BodyHandle(pub u64);

/// Opaque handle to a collider.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ColliderHandle(pub u64);

/// Physics body kind.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyKind {
    /// Fully simulated body.
    #[default]
    Dynamic,
    /// Moved by the user, pushes dynamic bodies.
    Kinematic,
    /// Never moves.
    Static,
}

/// Rigidbody creation parameters.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct RigidbodyDesc {
    /// Initial world-space transform.
    pub transform: Transform,
    /// Body kind.
    pub kind: BodyKind,
    /// Linear damping coefficient.
    pub linear_damping: f32,
    /// Angular damping coefficient.
    pub angular_damping: f32,
    /// Gravity scale multiplier.
    pub gravity_scale: f32,
}

impl Default for RigidbodyDesc {
    fn default() -> Self {
        Self {
            transform: Transform::IDENTITY,
            kind: BodyKind::Dynamic,
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 1.0,
        }
    }
}

// ── Collider shapes ──────────────────────────────────────────────────────────

/// Collider shape.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ColliderShape {
    /// Axis-aligned box.
    Box {
        /// Half-extents on each axis.
        half_extents: Vec3,
    },
    /// Sphere.
    Sphere {
        /// Radius.
        radius: f32,
    },
    /// Capsule aligned along the Y axis.
    Capsule {
        /// Half-height of the cylindrical section.
        half_height: f32,
        /// Radius of the end caps.
        radius: f32,
    },
    /// Convex mesh approximation (vertex soup).
    Mesh {
        /// Flat list of vertex positions (x,y,z triplets).
        vertices: Vec<f32>,
    },
}

/// Collider creation parameters.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ColliderDesc {
    /// Shape.
    pub shape: ColliderShape,
    /// Friction coefficient.
    pub friction: f32,
    /// Restitution (bounciness) coefficient.
    pub restitution: f32,
    /// When true the collider fires overlap events instead of resolving contacts.
    pub is_trigger: bool,
    /// Collision layer this collider belongs to.
    pub layer: u32,
    /// Bitmask of layers this collider collides with.
    pub mask: u32,
}

impl Default for ColliderDesc {
    fn default() -> Self {
        Self {
            shape: ColliderShape::Box {
                half_extents: Vec3::new(0.5, 0.5, 0.5),
            },
            friction: 0.5,
            restitution: 0.0,
            is_trigger: false,
            layer: 1,
            mask: !0,
        }
    }
}

// ── Contact callbacks ────────────────────────────────────────────────────────

/// A contact event between two bodies.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactEvent {
    /// First body.
    pub body_a: BodyHandle,
    /// Second body.
    pub body_b: BodyHandle,
    /// Contact point in world space.
    pub point: Vec3,
    /// Contact normal pointing from B toward A.
    pub normal: Vec3,
    /// Whether this is an enter (true) or exit (false) event.
    pub entered: bool,
}

// ── Query types ──────────────────────────────────────────────────────────────

/// A single raycast hit.
#[derive(Clone, Debug, PartialEq)]
pub struct RayHit {
    /// Hit body.
    pub body: BodyHandle,
    /// Hit collider.
    pub collider: ColliderHandle,
    /// Hit point in world space.
    pub point: Vec3,
    /// Surface normal at the hit point.
    pub normal: Vec3,
    /// Distance from ray origin.
    pub distance: f32,
}

/// A single overlap result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlapResult {
    /// Overlapping body.
    pub body: BodyHandle,
    /// Overlapping collider.
    pub collider: ColliderHandle,
}

/// Query filter controlling which layers are tested.
#[derive(Clone, Copy, Debug, Default)]
pub struct QueryFilter {
    /// Layer mask; zero means test all layers.
    pub mask: u32,
}

// ── Layer matrix ─────────────────────────────────────────────────────────────

/// 32-layer collision matrix.
#[derive(Clone, Debug)]
pub struct LayerMatrix {
    rows: [u32; 32],
}

impl Default for LayerMatrix {
    fn default() -> Self {
        // All layers collide with all layers by default.
        Self { rows: [!0u32; 32] }
    }
}

impl LayerMatrix {
    /// Returns whether layer `a` collides with layer `b`.
    pub fn collides(&self, a: u32, b: u32) -> bool {
        let a = (a as usize).min(31);
        let b = (b as usize).min(31);
        self.rows[a] & (1 << b) != 0
    }

    /// Sets whether layer `a` collides with layer `b` (symmetric).
    pub fn set(&mut self, a: u32, b: u32, enabled: bool) {
        let a = (a as usize).min(31);
        let b = (b as usize).min(31);
        if enabled {
            self.rows[a] |= 1 << b;
            self.rows[b] |= 1 << a;
        } else {
            self.rows[a] &= !(1 << b);
            self.rows[b] &= !(1 << a);
        }
    }
}

// ── Backend trait ────────────────────────────────────────────────────────────

/// Pluggable physics backend contract.
///
/// Implementations are expected to step the simulation on `fixed_update` and
/// synchronise body transforms with the ECS on `sync_transforms`.
pub trait PhysicsBackend: Send + Sync {
    /// Advances the simulation by `dt` seconds.
    fn fixed_update(&mut self, dt: f32);

    /// Creates a rigidbody and returns its handle.
    fn create_body(&mut self, desc: &RigidbodyDesc) -> EngineResult<BodyHandle>;

    /// Destroys a body and all attached colliders.
    fn destroy_body(&mut self, body: BodyHandle) -> EngineResult<()>;

    /// Attaches a collider to a body.
    fn add_collider(
        &mut self,
        body: BodyHandle,
        desc: &ColliderDesc,
    ) -> EngineResult<ColliderHandle>;

    /// Removes a collider.
    fn remove_collider(&mut self, collider: ColliderHandle) -> EngineResult<()>;

    /// Returns the current world-space transform of a body.
    fn body_transform(&self, body: BodyHandle) -> EngineResult<Transform>;

    /// Teleports a body to a new world-space transform.
    fn set_body_transform(&mut self, body: BodyHandle, transform: Transform) -> EngineResult<()>;

    /// Applies a linear impulse to a body.
    fn apply_impulse(&mut self, body: BodyHandle, impulse: Vec3) -> EngineResult<()>;

    /// Casts a ray and returns the closest hit, if any.
    fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        filter: QueryFilter,
    ) -> Option<RayHit>;

    /// Returns all colliders overlapping a sphere.
    fn overlap_sphere(&self, center: Vec3, radius: f32, filter: QueryFilter) -> Vec<OverlapResult>;

    /// Sweeps a sphere along a direction and returns the first hit, if any.
    fn sweep_sphere(
        &self,
        center: Vec3,
        radius: f32,
        direction: Vec3,
        max_distance: f32,
        filter: QueryFilter,
    ) -> Option<RayHit>;

    /// Drains pending contact events since the last call.
    fn drain_contacts(&mut self) -> Vec<ContactEvent>;
}

// ── Null backend ─────────────────────────────────────────────────────────────

/// No-op physics backend. Compiles everywhere; produces no simulation.
#[derive(Default)]
pub struct NullPhysicsBackend;

impl PhysicsBackend for NullPhysicsBackend {
    fn fixed_update(&mut self, _dt: f32) {}

    fn create_body(&mut self, _desc: &RigidbodyDesc) -> EngineResult<BodyHandle> {
        Err(EngineError::other("null physics backend"))
    }

    fn destroy_body(&mut self, _body: BodyHandle) -> EngineResult<()> {
        Ok(())
    }

    fn add_collider(
        &mut self,
        _body: BodyHandle,
        _desc: &ColliderDesc,
    ) -> EngineResult<ColliderHandle> {
        Err(EngineError::other("null physics backend"))
    }

    fn remove_collider(&mut self, _collider: ColliderHandle) -> EngineResult<()> {
        Ok(())
    }

    fn body_transform(&self, _body: BodyHandle) -> EngineResult<Transform> {
        Err(EngineError::other("null physics backend"))
    }

    fn set_body_transform(&mut self, _body: BodyHandle, _transform: Transform) -> EngineResult<()> {
        Ok(())
    }

    fn apply_impulse(&mut self, _body: BodyHandle, _impulse: Vec3) -> EngineResult<()> {
        Ok(())
    }

    fn raycast(
        &self,
        _origin: Vec3,
        _direction: Vec3,
        _max_distance: f32,
        _filter: QueryFilter,
    ) -> Option<RayHit> {
        None
    }

    fn overlap_sphere(
        &self,
        _center: Vec3,
        _radius: f32,
        _filter: QueryFilter,
    ) -> Vec<OverlapResult> {
        Vec::new()
    }

    fn sweep_sphere(
        &self,
        _center: Vec3,
        _radius: f32,
        _direction: Vec3,
        _max_distance: f32,
        _filter: QueryFilter,
    ) -> Option<RayHit> {
        None
    }

    fn drain_contacts(&mut self) -> Vec<ContactEvent> {
        Vec::new()
    }
}

// ── World-level physics context ───────────────────────────────────────────────

/// Physics world that owns a backend and the layer matrix.
pub struct PhysicsWorld {
    backend: Box<dyn PhysicsBackend>,
    /// Layer collision matrix.
    pub layer_matrix: LayerMatrix,
}

impl fmt::Debug for PhysicsWorld {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PhysicsWorld")
            .field("layer_matrix", &self.layer_matrix)
            .finish_non_exhaustive()
    }
}

impl PhysicsWorld {
    /// Creates a physics world with the given backend.
    pub fn new(backend: impl PhysicsBackend + 'static) -> Self {
        Self {
            backend: Box::new(backend),
            layer_matrix: LayerMatrix::default(),
        }
    }

    /// Creates a physics world backed by the null backend.
    pub fn null() -> Self {
        Self::new(NullPhysicsBackend)
    }

    /// Steps the simulation.
    pub fn fixed_update(&mut self, dt: f32) {
        self.backend.fixed_update(dt);
    }

    /// Delegates to the backend.
    pub fn backend_mut(&mut self) -> &mut dyn PhysicsBackend {
        self.backend.as_mut()
    }

    /// Delegates to the backend (read-only).
    pub fn backend(&self) -> &dyn PhysicsBackend {
        self.backend.as_ref()
    }
}

// ── Simple deterministic backend ─────────────────────────────────────────────

#[derive(Clone, Debug)]
struct SimpleBody {
    desc: RigidbodyDesc,
    transform: Transform,
    velocity: Vec3,
    colliders: Vec<ColliderHandle>,
}

#[derive(Clone, Debug)]
struct SimpleCollider {
    body: BodyHandle,
    desc: ColliderDesc,
}

/// Small deterministic physics backend used until a native Rapier/Jolt backend is wired.
///
/// The backend supports rigidbody creation, collider lifetime, gravity for dynamic
/// bodies, sphere/box overlap, raycast, sphere sweep, and enter/exit events. It is
/// intentionally conservative: collision resolution is not attempted yet, so game
/// code can rely on queries and triggers while the engine keeps a dependency-light
/// default path.
#[derive(Debug)]
pub struct SimplePhysicsBackend {
    next_body: u64,
    next_collider: u64,
    bodies: HashMap<BodyHandle, SimpleBody>,
    colliders: HashMap<ColliderHandle, SimpleCollider>,
    active_pairs: HashSet<(ColliderHandle, ColliderHandle)>,
    contacts: Vec<ContactEvent>,
    gravity: Vec3,
}

impl Default for SimplePhysicsBackend {
    fn default() -> Self {
        Self {
            next_body: 1,
            next_collider: 1,
            bodies: HashMap::new(),
            colliders: HashMap::new(),
            active_pairs: HashSet::new(),
            contacts: Vec::new(),
            gravity: Vec3::new(0.0, -9.81, 0.0),
        }
    }
}

impl SimplePhysicsBackend {
    /// Creates a new simple physics backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of live bodies.
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Returns the number of live colliders.
    pub fn collider_count(&self) -> usize {
        self.colliders.len()
    }

    fn body(&self, handle: BodyHandle) -> EngineResult<&SimpleBody> {
        self.bodies
            .get(&handle)
            .ok_or_else(|| EngineError::invalid_handle("physics body does not exist"))
    }

    fn body_mut(&mut self, handle: BodyHandle) -> EngineResult<&mut SimpleBody> {
        self.bodies
            .get_mut(&handle)
            .ok_or_else(|| EngineError::invalid_handle("physics body does not exist"))
    }

    fn collider_world_sphere(&self, collider: ColliderHandle) -> Option<(Vec3, f32)> {
        let collider = self.colliders.get(&collider)?;
        let body = self.bodies.get(&collider.body)?;
        Some(shape_world_sphere(
            body.transform.translation,
            &collider.desc.shape,
        ))
    }

    fn collide(&self, a: ColliderHandle, b: ColliderHandle) -> Option<ContactEvent> {
        let collider_a = self.colliders.get(&a)?;
        let collider_b = self.colliders.get(&b)?;
        if collider_a.body == collider_b.body {
            return None;
        }
        if !layers_match(&collider_a.desc, &collider_b.desc) {
            return None;
        }
        let (center_a, radius_a) = self.collider_world_sphere(a)?;
        let (center_b, radius_b) = self.collider_world_sphere(b)?;
        let delta = center_a - center_b;
        let distance_squared = delta.length_squared();
        let radius = radius_a + radius_b;
        if distance_squared > radius * radius {
            return None;
        }
        let normal = if distance_squared <= f32::EPSILON {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            delta.normalized()
        };
        Some(ContactEvent {
            body_a: collider_a.body,
            body_b: collider_b.body,
            point: center_b + normal * radius_b,
            normal,
            entered: true,
        })
    }

    fn update_contacts(&mut self) {
        let handles = self.colliders.keys().copied().collect::<Vec<_>>();
        let mut current_pairs = HashSet::new();
        for (index, left) in handles.iter().enumerate() {
            for right in handles.iter().skip(index + 1) {
                let pair = ordered_pair(*left, *right);
                if let Some(mut event) = self.collide(*left, *right) {
                    current_pairs.insert(pair);
                    if !self.active_pairs.contains(&pair) {
                        event.entered = true;
                        self.contacts.push(event);
                    }
                }
            }
        }
        for pair in self.active_pairs.difference(&current_pairs) {
            if let (Some(left), Some(right)) =
                (self.colliders.get(&pair.0), self.colliders.get(&pair.1))
            {
                self.contacts.push(ContactEvent {
                    body_a: left.body,
                    body_b: right.body,
                    point: Vec3::ZERO,
                    normal: Vec3::ZERO,
                    entered: false,
                });
            }
        }
        self.active_pairs = current_pairs;
    }
}

impl PhysicsBackend for SimplePhysicsBackend {
    fn fixed_update(&mut self, dt: f32) {
        for body in self.bodies.values_mut() {
            if body.desc.kind == BodyKind::Dynamic {
                body.velocity += self.gravity * body.desc.gravity_scale * dt;
                body.transform.translation += body.velocity * dt;
            }
        }
        self.update_contacts();
    }

    fn create_body(&mut self, desc: &RigidbodyDesc) -> EngineResult<BodyHandle> {
        let handle = BodyHandle(self.next_body);
        self.next_body = self.next_body.saturating_add(1).max(1);
        self.bodies.insert(
            handle,
            SimpleBody {
                desc: desc.clone(),
                transform: desc.transform,
                velocity: Vec3::ZERO,
                colliders: Vec::new(),
            },
        );
        Ok(handle)
    }

    fn destroy_body(&mut self, body: BodyHandle) -> EngineResult<()> {
        let body = self
            .bodies
            .remove(&body)
            .ok_or_else(|| EngineError::invalid_handle("physics body does not exist"))?;
        for collider in body.colliders {
            self.colliders.remove(&collider);
        }
        self.active_pairs.retain(|(left, right)| {
            self.colliders.contains_key(left) && self.colliders.contains_key(right)
        });
        Ok(())
    }

    fn add_collider(
        &mut self,
        body: BodyHandle,
        desc: &ColliderDesc,
    ) -> EngineResult<ColliderHandle> {
        self.body(body)?;
        let handle = ColliderHandle(self.next_collider);
        self.next_collider = self.next_collider.saturating_add(1).max(1);
        self.colliders.insert(
            handle,
            SimpleCollider {
                body,
                desc: desc.clone(),
            },
        );
        self.body_mut(body)?.colliders.push(handle);
        Ok(handle)
    }

    fn remove_collider(&mut self, collider: ColliderHandle) -> EngineResult<()> {
        let removed = self
            .colliders
            .remove(&collider)
            .ok_or_else(|| EngineError::invalid_handle("physics collider does not exist"))?;
        if let Some(body) = self.bodies.get_mut(&removed.body) {
            body.colliders.retain(|candidate| *candidate != collider);
        }
        self.active_pairs
            .retain(|(left, right)| *left != collider && *right != collider);
        Ok(())
    }

    fn body_transform(&self, body: BodyHandle) -> EngineResult<Transform> {
        Ok(self.body(body)?.transform)
    }

    fn set_body_transform(&mut self, body: BodyHandle, transform: Transform) -> EngineResult<()> {
        self.body_mut(body)?.transform = transform;
        Ok(())
    }

    fn apply_impulse(&mut self, body: BodyHandle, impulse: Vec3) -> EngineResult<()> {
        let body = self.body_mut(body)?;
        if body.desc.kind == BodyKind::Dynamic {
            body.velocity += impulse;
        }
        Ok(())
    }

    fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        filter: QueryFilter,
    ) -> Option<RayHit> {
        let direction = direction.normalized();
        if direction == Vec3::ZERO {
            return None;
        }
        self.colliders
            .iter()
            .filter(|(_, collider)| filter_matches(collider.desc.layer, filter))
            .filter_map(|(handle, collider)| {
                let (center, radius) = self.collider_world_sphere(*handle)?;
                ray_sphere(origin, direction, max_distance, center, radius).map(|distance| RayHit {
                    body: collider.body,
                    collider: *handle,
                    point: origin + direction * distance,
                    normal: (origin + direction * distance - center).normalized(),
                    distance,
                })
            })
            .min_by(|left, right| left.distance.total_cmp(&right.distance))
    }

    fn overlap_sphere(&self, center: Vec3, radius: f32, filter: QueryFilter) -> Vec<OverlapResult> {
        self.colliders
            .iter()
            .filter(|(_, collider)| filter_matches(collider.desc.layer, filter))
            .filter_map(|(handle, collider)| {
                let (other_center, other_radius) = self.collider_world_sphere(*handle)?;
                ((center - other_center).length_squared() <= (radius + other_radius).powi(2))
                    .then_some(OverlapResult {
                        body: collider.body,
                        collider: *handle,
                    })
            })
            .collect()
    }

    fn sweep_sphere(
        &self,
        center: Vec3,
        radius: f32,
        direction: Vec3,
        max_distance: f32,
        filter: QueryFilter,
    ) -> Option<RayHit> {
        let direction = direction.normalized();
        if direction == Vec3::ZERO {
            return None;
        }
        self.colliders
            .iter()
            .filter(|(_, collider)| filter_matches(collider.desc.layer, filter))
            .filter_map(|(handle, collider)| {
                let (other_center, other_radius) = self.collider_world_sphere(*handle)?;
                ray_sphere(
                    center,
                    direction,
                    max_distance,
                    other_center,
                    radius + other_radius,
                )
                .map(|distance| RayHit {
                    body: collider.body,
                    collider: *handle,
                    point: center + direction * distance,
                    normal: (center + direction * distance - other_center).normalized(),
                    distance,
                })
            })
            .min_by(|left, right| left.distance.total_cmp(&right.distance))
    }

    fn drain_contacts(&mut self) -> Vec<ContactEvent> {
        std::mem::take(&mut self.contacts)
    }
}

fn ordered_pair(left: ColliderHandle, right: ColliderHandle) -> (ColliderHandle, ColliderHandle) {
    if left.0 <= right.0 {
        (left, right)
    } else {
        (right, left)
    }
}

fn layers_match(left: &ColliderDesc, right: &ColliderDesc) -> bool {
    (left.mask & (1 << right.layer.min(31))) != 0 && (right.mask & (1 << left.layer.min(31))) != 0
}

fn filter_matches(layer: u32, filter: QueryFilter) -> bool {
    filter.mask == 0 || (filter.mask & (1 << layer.min(31))) != 0
}

fn shape_world_sphere(center: Vec3, shape: &ColliderShape) -> (Vec3, f32) {
    let radius = match shape {
        ColliderShape::Box { half_extents } => half_extents.length(),
        ColliderShape::Sphere { radius } => *radius,
        ColliderShape::Capsule {
            half_height,
            radius,
        } => half_height + radius,
        ColliderShape::Mesh { vertices } => vertices
            .chunks_exact(3)
            .map(|chunk| Vec3::new(chunk[0], chunk[1], chunk[2]).length())
            .fold(0.0, f32::max),
    };
    (center, radius)
}

fn ray_sphere(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    center: Vec3,
    radius: f32,
) -> Option<f32> {
    let to_center = center - origin;
    let projection = to_center.dot(direction);
    let closest_squared = to_center.length_squared() - projection * projection;
    let radius_squared = radius * radius;
    if closest_squared > radius_squared {
        return None;
    }
    let offset = (radius_squared - closest_squared).sqrt();
    let distance = if projection - offset >= 0.0 {
        projection - offset
    } else {
        projection + offset
    };
    (distance >= 0.0 && distance <= max_distance).then_some(distance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_backend_raycast_returns_none() {
        let world = PhysicsWorld::null();
        let hit = world.backend().raycast(
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 1.0),
            100.0,
            QueryFilter::default(),
        );
        assert!(hit.is_none());
    }

    #[test]
    fn null_backend_overlap_returns_empty() {
        let world = PhysicsWorld::null();
        let results = world
            .backend()
            .overlap_sphere(Vec3::ZERO, 1.0, QueryFilter::default());
        assert!(results.is_empty());
    }

    #[test]
    fn null_backend_contacts_are_empty() {
        let mut world = PhysicsWorld::null();
        assert!(world.backend_mut().drain_contacts().is_empty());
    }

    #[test]
    fn layer_matrix_symmetric_disable() {
        let mut matrix = LayerMatrix::default();
        assert!(matrix.collides(0, 1));
        matrix.set(0, 1, false);
        assert!(!matrix.collides(0, 1));
        assert!(!matrix.collides(1, 0));
    }

    #[test]
    fn collider_desc_defaults_are_sensible() {
        let desc = ColliderDesc::default();
        assert!(!desc.is_trigger);
        assert_eq!(desc.friction, 0.5);
    }

    #[test]
    fn simple_backend_raycast_hits_closest_collider() {
        let mut backend = SimplePhysicsBackend::new();
        let body = backend
            .create_body(&RigidbodyDesc {
                transform: Transform {
                    translation: Vec3::new(0.0, 0.0, 5.0),
                    ..Transform::IDENTITY
                },
                kind: BodyKind::Static,
                ..RigidbodyDesc::default()
            })
            .unwrap();
        backend
            .add_collider(body, &ColliderDesc::default())
            .unwrap();

        let hit = backend
            .raycast(
                Vec3::ZERO,
                Vec3::new(0.0, 0.0, 1.0),
                10.0,
                QueryFilter::default(),
            )
            .unwrap();

        assert_eq!(hit.body, body);
        assert!(hit.distance > 4.0);
    }

    #[test]
    fn simple_backend_emits_enter_and_exit_events() {
        let mut backend = SimplePhysicsBackend::new();
        let first = backend.create_body(&RigidbodyDesc::default()).unwrap();
        let second = backend
            .create_body(&RigidbodyDesc {
                transform: Transform {
                    translation: Vec3::new(0.5, 0.0, 0.0),
                    ..Transform::IDENTITY
                },
                ..RigidbodyDesc::default()
            })
            .unwrap();
        backend
            .add_collider(first, &ColliderDesc::default())
            .unwrap();
        backend
            .add_collider(second, &ColliderDesc::default())
            .unwrap();

        backend.fixed_update(0.0);
        assert!(backend.drain_contacts().iter().any(|event| event.entered));

        backend
            .set_body_transform(
                second,
                Transform {
                    translation: Vec3::new(10.0, 0.0, 0.0),
                    ..Transform::IDENTITY
                },
            )
            .unwrap();
        backend.fixed_update(0.0);
        assert!(backend.drain_contacts().iter().any(|event| !event.entered));
    }

    #[test]
    fn simple_backend_overlap_sphere_filters_by_layer() {
        let mut backend = SimplePhysicsBackend::new();
        let body = backend.create_body(&RigidbodyDesc::default()).unwrap();
        backend
            .add_collider(
                body,
                &ColliderDesc {
                    layer: 3,
                    ..ColliderDesc::default()
                },
            )
            .unwrap();

        assert_eq!(
            backend
                .overlap_sphere(Vec3::ZERO, 2.0, QueryFilter { mask: 1 << 3 },)
                .len(),
            1
        );
        assert!(backend
            .overlap_sphere(Vec3::ZERO, 2.0, QueryFilter { mask: 1 << 2 })
            .is_empty());
    }
}
