//! Structured error types for runtime boundaries.

use std::path::PathBuf;

/// Common engine result alias.
pub type EngineResult<T> = Result<T, EngineError>;

/// Stable error categories that can be mapped at CLI, editor, log, and script boundaries.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Configuration was invalid or incomplete.
    #[error("configuration error: {message}")]
    Config {
        /// Human-readable context.
        message: String,
    },

    /// A filesystem operation failed at a known path.
    #[error("filesystem error at {path:?}: {source}")]
    Filesystem {
        /// Path involved in the failing operation.
        path: PathBuf,
        /// Source IO error.
        #[source]
        source: std::io::Error,
    },

    /// A stable handle was stale or malformed.
    #[error("invalid handle: {message}")]
    InvalidHandle {
        /// Human-readable context.
        message: String,
    },

    /// A platform capability is unavailable in the current environment.
    #[error("unsupported platform capability: {capability}")]
    UnsupportedCapability {
        /// Capability name.
        capability: &'static str,
    },

    /// A callback was invoked on a thread that is not legal for that callback.
    #[error("thread violation: expected {expected}, got {actual}")]
    ThreadViolation {
        /// Expected execution thread.
        expected: &'static str,
        /// Actual execution thread.
        actual: &'static str,
    },

    /// Any other bounded runtime error.
    #[error("{message}")]
    Other {
        /// Human-readable context.
        message: String,
    },
}

impl EngineError {
    /// Creates a configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Creates an invalid handle error.
    pub fn invalid_handle(message: impl Into<String>) -> Self {
        Self::InvalidHandle {
            message: message.into(),
        }
    }

    /// Creates a generic bounded runtime error.
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }
}
