//! app: Application logic layer connecting config-core, discovery, and auth.

pub mod actions;
pub mod error;
pub mod import;
pub mod state;

pub use error::{AppError, Result};
