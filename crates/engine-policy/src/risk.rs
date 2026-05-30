//! Risk classification for operations, commands, and scripts.
//!
//! Maps operation types, file patterns, command IDs, credential access,
//! network access, and rollback quality to four deterministic risk levels.

/// Risk classification for AI operations.
///
/// Determined by the deterministic RiskClassifier based on operation type,
/// file patterns, command IDs, credential access, network access, process
/// execution, dependency changes, and rollback quality.
///
/// Full implementation in step 4.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum RiskClass {
    /// Routine operations: read files, explain scene, format code.
    Low,
    /// Write operations with clear rollback: create entity, modify field.
    Medium,
    /// Destructive or wide-scope: delete asset tree, dependency changes.
    High,
    /// Credential, network, process execution, or irreversible changes.
    Critical,
}

