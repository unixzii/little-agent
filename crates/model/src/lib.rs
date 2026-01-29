//! An abstraction layer for different LLMs.
//!
//! This crate establishes an unified protocol for the agent to interact
//! with various supported LLMs, so that the agent can seamlessly switch
//! between them without modifying the core codebase.
//!
//! Types in this crate don't define any behavior, instead they are the
//! constraints that the implementors should adhere to.
//!
//! Users of this crate may add some extra functionalities or wrappers,
//! depending on their own use cases. Those extra code should be placed
//! in their own crate.

#![deny(missing_docs)]

mod error;
mod opaque;
mod provider;
mod request;
mod response;

pub use error::*;
pub use opaque::*;
pub use provider::*;
pub use request::*;
pub use response::*;
