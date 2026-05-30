//! Trust boundary labels for context packets, messages, and model prompts.
//!
//! Every piece of data that enters a model prompt or tool execution must
//! carry an explicit trust label. Labels are structured so that the tool
//! layer can enforce: untrusted data cannot alter policy, scope, grants,
//! validation requirements, or approval requirements.

use serde::{Deserialize, Serialize};

/// Trust classification for a piece of data.
///
/// The tool layer MUST enforce:
/// - `Trusted` data may influence policy decisions.
/// - `Untrusted` data MUST NOT alter system instructions, task scope,
///   permission grants, tool schemas, validation requirements, approval
///   requirements, or Reviewer rubric.
/// - `Deterministic` data is produced by trusted Rust code (validators,
///   auditors) and may influence policy.
/// - `Advisory` data is AI judgment (Reviewer, Risk Auditor output); it
///   may raise risk or recommend blocking but must NOT grant permissions,
///   lower risk classifications, or override deterministic decisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLabel {
    /// Trusted compiled policy code, operation schemas, sandbox checks.
    TrustedPolicy,
    /// Trusted task scope as validated by the Capability Issuer.
    TrustedTaskScope,
    /// Trusted command schema from the capability registry.
    TrustedCommandSchema,
    /// Output of a deterministic validator, auditor, or preprocessor.
    DeterministicValidationReport,
    /// Output of a deterministic static script auditor.
    DeterministicAuditReport,
    /// The user's raw prompt text — may contain prompt injection.
    UntrustedUserPrompt,
    /// Content from a project file — may contain poisoned comments/metadata.
    UntrustedProjectFile,
    /// Script source code before preprocessing.
    UntrustedScriptContent,
    /// Plugin metadata or command labels from third-party sources.
    UntrustedPluginMetadata,
    /// A Worker agent's self-report, rationale, or claimed test results.
    UntrustedWorkerReport,
    /// A Reviewer agent's output (advisory only, not authoritative).
    AdvisoryReviewerOutput,
    /// A Risk Auditor agent's output (advisory, monotonic).
    AdvisoryRiskAuditOutput,
    /// A Manager agent's plan or decomposition (untrusted, reviewable).
    UntrustedManagerPlan,
    /// Third-party skill instruction body (prompt-injection vector).
    UntrustedSkillInstruction,
    /// MCP server response data.
    UntrustedMcpResponse,
}

impl TrustLabel {
    /// Returns true when the label represents trusted or deterministic data.
    pub const fn is_trusted(self) -> bool {
        matches!(
            self,
            Self::TrustedPolicy
                | Self::TrustedTaskScope
                | Self::TrustedCommandSchema
                | Self::DeterministicValidationReport
                | Self::DeterministicAuditReport
        )
    }

    /// Returns true when the label represents untrusted data that must
    /// not influence policy, scope, grants, or validation.
    pub const fn is_untrusted(self) -> bool {
        matches!(
            self,
            Self::UntrustedUserPrompt
                | Self::UntrustedProjectFile
                | Self::UntrustedScriptContent
                | Self::UntrustedPluginMetadata
                | Self::UntrustedWorkerReport
                | Self::UntrustedManagerPlan
                | Self::UntrustedSkillInstruction
                | Self::UntrustedMcpResponse
        )
    }

    /// Returns true when the label represents advisory AI output that
    /// is monotonic (can raise risk, cannot lower or grant).
    pub const fn is_advisory(self) -> bool {
        matches!(
            self,
            Self::AdvisoryReviewerOutput | Self::AdvisoryRiskAuditOutput
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_labels_are_identified() {
        assert!(TrustLabel::TrustedPolicy.is_trusted());
        assert!(TrustLabel::DeterministicValidationReport.is_trusted());
        assert!(!TrustLabel::UntrustedUserPrompt.is_trusted());
        assert!(!TrustLabel::AdvisoryReviewerOutput.is_trusted());
    }

    #[test]
    fn untrusted_labels_are_identified() {
        assert!(TrustLabel::UntrustedUserPrompt.is_untrusted());
        assert!(TrustLabel::UntrustedWorkerReport.is_untrusted());
        assert!(!TrustLabel::TrustedPolicy.is_untrusted());
    }

    #[test]
    fn advisory_labels_are_neither_trusted_nor_untrusted() {
        assert!(TrustLabel::AdvisoryReviewerOutput.is_advisory());
        assert!(!TrustLabel::AdvisoryReviewerOutput.is_trusted());
        assert!(!TrustLabel::AdvisoryReviewerOutput.is_untrusted());
    }
}
