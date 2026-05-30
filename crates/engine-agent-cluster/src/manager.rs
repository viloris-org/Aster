//! Manager agent orchestration trait.
//!
//! The Manager is the orchestrator in Auto mode. It decomposes user requests,
//! routes tasks to Workers, requests capability grants from the Issuer, merges
//! Worker outputs into an integration candidate, coordinates review, manages
//! the repair loop, and generates the final report.
//!
//! **The Manager is untrusted AI output.** Its decisions are reviewable
//! artifacts, not trusted proof. Policy code, capability grants, validators,
//! and the Deep Reviewer independently verify every Manager claim.

use engine_core::EngineResult;

use engine_policy::context::ContextPacket;
use engine_policy::grant::CapabilityGrant;
use engine_policy::ids::TaskId;

use crate::protocol::{
    ProblemReport, QuickFixAction, ReviewDecision, ReviewRequest,
    TaskAssignment, TaskDecomposition, WorkerOutput,
};
use crate::bundle::TransactionBundle;

/// The Manager agent trait.
///
/// Implementations use an AI model to make decisions, but the trait
/// enforces that Manager output is always structured (never free-form prose)
/// and always checked by deterministic policy before taking effect.
pub trait Manager {
    /// Decomposes a user request into independently reviewable tasks.
    ///
    /// The Manager reads project context from the immutable snapshot and
    /// produces a `TaskDecomposition`. This is the first Manager action
    /// in Auto mode.
    ///
    /// # Errors
    /// Returns an error if the request is underspecified, ambiguous in a
    /// way that affects product intent, or cannot be decomposed into
    /// safe bounded tasks. The Manager should ask for clarification rather
    /// than guess at product intent.
    fn decompose(
        &self,
        user_request: &str,
        snapshot_id: engine_policy::ids::SnapshotId,
        workspace_id: engine_policy::ids::WorkspaceId,
        base_revision: &str,
        context: &serde_json::Value,
    ) -> EngineResult<TaskDecomposition>;

    /// Requests capability grants for Workers.
    ///
    /// For each task in the decomposition, the Manager emits a structured
    /// `CapabilityRequest`. The Capability Issuer (deterministic code, not AI)
    /// evaluates each request and returns a `CapabilityDecision`. The Manager
    /// collects the issued grants and builds `TaskAssignment`s.
    ///
    /// The Manager MAY NOT mint grants directly. It can only request them.
    fn request_grants(
        &self,
        tasks: &TaskDecomposition,
        issuer: &dyn engine_policy::grant::CapabilityIssuer,
    ) -> EngineResult<Vec<(TaskAssignment, CapabilityGrant)>>;

    /// Assigns a task to a Worker by building its fresh-session context packet.
    ///
    /// The Manager specifies the Worker role, context packet contents, and
    /// the task assignment. The session orchestrator handles spawning the
    /// actual fresh session.
    fn build_context_packet(
        &self,
        assignment: &TaskAssignment,
        snapshot_data: &serde_json::Value,
        accepted_artifacts: &[crate::protocol::Artifact],
        validator_output: Option<&serde_json::Value>,
    ) -> EngineResult<ContextPacket>;

    /// Merges approved Worker outputs into an integration candidate.
    ///
    /// After local review passes, the Manager merges accepted artifacts
    /// into a single integration candidate. This candidate becomes the
    /// input for deterministic validation and deep review.
    fn merge(
        &self,
        approved_outputs: &[WorkerOutput],
        workspace_root: &str,
    ) -> EngineResult<IntegrationCandidate>;

    /// Invokes the Deep Reviewer on the integration candidate.
    ///
    /// The Manager spawns a fresh session for the Deep Reviewer with only
    /// the review rubric, accepted artifacts, validator output, and
    /// audit report — never raw Worker chat or Manager deliberation.
    fn request_deep_review(
        &self,
        candidate: &IntegrationCandidate,
        review_request: &ReviewRequest,
    ) -> EngineResult<ReviewDecision>;

