//! Base scene storage.

use crate::{TransformHierarchy, World};

/// Minimal scene made from an ECS world plus transform hierarchy.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    /// Entity storage.
    pub world: World,
    /// Transform hierarchy storage.
    pub transforms: TransformHierarchy,
}
