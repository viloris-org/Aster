#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Agent Cluster orchestration for the Aster AI Editor Copilot.
//!
//! In Auto mode, a Manager decomposes user requests into bounded tasks,
//! specialized Workers execute in parallel inside an isolated git-backed
//! task workspace, and a Deep Reviewer inspects the integrated result.
//!
//! This crate owns:
//! - Message protocol types (TaskAssignment, WorkerOutput, ReviewDecision, etc.)
//! - Manager orchestration (decomposition, integration, report generation)
//! - Worker traits and concrete Worker implementations
//! - Reviewer traits and review logic
//! - Transaction bundle definition and verification
//! - Repair loop and problem reporting

pub mod protocol;
pub mod manager;
pub mod worker;
pub mod reviewer;
pub mod bundle;

// Re-export ModelProvider for convenience (used by Worker and Reviewer impls)
pub use worker::ModelProvider;
