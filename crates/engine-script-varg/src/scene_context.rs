use std::collections::HashMap;
use std::sync::Arc;

use engine_core::math::Vec3;

/// Read-only scene facts available to one script invocation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VargSceneContext {
    /// User-visible name of the entity this script is attached to.
    pub entity_name: String,
    /// User-visible tag of the entity this script is attached to.
    pub entity_tag: String,
    /// Local positions keyed by user-visible object name.
    pub positions_by_name: HashMap<String, Vec3>,
    /// Local positions grouped by tag.
    pub positions_by_tag: HashMap<String, Vec<Vec3>>,
    /// Shared local positions keyed by user-visible object name.
    pub shared_positions_by_name: Option<Arc<HashMap<String, Vec3>>>,
    /// Shared local positions grouped by tag.
    pub shared_positions_by_tag: Option<Arc<HashMap<String, Vec<Vec3>>>>,
    /// World-space bounds keyed by user-visible object name.
    pub bounds_by_name: HashMap<String, VargSceneBounds>,
    /// World-space bounds grouped by tag.
    pub bounds_by_tag: HashMap<String, Vec<VargSceneBounds>>,
    /// Shared world-space bounds keyed by user-visible object name.
    pub shared_bounds_by_name: Option<Arc<HashMap<String, VargSceneBounds>>>,
    /// Shared world-space bounds grouped by tag.
    pub shared_bounds_by_tag: Option<Arc<HashMap<String, Vec<VargSceneBounds>>>>,
}

/// Axis-aligned world-space bounds available to gameplay scripts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VargSceneBounds {
    /// Minimum corner.
    pub min: Vec3,
    /// Maximum corner.
    pub max: Vec3,
}

impl VargSceneContext {
    /// Creates a context backed by shared frame-level scene position snapshots.
    pub fn from_shared_positions(
        entity_name: impl Into<String>,
        entity_tag: impl Into<String>,
        positions_by_name: Arc<HashMap<String, Vec3>>,
        positions_by_tag: Arc<HashMap<String, Vec<Vec3>>>,
    ) -> Self {
        Self::from_shared_scene(
            entity_name,
            entity_tag,
            positions_by_name,
            positions_by_tag,
            Arc::new(HashMap::new()),
            Arc::new(HashMap::new()),
        )
    }

    /// Creates a context backed by shared frame-level scene snapshots.
    pub fn from_shared_scene(
        entity_name: impl Into<String>,
        entity_tag: impl Into<String>,
        positions_by_name: Arc<HashMap<String, Vec3>>,
        positions_by_tag: Arc<HashMap<String, Vec<Vec3>>>,
        bounds_by_name: Arc<HashMap<String, VargSceneBounds>>,
        bounds_by_tag: Arc<HashMap<String, Vec<VargSceneBounds>>>,
    ) -> Self {
        Self {
            entity_name: entity_name.into(),
            entity_tag: entity_tag.into(),
            positions_by_name: HashMap::new(),
            positions_by_tag: HashMap::new(),
            shared_positions_by_name: Some(positions_by_name),
            shared_positions_by_tag: Some(positions_by_tag),
            bounds_by_name: HashMap::new(),
            bounds_by_tag: HashMap::new(),
            shared_bounds_by_name: Some(bounds_by_name),
            shared_bounds_by_tag: Some(bounds_by_tag),
        }
    }

    /// Returns true when the owning entity has the requested tag.
    pub fn entity_has_tag(&self, tag: &str) -> bool {
        self.entity_tag == tag
    }

    /// Returns the distance from the owning entity position to the first object
    /// with the given name.
    pub fn distance_to_name(&self, origin: Vec3, name: &str) -> Option<f32> {
        self.positions_by_name()
            .get(name)
            .map(|target| (*target - origin).length())
    }

