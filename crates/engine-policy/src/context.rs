//! Role-specific context packets for fresh-session agent isolation.
//!
//! In Auto mode, Workers, Reviewers, Risk Auditors, and Repair Workers
//! execute in **fresh sessions** — each receives only its role-specific
//! `ContextPacket`, never the Manager's full conversation, sibling Worker
//! chat, prior repair reasoning, or raw user prompt text.
//!
//! A `ContextPacket` is an immutable, hash-verified bundle of labeled
//! sections. Every section carries a `TrustLabel`. The packet hash is
//! computed at generation time; if the hash mismatches at consumption,
//! the session is aborted (stale or tampered context).

use serde::{Deserialize, Serialize};

use crate::ids::{ContextHash, SnapshotId, TaskId};
use crate::trust::TrustLabel;

/// A complete role-specific context packet for one fresh session.
///
/// Generated from an immutable snapshot by the session orchestrator
/// at the Manager's request. Contains only the sections relevant to
/// the target role, each trust-labeled.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextPacket {
    /// Stable packet identifier.
    pub packet_id: String,

    /// The task this packet serves.
    pub task_id: TaskId,

    /// The snapshot this context was derived from.
    pub snapshot_id: SnapshotId,

    /// Content hash — computed over the canonical JSON of all sections.
    /// Mismatch → stale context or tampering → session aborted.
    pub context_hash: ContextHash,

    /// The role this packet was generated for.
    pub target_role: AgentRole,

    /// Ordered, labeled sections.
    pub sections: Vec<ContextSection>,

    /// Sources used to build this packet (for traceability).
    pub sources: Vec<ContextSource>,

    /// Expiration rules for this packet.
    pub expiration: ContextExpiration,

    /// ISO-8601 creation timestamp.
    pub created_at: String,
}

/// Agent role receiving a context packet.
///
/// Each role has a strict allowlist of section types it may receive.
/// The session orchestrator enforces this — a `scene_worker` must not
/// receive `Reviewer`-only evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// The Manager agent (orchestration session, not a fresh session).
    Manager,
    /// A specialized Worker executing a bounded task.
    Worker(WorkerKind),
    /// The Deep Reviewer inspecting an integration candidate.
    DeepReviewer,
    /// The Risk Auditor reviewing high-risk scripts/commands.
    RiskAuditor,
    /// A Repair Worker patching a specific failure.
    RepairWorker,
}

/// Worker specialization (maps to trusted tool boundaries).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerKind {
    /// Scene manipulation (create/destroy entities, modify components).
    Scene,
    /// Script creation and modification under the asset root.
    Script,
    /// Asset import, reference validation, and metadata changes.
    Asset,
    /// Diagnostics analysis and repair suggestions.
    Diagnostics,
    /// Read-only explanation and project inspection.
    Explain,
    /// Scoped repair against a specific repair ticket.
    Repair,
    /// Auditing high-risk scripts (Risk Auditor Worker).
    Audit,
}

/// A single labeled section within a context packet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextSection {
    /// Section identifier within the packet (e.g. "task-brief", "scene-snapshot").
    pub section_id: String,

    /// Trust classification for this section's content.
    pub label: TrustLabel,

    /// The section's content as structured JSON.
    ///
    /// The shape depends on `section_id`:
    /// - `"task-brief"` → `TaskBrief` JSON
    /// - `"scene-snapshot"` → scene hierarchy JSON (subset relevant to task)
    /// - `"asset-index"` → filtered asset list JSON
    /// - `"validator-output"` → deterministic validator report
    /// - `"audit-report"` → Deterministic Static Auditor output
    /// - `"accepted-artifacts"` → diffs and approved Worker outputs
    /// - `"repair-ticket"` → scoped repair ticket (Repair Worker only)
    pub content: serde_json::Value,
}

/// A source referenced when building a context packet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextSource {
    /// Source identifier (e.g. snapshot path, Worker output ID).
    pub source_id: String,

    /// Trust label for the source material.
    pub label: TrustLabel,

    /// Human-readable description of what was extracted.
    pub description: String,
}

/// Expiration rules for a context packet.
///
/// A packet becomes invalid when any of its expiration conditions are met.
/// The session orchestrator checks expiration before every fresh-session
/// invocation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextExpiration {
    /// Maximum number of times this packet may be used for a fresh session.
    /// `None` means unlimited (within the lifetime of the parent task).
    pub max_uses: Option<u32>,

    /// ISO-8601 absolute expiry time, after which the packet is stale.
    /// `None` means the packet lives for the duration of the task.
    pub expires_at: Option<String>,

    /// If true, the packet is invalidated when the parent snapshot changes.
    pub invalidate_on_snapshot_change: bool,

    /// If true, the packet is invalidated when accepted artifacts change.
    pub invalidate_on_artifact_change: bool,
}

impl Default for ContextExpiration {
    fn default() -> Self {
        Self {
            max_uses: Some(1), // fresh sessions are single-use by default
            expires_at: None,
            invalidate_on_snapshot_change: true,
            invalidate_on_artifact_change: true,
        }
    }
}

