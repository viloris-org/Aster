//! Capability grant definition, signing, and enforcement contracts.
//!
//! A `CapabilityGrant` binds a Worker to a specific task, workspace, tool set,
//! risk class, and expiration. Grants are issued by the `CapabilityIssuer`
//! (deterministic Rust code) and enforced by the tool layer on every Worker
//! tool call.
//!
//! ## Grant Flow
//!
//! ```text
//! Manager requests grant → CapabilityIssuer evaluates
//!   ↓                              ↓
//!   grant_request              Deterministic checks:
//!   (task scope,                • Schema validity
//!    worker kind,                • Task binding
//!    needed tools)               • Path scope vs sandbox
//!                                • Command identity vs registry
//!                                • Risk classification
//!                                • Expiration + evidence contract
//!                                ↓
//!                           CapabilityDecision:
//!                           • Approved → signed CapabilityGrant
//!                           • Narrowed → grant with reduced scope
//!                           • Denied → rejection with reason
//!                           • Escalated → requires user/org approval
//! ```
//!
//! ## Signing
//!
//! Grants are HMAC-SHA256 signed with the issuer's secret. The tool layer
//! verifies the signature on every Worker tool call. No Worker, Manager,
//! Reviewer, or user can forge a valid grant signature.

use serde::{Deserialize, Serialize};

use crate::ids::{GrantHash, SnapshotId, TaskId, WorkspaceId};
use crate::risk::RiskClass;

// ── CapabilityGrant ───────────────────────────────────────────────────────────

/// A signed capability grant authorizing a Worker to execute specific tools
/// within a bounded task, workspace, time window, and risk class.
///
/// The grant is the SOLE source of truth for what a Worker may do. The tool
/// layer checks the grant hash on every call. No other component (Manager,
/// Worker prompt, Reviewer report, user approval) can grant permissions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityGrant {
    /// The task this grant authorizes.
    pub task_id: TaskId,

    /// The Worker this grant is issued to.
    pub worker_id: String,

    /// The immutable snapshot this grant is bound to.
    pub snapshot_id: SnapshotId,

    /// The isolated workspace for all writes.
    pub workspace_id: WorkspaceId,

    /// Absolute path to the workspace root on disk.
    pub workspace_root: String,

    /// Base git revision at snapshot time.
    pub base_revision: String,

    /// Content-addressed hash of this grant (HMAC-SHA256).
    pub grant_hash: GrantHash,

    /// Allowed operations the Worker may perform.
    pub allowed: GrantScope,

    /// Explicitly forbidden operations (overrides `allowed`).
    pub forbidden: Vec<String>,

    /// Risk classification (deterministic, not from AI).
    pub risk_class: RiskClass,

    /// Required review route before Worker output is accepted.
    pub review_route: ReviewRoute,

    /// Escalation route when the Worker needs broader access.
    pub escalation_route: EscalationRoute,

    /// Expiration, step limits, and revocation rules.
    pub limits: GrantLimits,

    /// Required evidence the Worker must produce.
    pub evidence_contract: EvidenceContract,

    /// ISO-8601 issuance timestamp.
    pub issued_at: String,

    /// HMAC-SHA256 signature over the canonical JSON of this grant.
    /// Computed by the CapabilityIssuer. Verified by the tool layer.
    #[serde(skip)]
    pub signature: Option<Vec<u8>>,
}

/// The scope of what a grant allows.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrantScope {
    /// Allowed AI tools (e.g., "create_object", "write_script").
    pub tools: Vec<String>,

    /// Allowed editor commands by ID.
    pub commands: Vec<String>,

    /// Allowed read paths (relative to project root or canonical).
    pub read_paths: Vec<String>,

    /// Allowed write paths (inside the task workspace).
    pub write_paths: Vec<String>,

    /// Allowed entity IDs or ID patterns (e.g., "1:*").
    pub entities: Vec<String>,

    /// Allowed scene file paths.
    pub scenes: Vec<String>,

    /// Allowed asset paths or GUIDs.
    pub assets: Vec<String>,

    /// Allowed operation types (map to AgentOperation variants).
    pub operation_types: Vec<String>,

    /// Whether process execution is allowed.
    pub process_execution: bool,

    /// Whether outbound network access is allowed.
    pub network: bool,

    /// Whether this is a narrow or broad grant.
    pub breadth: GrantBreadth,
}

/// Whether a grant is narrow (tight scope) or broad (enterprise tasks).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantBreadth {
    /// Tight scope: minimal paths, specific entities, limited tools.
    Narrow,
    /// Broad scope: may include `assets/**` reads, multiple entities,
    /// importer execution, or other enterprise-necessary access.
    /// Requires stronger evidence, audit, and review.
    Broad,
}

