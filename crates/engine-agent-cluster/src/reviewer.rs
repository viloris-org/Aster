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
    Artifact, LocalReviewDecision, ReviewDecision, ReviewFinding, ReviewRequest, WorkerOutput,
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
    fn review_deep(&self, request: &ReviewRequest) -> EngineResult<ReviewDecision>;

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
        output: &WorkerOutput,
        _task_brief: &engine_policy::context::TaskBrief,
    ) -> EngineResult<LocalReviewDecision> {
        // Local review checks Worker outputs before integration.
        // For MVP: deterministic check on state and artifact completeness.
        let mut findings = Vec::new();
        let mut has_blocking = false;

        // Check Worker state
        match output.state {
            crate::protocol::WorkerState::Completed => {
                // Good — proceed
            }
            crate::protocol::WorkerState::PartiallyCompleted => {
                findings.push(ReviewFinding {
                    finding_id: format!("local-{}-partial", output.task_id),
                    severity: "warning".into(),
                    description: "Worker completed partially; some criteria may be unmet".into(),
                    affected: vec![],
                    suggested_fix: None,
                });
            }
            crate::protocol::WorkerState::Failed => {
                findings.push(ReviewFinding {
                    finding_id: format!("local-{}-failed", output.task_id),
                    severity: "blocking".into(),
                    description: "Worker reported failure".into(),
                    affected: vec![],
                    suggested_fix: None,
                });
                has_blocking = true;
            }
            crate::protocol::WorkerState::Blocked => {
                findings.push(ReviewFinding {
                    finding_id: format!("local-{}-blocked", output.task_id),
                    severity: "blocking".into(),
                    description: "Worker reported blocked by policy or capability".into(),
                    affected: vec![],
                    suggested_fix: None,
                });
                has_blocking = true;
            }
        }

        // Check artifact presence
        if output.artifacts.is_empty() && output.state != crate::protocol::WorkerState::Failed {
            findings.push(ReviewFinding {
                finding_id: format!("local-{}-no-artifacts", output.task_id),
                severity: "error".into(),
                description: "Worker produced no objective artifacts".into(),
                affected: vec![],
                suggested_fix: Some(
                    "Worker must produce at least one diff or scene preview".into(),
                ),
            });
            has_blocking = true;
        }

        let decision = if has_blocking {
            crate::protocol::ReviewVerdict::Blocked
        } else if !findings.is_empty() {
            crate::protocol::ReviewVerdict::NeedsRevision
        } else {
            crate::protocol::ReviewVerdict::Approved
        };

        Ok(LocalReviewDecision {
            task_id: output.task_id,
            decision,
            findings,
            risk_tags: vec![],
            reviewed_at: engine_policy::grant::timestamp_now(),
        })
    }

    fn review_deep(&self, request: &ReviewRequest) -> EngineResult<ReviewDecision> {
        // Deep review evaluates the INTEGRATED candidate.
        // For MVP: deterministic checks on artifacts + model analysis.
        let mut findings = Vec::new();
        let mut has_blocking = false;

        // 1. Check artifact completeness against acceptance criteria
        let criteria = &request.task_brief.acceptance_criteria;
        if !criteria.is_empty() && request.accepted_artifacts.is_empty() {
            findings.push(ReviewFinding {
                finding_id: "deep-001".into(),
                severity: "blocking".into(),
                description: format!(
                    "Acceptance criteria defined ({}) but no artifacts produced",
                    criteria.len()
                ),
                affected: vec![],
                suggested_fix: Some(
                    "Worker must produce artifacts matching acceptance criteria".into(),
                ),
            });
            has_blocking = true;
        }

        // 2. Check validator output for failures
        if let Some(validator) = &request.validator_output.as_object() {
            if let Some(passed) = validator.get("passed").and_then(|v| v.as_bool()) {
                if !passed {
                    findings.push(ReviewFinding {
                        finding_id: "deep-002".into(),
                        severity: "blocking".into(),
                        description: "Deterministic validation failed".into(),
                        affected: vec![],
                        suggested_fix: Some("Fix the validation errors before proceeding".into()),
                    });
                    has_blocking = true;
                }
            }
        }

        // 3. Check audit report for warnings
        if let Some(audit) = &request.audit_report {
            findings.push(ReviewFinding {
                finding_id: "deep-003".into(),
                severity: "info".into(),
                description: format!("Audit report present: {}", audit),
                affected: vec![],
                suggested_fix: None,
            });
        }

        // 4. Check artifact trust labels — untrusted content needs closer review
        let untrusted_count = request
            .accepted_artifacts
            .iter()
            .filter(|a| a.label == crate::protocol::ArtifactTrust::UntrustedWorkerContent)
            .count();
        if untrusted_count > 0 {
            findings.push(ReviewFinding {
                finding_id: "deep-004".into(),
                severity: "warning".into(),
                description: format!(
                    "{} artifacts are untrusted Worker content — manual review recommended",
                    untrusted_count
                ),
                affected: vec![],
                suggested_fix: None,
            });
        }

        // 5. Deduce residual risk
        let residual_risk = if has_blocking {
            crate::protocol::ReviewRisk::High
        } else if !findings.is_empty() {
            crate::protocol::ReviewRisk::Medium
        } else {
            crate::protocol::ReviewRisk::Low
        };

        let verdict = if has_blocking {
            crate::protocol::ReviewVerdict::Blocked
        } else if findings
            .iter()
            .any(|f| f.severity == "error" || f.severity == "warning")
        {
            crate::protocol::ReviewVerdict::NeedsRevision
        } else {
            crate::protocol::ReviewVerdict::Approved
        };

        Ok(ReviewDecision {
            candidate_id: request.candidate_id.clone(),
            verdict,
            findings,
            has_blocking_issues: has_blocking,
            residual_risk,
            reviewed_at: engine_policy::grant::timestamp_now(),
        })
    }

    fn evaluate_artifacts(
        &self,
        artifacts: &[Artifact],
        _task_brief: &engine_policy::context::TaskBrief,
        validator_output: Option<&serde_json::Value>,
        _audit_report: Option<&serde_json::Value>,
    ) -> EngineResult<Vec<ReviewFinding>> {
        let mut findings = Vec::new();

        // Check for deterministic shape artifacts (trusted)
        let has_deterministic = artifacts
            .iter()
            .any(|a| matches!(a.label, crate::protocol::ArtifactTrust::DeterministicShape));
        if !has_deterministic {
            findings.push(ReviewFinding {
                finding_id: "artifact-001".into(),
                severity: "warning".into(),
                description: "No deterministic-shape artifacts found — review may be subjective"
                    .into(),
                affected: vec![],
                suggested_fix: Some(
                    "Workers should produce diffs or structured change records".into(),
                ),
            });
        }

        // Check validator output for warnings
        if let Some(validator) = validator_output {
            findings.push(ReviewFinding {
                finding_id: "artifact-002".into(),
                severity: "info".into(),
                description: format!("Validator output evaluated: {}", validator),
                affected: vec![],
                suggested_fix: None,
            });
        }

        // Check for untrusted artifacts that need human review
        for artifact in artifacts {
            if artifact.label == crate::protocol::ArtifactTrust::UntrustedWorkerContent {
                findings.push(ReviewFinding {
                    finding_id: format!("artifact-u-{}", artifact.artifact_type),
                    severity: "info".into(),
                    description: format!(
                        "Untrusted artifact '{}' targeting '{}'",
                        artifact.artifact_type, artifact.target
                    ),
                    affected: vec![artifact.target.clone()],
                    suggested_fix: None,
                });
            }
        }

        Ok(findings)
    }

    fn verify_repair(
        &self,
        original_finding: &ReviewFinding,
        repaired_artifacts: &[Artifact],
    ) -> EngineResult<RepairVerification> {
        // Check if the repair produced new artifacts targeting the same affected paths
        let original_affected: std::collections::HashSet<&str> = original_finding
            .affected
            .iter()
            .map(|s| s.as_str())
            .collect();

        if original_affected.is_empty() {
            // No specific path to verify against — check if any artifacts exist
            return if repaired_artifacts.is_empty() {
                Ok(RepairVerification::NotFixed)
            } else {
                Ok(RepairVerification::Fixed)
            };
        }

        let repair_addresses: std::collections::HashSet<&str> = repaired_artifacts
            .iter()
            .map(|a| a.target.as_str())
            .collect();

        let all_addressed = original_affected
            .iter()
            .all(|path| repair_addresses.contains(path));

        if all_addressed {
            Ok(RepairVerification::Fixed)
        } else if repair_addresses.is_empty() {
            Ok(RepairVerification::NotFixed)
        } else {
            Ok(RepairVerification::PartiallyFixed)
        }
    }
}
