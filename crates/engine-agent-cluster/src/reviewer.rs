//! Reviewer trait and review logic.
//!
//! Reviewers evaluate objective artifacts (diffs, validator output, scene
//! previews) against the task brief and acceptance criteria. They do NOT
//! trust Worker self-reports, rationales, or claimed test results.
//!
//! ## Reviewer Authority
//!
//! - Reviewers may return `approved`, `needs_revision`, or `blocked`.
//! - They may raise risk level or recommend blocking.
//! - They MUST NOT grant permissions, lower deterministic risk
//!   classifications, approve blocked operations, or override validators.
//! - Reviewer output is advisory and monotonic.
//!
//! ## Local vs Deep Review
//!
//! - **Local Review**: evaluates a single Worker's output before integration.
//!   Performed by the Manager (or a dedicated local reviewer).
//! - **Deep Review**: evaluates the INTEGRATED candidate after all Worker
//!   outputs have been merged and deterministic validation has passed.
//!   Performed by a dedicated Deep Reviewer in a fresh session.

use engine_core::EngineResult;

use crate::protocol::{
    Artifact, LocalReviewDecision, ReviewDecision, ReviewFinding, ReviewRequest,
    WorkerOutput,
};

/// A reviewer that evaluates objective artifacts.
pub trait Reviewer {
    /// Reviews a single Worker's output before integration (local review).
    ///
    /// The reviewer evaluates objective artifacts against the task brief,
    /// acceptance criteria, and review rubric. Worker self-reports are
    /// treated as UNTRUSTED and used only for navigation hints.
    fn review_local(
        &self,
        output: &WorkerOutput,
        task_brief: &engine_policy::context::TaskBrief,
    ) -> EngineResult<LocalReviewDecision>;

    /// Reviews the integrated candidate (deep review).
    ///
    /// This is the final quality gate before the candidate is presented
    /// to the user for approval. The Deep Reviewer inspects:
    /// - Architecture consistency across all Worker outputs
    /// - Scene consistency (no conflicting entity modifications)
    /// - Script correctness and style
    /// - Performance risks
    /// - Regression risks
    /// - Editor workflow impact
    /// - Trace completeness
    fn review_deep(
        &self,
        request: &ReviewRequest,
    ) -> EngineResult<ReviewDecision>;

    /// Evaluates objective artifacts and produces findings.
    ///
    /// The reviewer must base findings on objective evidence, not Worker
    /// claims. If objective artifacts are insufficient to verify completion,
    /// safety, rollback, or scope compliance, the reviewer must return
    /// `needs_revision` or `blocked`.
    fn evaluate_artifacts(
        &self,
        artifacts: &[Artifact],
        task_brief: &engine_policy::context::TaskBrief,
        validator_output: Option<&serde_json::Value>,
        audit_report: Option<&serde_json::Value>,
    ) -> EngineResult<Vec<ReviewFinding>>;

    /// Verifies that a repair actually fixed the identified issue.
    ///
    /// Called after a Repair Worker patches the integration candidate.
    /// The reviewer checks that the original finding is resolved and
    /// that no new issues were introduced.
    fn verify_repair(
        &self,
        original_finding: &ReviewFinding,
        repaired_artifacts: &[Artifact],
    ) -> EngineResult<RepairVerification>;
}

/// Outcome of a repair verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepairVerification {
    /// The issue is fully resolved.
    Fixed,
    /// The issue is partially resolved; some aspects remain.
    PartiallyFixed,
    /// The issue is not resolved.
    NotFixed,
}

/// The Deep Reviewer implementation that uses an AI model in a fresh session.
///
/// The Deep Reviewer receives:
/// - The review rubric and task brief (from the context packet)
/// - Accepted Worker artifacts (objective diffs, previews, logs)
/// - Deterministic validator output
/// - Deterministic audit report (if applicable)
///
/// It does NOT receive:
/// - The Manager's full conversation
/// - Raw Worker chat or self-reports
/// - Prior repair reasoning
/// - The user's raw prompt text
pub struct DeepReviewer<M: crate::ModelProvider> {
    model: M,
}

impl<M: crate::ModelProvider> DeepReviewer<M> {
    /// Creates a Deep Reviewer backed by an AI model provider.
    pub fn new(model: M) -> Self {
        Self { model }
    }
}

impl<M: crate::ModelProvider> Reviewer for DeepReviewer<M> {
    fn review_local(
        &self,
        _output: &WorkerOutput,
        _task_brief: &engine_policy::context::TaskBrief,
    ) -> EngineResult<LocalReviewDecision> {
        // Local review is typically performed by the Manager or a lightweight
        // reviewer. The Deep Reviewer focuses on the integrated candidate.
        // This method exists for completeness; in practice, local review may
        // be a Manager responsibility or a separate lightweight Worker.
        todo!("Local review is a Manager responsibility in the initial implementation")
    }

    fn review_deep(
        &self,
        _request: &ReviewRequest,
    ) -> EngineResult<ReviewDecision> {
        // The Deep Reviewer receives the review request in a fresh session.
        // It evaluates the integrated candidate against the task brief,
        // acceptance criteria, and review rubric.
        todo!("Deep review implementation (M4 milestone)")
    }

    fn evaluate_artifacts(
        &self,
        _artifacts: &[Artifact],
        _task_brief: &engine_policy::context::TaskBrief,
        _validator_output: Option<&serde_json::Value>,
        _audit_report: Option<&serde_json::Value>,
    ) -> EngineResult<Vec<ReviewFinding>> {
        todo!("Artifact evaluation against review rubric")
    }

    fn verify_repair(
        &self,
        _original_finding: &ReviewFinding,
        _repaired_artifacts: &[Artifact],
    ) -> EngineResult<RepairVerification> {
        todo!("Repair verification")
    }
}