/// Required review routing for Worker output.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewRoute {
    /// Whether local review is required before integration.
    pub local_review_required: bool,

    /// Whether deep review of the integrated result is required.
    pub deep_review_required: bool,

    /// Whether a Risk Auditor pass is required for scripts/commands.
    pub risk_audit_required: bool,

    /// Whether user/org approval is required before apply.
    pub user_approval_required: bool,
}

/// Escalation route when the Worker needs broader access.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EscalationRoute {
    /// Whether the Worker may request capability escalation.
    pub escalation_allowed: bool,

    /// Who decides escalation requests.
    pub escalated_to: EscalationTarget,
}

/// Target for escalated capability requests.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationTarget {
    /// Manager decides (default for routine scope adjustments).
    Manager,
    /// Manager consults a peer reviewer before deciding.
    ManagerWithPeerReview,
    /// Risk Auditor must review before Manager decides.
    RiskAuditorReview,
    /// Organization policy must approve (enterprise).
    OrganizationPolicy,
    /// User must explicitly approve.
    User,
}

/// Limits on grant usage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrantLimits {
    /// Maximum number of tool calls the Worker may make.
    pub max_steps: u32,

    /// ISO-8601 absolute expiry time.
    pub expires_at: Option<String>,

    /// Maximum number of retries for the Worker's task.
    pub max_retries: u32,

    /// Whether this grant has been revoked.
    pub revoked: bool,

    /// Trace parent ID for linking all Worker tool calls.
    pub trace_parent_id: String,
}

/// Required evidence the Worker must produce for its output to be accepted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceContract {
    /// Required artifact types (e.g., "diff", "scene_preview").
    pub required_artifacts: Vec<String>,

    /// Whether a diff of all changes is required.
    pub diff_required: bool,

    /// Whether a scene preview is required.
    pub scene_preview_required: bool,

    /// Whether asset reference validation is required.
    pub asset_reference_check_required: bool,

    /// Whether validator logs must be attached.
    pub validator_log_required: bool,

    /// Whether a rollback plan must be provided.
    pub rollback_plan_required: bool,
}

// ── CapabilityRequest (Manager → Issuer) ──────────────────────────────────────

/// A structured request from the Manager to the Capability Issuer.
///
/// The Manager proposes what a Worker needs; the Issuer validates and decides.
/// The Manager CANNOT mint grants — only the Issuer can.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityRequest {
    /// The task this grant is for.
    pub task_id: TaskId,

    /// The Worker kind making the request.
    pub worker_kind: String,

    /// Unique Worker identifier.
    pub worker_id: String,

    /// The snapshot the Worker will read from.
    pub snapshot_id: SnapshotId,

    /// The workspace the Worker will write to.
    pub workspace_id: WorkspaceId,

    /// Absolute workspace root path.
    pub workspace_root: String,

    /// Base git revision.
    pub base_revision: String,

    /// Requested tools.
    pub requested_tools: Vec<String>,

    /// Requested commands.
    pub requested_commands: Vec<String>,

    /// Requested read paths.
    pub requested_read_paths: Vec<String>,

    /// Requested write paths (inside workspace).
    pub requested_write_paths: Vec<String>,

    /// Requested entities.
    pub requested_entities: Vec<String>,

    /// Requested scenes.
    pub requested_scenes: Vec<String>,

    /// Requested assets.
    pub requested_assets: Vec<String>,

    /// Requested operation types.
    pub requested_operations: Vec<String>,

    /// Whether process execution is needed.
    pub needs_process_execution: bool,

    /// Whether network access is needed.
    pub needs_network: bool,

    /// Why this grant is needed (for audit and review).
    pub justification: String,

    /// Expected artifacts from the Worker.
    pub expected_artifacts: Vec<String>,

    /// Alternative approaches considered (for broad requests).
    pub alternatives_considered: Vec<String>,

    /// Risk tags self-identified by the Manager.
    pub self_identified_risks: Vec<String>,
}

/// Decision from the Capability Issuer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CapabilityDecision {
    /// Grant approved as requested.
    Approved {
        /// The signed capability grant.
        grant: CapabilityGrant,
    },
    /// Grant approved with narrowed scope.
    Narrowed {
        /// The narrowed grant.
        grant: CapabilityGrant,
        /// What was narrowed and why.
        narrowing_reasons: Vec<String>,
    },
    /// Grant denied.
    Denied {
        /// Why the grant was denied.
        reasons: Vec<String>,
    },
    /// Grant requires escalation (user/org approval).
    EscalationRequired {
        /// What needs escalation.
        escalated_items: Vec<String>,
        /// The escalation target.
        escalate_to: EscalationTarget,
    },
}

