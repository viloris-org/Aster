//! Logging facade helpers.

/// Logs a structured runtime startup event.
pub fn log_runtime_start(app_name: &str, profile: &str) {
    tracing::info!(app_name, profile, "runtime starting");
}

/// Logs a structured frame event.
pub fn log_frame(frame_index: u64) {
    tracing::trace!(frame_index, "frame tick");
}