    /// Creates a repair ticket when validation or review fails.
    ///
    /// Repair tickets are scoped to the specific failure. Repair Workers
    /// receive only the ticket, failing evidence, and integration candidate
    /// in a fresh session.
    fn create_repair_ticket(
        &self,
        candidate: &IntegrationCandidate,
        failed_review: &ReviewDecision,
        retry_count: u32,
    ) -> EngineResult<crate::protocol::RepairTicket>;

    /// Generates the final task report for user review.
    ///
    /// The report includes a summary, all changes, logical change groups,
    /// validation results, review findings, repaired issues, unresolved
    /// problems with quick-fix actions, risk assessment, and traceability
    /// references.
    fn generate_final_report(
        &self,
        candidate: &IntegrationCandidate,
        review: &ReviewDecision,
        problems: &[ProblemReport],
        quick_fixes: &[QuickFixAction],
    ) -> EngineResult<FinalReport>;
}

/// An integration candidate assembled by the Manager from approved Worker outputs.
///
/// This is the single source for deterministic validation and deep review.
/// It is backed by the git task workspace.
#[derive(Clone, Debug)]
pub struct IntegrationCandidate {
    /// Stable candidate identifier.
    pub candidate_id: String,

    /// The task this candidate serves.
    pub task_id: TaskId,

    /// All accepted Worker outputs merged into this candidate.
    pub merged_outputs: Vec<WorkerOutput>,

    /// Logical change groups for user-facing presentation.
    pub change_groups: Vec<ChangeGroup>,

    /// The immutable transaction bundle (populated after review passes).
    pub bundle: Option<TransactionBundle>,

    /// Current state of the integration.
    pub state: IntegrationState,

    /// ISO-8601 creation timestamp.
    pub created_at: String,
}

/// A logical group of related changes for user-facing presentation.
#[derive(Clone, Debug)]
pub struct ChangeGroup {
    /// Group identifier.
    pub group_id: String,

    /// Human-readable description (e.g., "Player controller script").
    pub description: String,

    /// Files modified in this group.
    pub files: Vec<String>,

    /// Entities modified in this group.
    pub entities: Vec<String>,
}

/// Lifecycle state of an integration candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrationState {
    /// Worker outputs are being merged.
    Merging,
    /// Deterministic validation is running.
    Validating,
    /// Deep review is in progress.
    InReview,
    /// Review passed; bundle is ready for user approval.
    ReadyForApproval,
    /// A repair cycle is active.
    Repairing(u32),
    /// Integration failed (blocked).
    Blocked,
    /// User approved; bundle applied.
    Applied,
}

/// The final report presented to the user before approval.
#[derive(Clone, Debug)]
pub struct FinalReport {
    /// Report identifier.
    pub report_id: String,

    /// Summary of what was done and why.
    pub summary: String,

    /// All logical change groups.
    pub change_groups: Vec<ChangeGroup>,

    /// Validator results (deterministic).
    pub validation_results: serde_json::Value,

    /// Review findings (Deep Reviewer output).
    pub review_findings: Vec<crate::protocol::ReviewFinding>,

    /// Issues that were repaired during the repair loop.
    pub repaired_issues: Vec<ProblemReport>,

    /// Unresolved problems with quick-fix actions.
    pub unresolved_problems: Vec<ProblemReport>,

    /// Available quick-fix actions for unresolved problems.
    pub quick_fix_actions: Vec<QuickFixAction>,

    /// Risk assessment (deterministic classification + reviewer residual risk).
    pub risk_assessment: RiskAssessment,

    /// The transaction bundle (if the report is ready for user approval).
    pub bundle: Option<TransactionBundle>,

    /// ISO-8601 generation timestamp.
    pub generated_at: String,
}

/// Risk assessment in the final report.
#[derive(Clone, Debug)]
pub struct RiskAssessment {
    /// Deterministic risk level from the RiskClassifier.
    pub deterministic_risk: engine_policy::risk::RiskClass,

    /// Residual risk from the Deep Reviewer.
    pub reviewer_residual_risk: crate::protocol::ReviewRisk,

    /// Whether any high-risk operations are included.
    pub has_high_risk_operations: bool,

    /// Whether step-up confirmation is required.
    pub requires_step_up_confirmation: bool,

    /// Human-readable risk summary.
    pub summary: String,
}
