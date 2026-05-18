#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Atomic ECS and base scene storage.

pub mod scene;
pub mod transform;
pub mod world;

pub use scene::Scene;
pub use transform::TransformHierarchy;
pub use world::{Entity, World};