/// A normalized task brief included in a Worker's context packet.
///
/// This is the POLICY-CHECKED task scope, not the Manager's raw prose.
/// The Capability Issuer validates the brief before it is sealed into a
/// context packet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskBrief {
    /// The task this brief describes.
    pub task_id: TaskId,

    /// One-sentence objective.
    pub objective: String,

    /// Explicitly forbidden changes and operations.
    pub non_goals: Vec<String>,

    /// Allowed files (paths relative to project root or workspace root).
    pub allowed_files: Vec<String>,

    /// Allowed entity IDs (or entity ID patterns).
    pub allowed_entities: Vec<String>,

    /// Allowed scenes (by path).
    pub allowed_scenes: Vec<String>,

    /// Allowed asset paths or GUIDs.
    pub allowed_assets: Vec<String>,

    /// Allowed operation types.
    pub allowed_operations: Vec<String>,

    /// Forbidden operation types (even if implicitly allowed by the above).
    pub forbidden_operations: Vec<String>,

    /// Observable acceptance criteria (not subjective claims).
    pub acceptance_criteria: Vec<String>,

    /// Expected output artifacts with types.
    pub expected_artifacts: Vec<ExpectedArtifact>,

    /// Review rubric with correctness, scope, safety, rollback checks.
    pub review_rubric: ReviewRubric,

    /// Required evidence types that must be produced.
    pub required_evidence: Vec<EvidenceType>,

    /// Repair policy: max retries and escalation conditions.
    pub repair_policy: RepairPolicy,
}

/// An expected output artifact from a Worker task.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExpectedArtifact {
    /// Artifact type: "scene_change", "script_file", "asset_import", "diagnostic_report".
    pub artifact_type: String,

    /// Human-readable description.
    pub description: String,

    /// File path or entity/asset reference.
    pub target: String,
}

/// Objective review criteria for a task.
///
/// Reviewers evaluate artifacts against this rubric, not against subjective
/// judgments or Worker claims.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewRubric {
    /// Correctness checks (e.g., "scene loads without errors", "script compiles").
    pub correctness: Vec<String>,

    /// Scope checks (e.g., "no files modified outside allowed_files").
    pub scope: Vec<String>,

    /// Safety checks (e.g., "no process execution", "no network access").
    pub safety: Vec<String>,

    /// Rollback checks (e.g., "undo journal covers all file operations").
    pub rollback: Vec<String>,

    /// User-impact checks (e.g., "no breaking changes to existing entities").
    pub user_impact: Vec<String>,
}

/// Types of evidence a Worker must produce.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    /// A diff (unified or structured) of all changes.
    Diff,
    /// A scene preview (JSON representation of changed entities).
    ScenePreview,
    /// Asset reference checks (all referenced GUIDs exist).
    AssetReferenceCheck,
    /// Validator logs (output of deterministic validators).
    ValidatorLog,
    /// Audit output (Deterministic Static Auditor report).
    AuditOutput,
    /// Diagnostic comparison (before/after diagnostic counts).
    DiagnosticComparison,
}

/// Repair policy for a task.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepairPolicy {
    /// Maximum number of automatic repair cycles (default 3).
    pub max_retries: u32,

    /// Conditions that escalate to the user instead of retrying.
    pub escalate_on: Vec<String>,
}

impl Default for RepairPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            escalate_on: vec![
                "product_intent_ambiguous".to_string(),
                "out_of_scope_need".to_string(),
                "irreversible_change".to_string(),
                "credential_required".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_packet_serialization_roundtrip() {
        let packet = ContextPacket {
            packet_id: "ctx-001".into(),
            task_id: TaskId::from_u128(1),
            snapshot_id: SnapshotId::from_u128(100),
            context_hash: ContextHash::new("abc123"),
            target_role: AgentRole::Worker(WorkerKind::Scene),
            sections: vec![ContextSection {
                section_id: "task-brief".into(),
                label: TrustLabel::TrustedTaskScope,
                content: serde_json::json!({"objective": "create a player object"}),
            }],
            sources: vec![ContextSource {
                source_id: "snap-100".into(),
                label: TrustLabel::TrustedPolicy,
                description: "Immutable project snapshot".into(),
            }],
            expiration: ContextExpiration::default(),
            created_at: "2025-01-01T00:00:00Z".into(),
        };

        let json = serde_json::to_string(&packet).unwrap();
        let decoded: ContextPacket = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.packet_id, "ctx-001");
        assert_eq!(decoded.sections.len(), 1);
        assert_eq!(decoded.sections[0].label, TrustLabel::TrustedTaskScope);
    }

    #[test]
    fn default_expiration_is_single_use() {
        let exp = ContextExpiration::default();
        assert_eq!(exp.max_uses, Some(1));
        assert!(exp.invalidate_on_snapshot_change);
    }

    #[test]
    fn repair_policy_defaults_to_three_retries() {
        let policy = RepairPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert!(!policy.escalate_on.is_empty());
    }
}
