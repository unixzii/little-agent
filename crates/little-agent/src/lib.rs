//! An out-of-the-box agent that assembles various tools and model providers.
//!
//! The crate includes a CLI tool for using in the terminal. And you can also
//! use it as a library to bring agent functionality into your own host apps.

#![deny(missing_docs)]

#[allow(unused_imports)]
#[macro_use]
extern crate tracing;

mod session;
pub mod tools;

pub use session::{Session, SessionBuilder};

/// Re-exports of [`little_agent_core`] crate.
pub mod core {
    pub use little_agent_core::*;
}
