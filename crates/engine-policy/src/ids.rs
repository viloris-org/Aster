//! Opaque AI Copilot identifiers.
//!
//! Every identifier is a newtype wrapper around `u128`, following the
//! `engine_core::ids` convention. These provide type-safe identity for
//! the Agent Cluster orchestration layer and grant enforcement.

use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
        )]
        pub struct $name(u128);

        impl $name {
            /// Creates an ID from raw bits.
            pub const fn from_u128(value: u128) -> Self {
                Self(value)
            }

            /// Returns raw ID bits for serialization boundaries.
            pub const fn as_u128(self) -> u128 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

id_type!(
    TaskId,
    "Unique identifier for an AI task (decomposition unit).\n\n\
     Bound to a specific user request, snapshot, and workspace.\n\
     Format: `task-{decimal}`, e.g. `task-1`."
);

id_type!(
    SnapshotId,
    "Immutable project state snapshot identifier.\n\n\
     Created before any AI write work begins. All Worker reads and\n\
     writes are validated against this snapshot hash.\n\
     Format: `snap-{timestamp}-{hash_prefix}`, e.g. `snap-1717200000-a1b2`."
);

id_type!(
    WorkspaceId,
    "Isolated task workspace identifier.\n\n\
     Maps to a git worktree under a stable branch convention\n\
     (`ai/task-0001`). Writes by Workers are confined to this\n\
     workspace; nothing in it reaches the active project without\n\
     passing through a validated transaction bundle.\n\
     Format: `ws-{decimal}`, e.g. `ws-42`."
);

/// Content-addressed hash for a Capability Grant.
///
/// Computed as HMAC-SHA256(grant_bytes, issuer_secret). The tool layer
/// compares the grant hash carried by every Worker tool call against
/// the issuer-signed hash. Mismatch → reject.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GrantHash(String);

impl GrantHash {
    /// Wraps a pre-computed hash string.
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    /// Returns the hash string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GrantHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

/// Content-addressed hash for a Transaction Bundle.
///
/// Computed as SHA-256 over the canonical JSON representation of the
/// bundle's operation list, touched artifacts, rollback journal, and
/// approval metadata. The apply layer verifies this hash before
/// touching the active project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleHash(String);

impl BundleHash {
    /// Wraps a pre-computed hash string.
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    /// Returns the hash string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BundleHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

/// Content-addressed hash for a Context Packet.
///
/// Computed as SHA-256 over the canonical JSON of the packet's
/// sections. Workers and Reviewers receive context packets; if the
/// packet hash doesn't match the expected hash, the session is
/// aborted (stale context or tampering).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContextHash(String);

impl ContextHash {
    /// Wraps a pre-computed hash string.
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    /// Returns the hash string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ContextHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_roundtrip() {
        let id = TaskId::from_u128(42);
        assert_eq!(id.as_u128(), 42);
        assert_eq!(id.to_string(), "42");
    }

    #[test]
    fn snapshot_id_roundtrip() {
        let id = SnapshotId::from_u128(100);
        assert_eq!(id.as_u128(), 100);
    }

    #[test]
    fn workspace_id_roundtrip() {
        let id = WorkspaceId::from_u128(7);
        assert_eq!(id.as_u128(), 7);
    }

    #[test]
    fn grant_hash_equality() {
        let hash1 = GrantHash::new("abc123");
        let hash2 = GrantHash::new("abc123");
        let hash3 = GrantHash::new("def456");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn bundle_hash_equality() {
        let hash1 = BundleHash::new("sha256:deadbeef");
        let hash2 = BundleHash::new("sha256:deadbeef");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn context_hash_equality() {
        let hash1 = ContextHash::new("ctx-a1b2c3");
        let hash2 = ContextHash::new("ctx-d4e5f6");
        assert_ne!(hash1, hash2);
    }
}
