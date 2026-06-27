//! Shared agent runtime kernel for Copilot and Quest execution surfaces.
//!
//! The kernel owns cross-surface machinery such as local tool dispatch and the
//! baseline permission policy. Product surfaces decide session lifetime and UI
//! workflow; the kernel keeps the execution substrate consistent.

use engine_editor::agent::PermissionPolicy;

use crate::tool_runtime::AgentToolRuntimeRegistry;

/// Shared execution substrate used by Editor Copilot and Quest.
#[derive(Clone)]
pub struct AgentKernel {
    tool_runtimes: AgentToolRuntimeRegistry,
    default_policy: PermissionPolicy,
}

impl AgentKernel {
    /// Creates a kernel with the default local tool registry and read-only policy.
    pub fn new() -> Self {
        Self {
            tool_runtimes: AgentToolRuntimeRegistry::with_default_tools(),
            default_policy: PermissionPolicy::read_only(),
        }
    }

    /// Creates a kernel with a custom baseline policy.
    pub fn with_default_policy(default_policy: PermissionPolicy) -> Self {
        Self {
            default_policy,
            ..Self::new()
        }
    }

    /// Returns the default permission policy for new short-lived sessions.
    pub fn default_policy(&self) -> &PermissionPolicy {
        &self.default_policy
    }

    pub(crate) fn tool_runtimes(&self) -> &AgentToolRuntimeRegistry {
        &self.tool_runtimes
    }
}

impl Default for AgentKernel {
    fn default() -> Self {
        Self::new()
    }
}