// ── CapabilityIssuer trait ────────────────────────────────────────────────────

/// The deterministic capability issuer.
///
/// This is the SINGLE component that may issue grants. It is trusted Rust
/// code, never an AI model. Its inputs are all deterministic:
/// - Command capability registry
/// - Sandbox policy
/// - Risk classifier
/// - Task scope (policy-checked)
/// - Snapshot metadata
///
/// The trait is designed so that in a future milestone, the implementation
/// can be moved into a separate Policy Daemon process communicating over
/// a local Unix domain socket.
pub trait CapabilityIssuer: Send + Sync {
    /// Evaluates a capability request and returns a decision.
    ///
    /// The Issuer validates the request against deterministic policy:
    /// 1. Schema validation (is the request well-formed?)
    /// 2. Task binding (are task_id, snapshot_id, workspace_id consistent?)
    /// 3. Command validation (are requested commands in the AI-safe registry?)
    /// 4. Path validation (are requested paths inside canonical roots?)
    /// 5. Risk classification (what risk class does this request map to?)
    /// 6. Evidence requirements (what evidence must the Worker produce?)
    /// 7. Rollback plan (is rollback feasible for the requested operations?)
    fn evaluate(&self, request: &CapabilityRequest) -> CapabilityDecision;

    /// Issues a signed grant for an approved request.
    ///
    /// Computes the grant hash (HMAC-SHA256 over canonical JSON) and signs it.
    /// Returns the fully populated CapabilityGrant with signature.
    fn issue(&self, request: &CapabilityRequest, scope: GrantScope) -> CapabilityGrant;

    /// Verifies a grant signature.
    ///
    /// Returns true if the grant's signature matches its content.
    /// The tool layer calls this before every Worker tool call.
    fn verify(&self, grant: &CapabilityGrant) -> bool;

    /// Signs the canonical JSON of a grant, returning the signature bytes.
    fn sign(&self, grant_json: &str) -> Vec<u8>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{GrantHash, SnapshotId, TaskId, WorkspaceId};

    #[test]
    fn capability_grant_has_required_fields() {
        let grant = CapabilityGrant {
            task_id: TaskId::from_u128(1),
            worker_id: "scene-worker-001".into(),
            snapshot_id: SnapshotId::from_u128(100),
            workspace_id: WorkspaceId::from_u128(42),
            workspace_root: "/tmp/ai-workspace/task-1".into(),
            base_revision: "HEAD".into(),
            grant_hash: GrantHash::new("test-hash"),
            allowed: GrantScope {
                tools: vec!["create_object".into()],
                commands: vec![],
                read_paths: vec!["scenes/".into()],
                write_paths: vec!["ai-workspace/".into()],
                entities: vec![],
                scenes: vec![],
                assets: vec![],
                operation_types: vec!["create_object".into()],
                process_execution: false,
                network: false,
                breadth: GrantBreadth::Narrow,
            },
            forbidden: vec!["destroy_object".into(), "execute_command".into()],
            risk_class: RiskClass::Medium,
            review_route: ReviewRoute {
                local_review_required: true,
                deep_review_required: false,
                risk_audit_required: false,
                user_approval_required: false,
            },
            escalation_route: EscalationRoute {
                escalation_allowed: true,
                escalated_to: EscalationTarget::Manager,
            },
            limits: GrantLimits {
                max_steps: 20,
                expires_at: None,
                max_retries: 3,
                revoked: false,
                trace_parent_id: "trace-task-1".into(),
            },
            evidence_contract: EvidenceContract {
                required_artifacts: vec!["diff".into(), "scene_preview".into()],
                diff_required: true,
                scene_preview_required: true,
                asset_reference_check_required: false,
                validator_log_required: false,
                rollback_plan_required: false,
            },
            issued_at: "2025-01-01T00:00:00Z".into(),
            signature: None,
        };

        assert_eq!(grant.task_id, TaskId::from_u128(1));
        assert_eq!(grant.allowed.tools.len(), 1);
        assert!(!grant.allowed.process_execution);
        assert_eq!(grant.limits.max_steps, 20);
        assert!(!grant.limits.revoked);
    }

    #[test]
    fn narrow_grant_differs_from_broad() {
        assert_ne!(GrantBreadth::Narrow, GrantBreadth::Broad);
    }
}
