//! Core logic including agent loop, tool execution, configurations, etc.

#![deny(missing_docs)]
#![deny(clippy::missing_safety_doc)]

#[macro_use]
extern crate tracing;

mod agent;
pub mod conversation;
mod model_client;

pub use agent::{Agent, AgentBuilder};
