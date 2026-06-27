#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Deterministic policy engine for the Varg AI Editor Copilot.
//!
//! Owns:
//! - Core AI identifiers (TaskId, SnapshotId, WorkspaceId, GrantHash, BundleHash, ContextHash)
//! - Trust boundary labels (TrustLabel)
//! - Capability grant definition and signing (CapabilityGrant, CapabilityIssuer)
//! - Risk classification (RiskClass, RiskClassifier)
//!
//! This crate is trusted Rust code. It contains no AI model runtime, no GPU
//! code, no asset pipeline, and no scripting engine. It is designed to be
//! auditable as a single compilation unit and, in a future milestone, to be
//! extracted into a dedicated Policy Daemon process.

pub mod context;
pub mod grant;
pub mod ids;
pub mod risk;
pub mod trust;
