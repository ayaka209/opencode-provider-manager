//! auth: Read-only access to OpenCode's auth.json for API key status display.
//!
//! OpenCode stores credentials in ~/.local/share/opencode/auth.json
//! with the format: { "provider_id": { "type": "api", "key": "sk-..." } }

pub mod error;
pub mod parser;
pub mod status;

pub use error::{AuthError, Result};
pub use status::provider_env_var;
pub use status::ProviderAuthStatus;
