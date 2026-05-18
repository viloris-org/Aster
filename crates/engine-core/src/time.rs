//! Frame and time primitives.

use std::time::Duration;

/// Monotonic frame counter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameCounter(u64);

impl FrameCounter {
    /// Current frame index.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Advances by one frame.
    pub fn advance(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}

/// Time elapsed for a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeStep {
    delta: Duration,
}

impl TimeStep {
    /// Creates a timestep from a duration.
    pub const fn new(delta: Duration) -> Self {
        Self { delta }
    }

    /// Delta duration.
    pub const fn delta(self) -> Duration {
        self.delta
    }

    /// Delta seconds as `f32`.
    pub fn seconds_f32(self) -> f32 {
        self.delta.as_secs_f32()
    }
}