    /// Returns the nearest distance from the owning entity position to objects
    /// with the given tag.
    pub fn distance_to_tag(&self, origin: Vec3, tag: &str) -> Option<f32> {
        self.positions_by_tag()
            .get(tag)?
            .iter()
            .map(|target| (*target - origin).length())
            .reduce(f32::min)
    }

    /// Returns the nearest distance from the owning entity position to object
    /// bounds with the given tag. The distance is zero while inside bounds.
    pub fn distance_to_tag_bounds(&self, origin: Vec3, tag: &str) -> Option<f32> {
        self.bounds_by_tag()
            .get(tag)?
            .iter()
            .map(|bounds| bounds.distance_to_point(origin))
            .reduce(f32::min)
    }

    /// Returns the nearest horizontal distance from the owning entity position
    /// to object bounds with the given tag. Y is ignored, so the distance is
    /// zero when the point is above or below the X/Z footprint.
    pub fn horizontal_distance_to_tag_bounds(&self, origin: Vec3, tag: &str) -> Option<f32> {
        self.bounds_by_tag()
            .get(tag)?
            .iter()
            .map(|bounds| bounds.horizontal_distance_to_point(origin))
            .reduce(f32::min)
    }

    /// Returns a named object's local X position.
    pub fn x_of_name(&self, name: &str) -> Option<f32> {
        self.positions_by_name()
            .get(name)
            .map(|position| position.x)
    }

    /// Returns a named object's local Y position.
    pub fn y_of_name(&self, name: &str) -> Option<f32> {
        self.positions_by_name()
            .get(name)
            .map(|position| position.y)
    }

    /// Returns a named object's local Z position.
    pub fn z_of_name(&self, name: &str) -> Option<f32> {
        self.positions_by_name()
            .get(name)
            .map(|position| position.z)
    }

    fn positions_by_name(&self) -> &HashMap<String, Vec3> {
        self.shared_positions_by_name
            .as_deref()
            .unwrap_or(&self.positions_by_name)
    }

    fn positions_by_tag(&self) -> &HashMap<String, Vec<Vec3>> {
        self.shared_positions_by_tag
            .as_deref()
            .unwrap_or(&self.positions_by_tag)
    }

    fn bounds_by_tag(&self) -> &HashMap<String, Vec<VargSceneBounds>> {
        self.shared_bounds_by_tag
            .as_deref()
            .unwrap_or(&self.bounds_by_tag)
    }
}

impl VargSceneBounds {
    /// Creates axis-aligned bounds from a center and full size.
    pub fn from_center_size(center: Vec3, size: Vec3) -> Self {
        let half = Vec3::new(size.x.abs(), size.y.abs(), size.z.abs()) * 0.5;
        Self {
            min: center - half,
            max: center + half,
        }
    }

    /// Returns the shortest 3D distance from these bounds to a point.
    pub fn distance_to_point(self, point: Vec3) -> f32 {
        let dx = if point.x < self.min.x {
            self.min.x - point.x
        } else if point.x > self.max.x {
            point.x - self.max.x
        } else {
            0.0
        };
        let dy = if point.y < self.min.y {
            self.min.y - point.y
        } else if point.y > self.max.y {
            point.y - self.max.y
        } else {
            0.0
        };
        let dz = if point.z < self.min.z {
            self.min.z - point.z
        } else if point.z > self.max.z {
            point.z - self.max.z
        } else {
            0.0
        };
        Vec3::new(dx, dy, dz).length()
    }

    /// Returns the shortest X/Z distance from these bounds to a point.
    pub fn horizontal_distance_to_point(self, point: Vec3) -> f32 {
        let dx = if point.x < self.min.x {
            self.min.x - point.x
        } else if point.x > self.max.x {
            point.x - self.max.x
        } else {
            0.0
        };
        let dz = if point.z < self.min.z {
            self.min.z - point.z
        } else if point.z > self.max.z {
            point.z - self.max.z
        } else {
            0.0
        };
        Vec3::new(dx, 0.0, dz).length()
    }
}
