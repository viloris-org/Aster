//! Editor Copilot runtime primitives.
//!
//! Copilot is intentionally short-lived and editor-scoped. It reuses the shared
//! [`AgentKernel`] while keeping conversation history, approval policy, and
//! Quest promotion state local to the editor surface.

use engine_core::{EngineError, EngineResult};
use engine_editor::{ProjectContext, agent::PermissionPolicy};

use crate::{AgentKernel, AgentSession, ChatMessage};

/// Runtime for short-lived Editor Copilot sessions.
#[derive(Clone, Default)]
pub struct CopilotRuntime {
    kernel: AgentKernel,
}

impl CopilotRuntime {
    /// Creates a Copilot runtime over a shared agent kernel.
    pub fn new(kernel: AgentKernel) -> Self {
        Self { kernel }
    }

    /// Starts a short-lived editor-scoped Copilot session.
    pub fn start_session(&self, context: ProjectContext) -> EngineResult<CopilotSession> {
        let agent = AgentSession::with_kernel(context, self.kernel.clone())?;
        Ok(CopilotSession {
            agent,
            history: Vec::new(),
            policy: self.kernel.default_policy().clone(),
        })
    }
}

/// A short-lived Copilot session bound to one editor project snapshot.
pub struct CopilotSession {
    /// Agent session used for planning and local tool execution.
    pub agent: AgentSession,
    /// Bounded conversation history owned by the Copilot surface.
    pub history: Vec<ChatMessage>,
    /// Current approval/write policy for this short session.
    pub policy: PermissionPolicy,
}

impl CopilotSession {
    /// Replaces the active permission policy.
    pub fn set_policy(&mut self, policy: PermissionPolicy) {
        self.policy = policy;
    }

    /// Records user and assistant messages into the short session history.
    pub fn record_exchange(&mut self, user: impl Into<String>, assistant: impl Into<String>) {
        self.history.push(ChatMessage::user(user));
        self.history.push(ChatMessage::assistant(assistant));
    }

    /// Builds the normalized payload for promoting this local session to Quest.
    pub fn promote_to_quest(
        &self,
        title: impl Into<String>,
        goal: impl Into<String>,
    ) -> EngineResult<CopilotQuestPromotion> {
        let title = title.into();
        let goal = goal.into();
        if title.trim().is_empty() {
            return Err(EngineError::config("Quest title must not be empty"));
        }
        if goal.trim().is_empty() {
            return Err(EngineError::config("Quest goal must not be empty"));
        }
        Ok(CopilotQuestPromotion {
            title,
            goal,
            history: self.history.clone(),
            project_root: self.agent.context.root.clone(),
        })
    }
}

/// Normalized handoff from Copilot into a persistent Quest.
#[derive(Clone, Debug)]
pub struct CopilotQuestPromotion {
    /// Quest title.
    pub title: String,
    /// Quest goal.
    pub goal: String,
    /// Copilot conversation context to seed the Quest intent/spec flow.
    pub history: Vec<ChatMessage>,
    /// Project root associated with the Copilot session.
    pub project_root: std::path::PathBuf,
}
