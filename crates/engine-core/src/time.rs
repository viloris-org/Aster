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

/// Aggregated time state for the game loop, tracking delta time, fixed timestep,
/// total elapsed time, frame counting, and time scale.
#[derive(Clone, Debug)]
pub struct TimeState {
    /// Wall-clock delta seconds for the current frame (before time scale).
    pub delta_seconds: f32,
    /// Target duration for each fixed timestep tick (default 1.0 / 60.0).
    pub fixed_delta_seconds: f32,
    /// Total elapsed time since the game loop started (time-scaled).
    pub total_time: f32,
    /// Monotonic frame index.
    pub frame_index: u64,
    /// Multiplier applied to delta time each frame (default 1.0).
    pub time_scale: f32,
    /// Maximum allowed delta seconds per frame (default 0.1). Prevents spiral of death.
    pub max_dt: f32,
    /// Fixed timestep accumulator. Incremented by (capped) delta each frame,
    /// consumed by fixed_update steps.
    pub accumulator: f32,
    /// Maximum number of fixed steps per frame (default 8). Prevents spiral of death.
    pub max_fixed_steps_per_frame: u32,
}

impl Default for TimeState {
    fn default() -> Self {
        Self {
            delta_seconds: 0.0,
            fixed_delta_seconds: 1.0 / 60.0,
            total_time: 0.0,
            frame_index: 0,
            time_scale: 1.0,
            max_dt: 0.1,
            accumulator: 0.0,
            max_fixed_steps_per_frame: 8,
        }
    }
}

impl TimeState {
    /// Creates a new `TimeState` with sensible defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advances the time state by `dt` seconds (wall-clock). The delta is capped
    /// by `max_dt`, the scaled delta is accumulated into `total_time`, and
    /// `frame_index` is incremented. The capped delta is also added to the
    /// fixed timestep `accumulator`.
    pub fn update(&mut self, dt: f32) {
        let capped = dt.min(self.max_dt);
        self.delta_seconds = capped;
        let scaled = capped * self.time_scale;
        self.total_time += scaled;
        self.accumulator += capped;
        self.frame_index = self.frame_index.saturating_add(1);
    }

    /// Returns whether another fixed step should run, consuming one step if so.
    /// Returns `true` when `accumulator >= fixed_delta_seconds` and
    /// `steps_this_frame < max_fixed_steps_per_frame`.
    pub fn consume_fixed_step(&mut self, steps_this_frame: u32) -> bool {
        if steps_this_frame >= self.max_fixed_steps_per_frame
            || self.accumulator < self.fixed_delta_seconds
        {
            return false;
        }
        self.accumulator -= self.fixed_delta_seconds;
        true
    }

    /// Returns the interpolation fraction for the current fixed timestep.
    /// Useful for smooth rendering between fixed updates.
    /// Value is in [0, 1) where 0 = just stepped, ~1 = about to step.
    pub fn interpolation_fraction(&self) -> f32 {
        if self.fixed_delta_seconds > 0.0 {
            (self.accumulator / self.fixed_delta_seconds).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_state_defaults() {
        let ts = TimeState::new();
        assert_eq!(ts.delta_seconds, 0.0);
        assert!((ts.fixed_delta_seconds - 1.0 / 60.0).abs() < f32::EPSILON);
        assert_eq!(ts.total_time, 0.0);
        assert_eq!(ts.frame_index, 0);
        assert_eq!(ts.time_scale, 1.0);
        assert_eq!(ts.max_dt, 0.1);
        assert_eq!(ts.accumulator, 0.0);
        assert_eq!(ts.max_fixed_steps_per_frame, 8);
    }

    #[test]
    fn time_state_update_accumulates_time() {
        let mut ts = TimeState::new();
        ts.update(0.016);
        assert_eq!(ts.delta_seconds, 0.016);
        assert!((ts.total_time - 0.016).abs() < 1e-6);
        assert_eq!(ts.frame_index, 1);
        assert!((ts.accumulator - 0.016).abs() < 1e-6);

        ts.update(0.032);
        assert_eq!(ts.delta_seconds, 0.032);
        assert!((ts.total_time - 0.048).abs() < 1e-5);
        assert_eq!(ts.frame_index, 2);
    }

    #[test]
    fn time_state_respects_time_scale() {
        let mut ts = TimeState::new();
        ts.time_scale = 0.5;
        ts.update(0.05);
        assert!((ts.total_time - 0.025).abs() < 1e-6);

        ts.time_scale = 2.0;
        ts.update(0.05);
        assert!((ts.total_time - 0.125).abs() < 1e-5);
    }

    #[test]
    fn time_state_frame_index_saturates() {
        let mut ts = TimeState::new();
        ts.frame_index = u64::MAX;
        ts.update(0.0);
        assert_eq!(ts.frame_index, u64::MAX);
    }

    #[test]
    fn time_state_max_dt_caps_delta() {
        let mut ts = TimeState::new();
        ts.max_dt = 0.1;
        ts.update(0.5);
        assert!(
            (ts.delta_seconds - 0.1).abs() < f32::EPSILON,
            "delta should be capped to max_dt"
        );
        assert!(
            (ts.accumulator - 0.1).abs() < f32::EPSILON,
            "accumulator should use capped delta"
        );
    }

    #[test]
    fn fixed_step_accumulator_three_steps() {
        let mut ts = TimeState::new();
        let fixed_dt = ts.fixed_delta_seconds; // 1/60
        // Use delta = 3 * fixed_dt + small remainder to guarantee exactly 3 steps
        let delta = fixed_dt * 3.0 + 0.001;
        ts.update(delta);
        let mut steps = 0;
        while ts.consume_fixed_step(steps) {
            steps += 1;
        }
        assert_eq!(
            steps, 3,
            "delta=3*fixed_dt+0.001 should trigger 3 fixed updates"
        );
        assert!(
            ts.accumulator < fixed_dt,
            "accumulator should be less than fixed_dt after consuming all steps, got {}",
            ts.accumulator
        );
    }

    #[test]
    fn fixed_step_max_dt_prevents_spiral_of_death() {
        let mut ts = TimeState::new();
        ts.max_dt = 0.1;
        // delta=0.5 is capped to 0.1 by max_dt
        // 0.1 / (1/60) ≈ 6.0, so at most 6 fixed steps
        ts.update(0.5);
        assert!(
            (ts.delta_seconds - 0.1).abs() < f32::EPSILON,
            "delta should be capped to max_dt=0.1"
        );
        let mut steps = 0;
        while ts.consume_fixed_step(steps) {
            steps += 1;
        }
        assert!(
            steps <= 6,
            "capped delta=0.1 should trigger at most 6 fixed updates, got {}",
            steps
        );
        assert!(
            steps >= 5,
            "capped delta=0.1 should trigger at least 5 fixed updates, got {}",
            steps
        );
    }

    #[test]
    fn interpolation_fraction_is_valid() {
        let mut ts = TimeState::new();
        ts.update(0.02);
        // After one step consumed: accumulator = 0.02 - 1/60 ≈ 0.00333
        let _ = ts.consume_fixed_step(0);
        let frac = ts.interpolation_fraction();
        assert!(
            frac >= 0.0 && frac < 1.0,
            "interpolation fraction should be in [0, 1), got {}",
            frac
        );
    }
}
