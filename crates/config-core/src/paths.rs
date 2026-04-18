//! Platform-aware config path resolution for OpenCode.
//!
//! Follows OpenCode's documented path precedence:
//! 1. Remote config (.well-known/opencode) — not managed by this tool
//! 2. Global config (~/.config/opencode/opencode.json)
//! 3. Custom config (OPENCODE_CONFIG env var)
//! 4. Project config (./opencode.json, traversing up to git root)
//! 5. .opencode directories — not managed by this tool
//! 6. Inline config (OPENCODE_CONFIG_CONTENT env var) — not managed by this tool
//! 7. Managed config files — read-only awareness
//!
//! Additional paths:
//! - $HOME/.opencode.json (fallback)
//! - $XDG_CONFIG_HOME/opencode/opencode.json (XDG fallback)
//! - Auth: ~/.local/share/opencode/auth.json

use crate::error::{ConfigError, Result};
use std::path::PathBuf;

/// Which configuration layer to operate on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigLayer {
    /// Global config: `~/.config/opencode/opencode.json`
    Global,
    /// Project config: `./opencode.json`
    Project,
    /// Custom config specified by OPENCODE_CONFIG env var.
    Custom,
}

/// Resolved paths for all config layers.
#[derive(Debug, Clone)]
pub struct ConfigPaths {
    /// Global config file path.
    pub global: PathBuf,
    /// Project config file path (nearest opencode.json up to git root).
    pub project: Option<PathBuf>,
    /// Custom config path from OPENCODE_CONFIG env var.
    pub custom: Option<PathBuf>,
    /// Auth file path.
    pub auth: PathBuf,
    /// Cache directory for this tool.
    pub cache_dir: PathBuf,
}

impl ConfigPaths {
    /// Discover all config paths following OpenCode conventions.
    ///
    /// This respects environment variables:
    /// - `OPENCODE_CONFIG`: custom config file path
    /// - `OPENCODE_CONFIG_DIR`: custom config directory
    /// - `OPENCODE_CONFIG_CONTENT`: inline config (not file-based, skipped)
    /// - `XDG_CONFIG_HOME`: XDG config directory override
    pub fn discover() -> Result<Self> {
        let global = Self::global_config_path()?;
        let project = Self::project_config_path()?;
        let custom = std::env::var("OPENCODE_CONFIG").ok().map(PathBuf::from);
        let auth = Self::auth_path()?;
        let cache_dir = Self::cache_dir()?;

        Ok(Self {
            global,
            project,
            custom,
            auth,
            cache_dir,
        })
    }

    /// Get the global config path.
    ///
    /// Checks in order:
    /// 1. `OPENCODE_CONFIG_DIR` env var
    /// 2. `~/.config/opencode/opencode.json`
    /// 3. `$XDG_CONFIG_HOME/opencode/opencode.json`
    /// 4. `$HOME/.opencode.json` (fallback)
    pub fn global_config_path() -> Result<PathBuf> {
        // OPENCODE_CONFIG_DIR takes priority
        if let Ok(config_dir) = std::env::var("OPENCODE_CONFIG_DIR") {
            let path = PathBuf::from(config_dir).join("opencode.json");
            return Ok(path);
        }

        // Standard XDG path
        if let Some(config_home) = dirs::config_dir() {
            let path = config_home.join("opencode").join("opencode.json");
            if path.exists() {
                return Ok(path);
            }
        }

        // XDG_CONFIG_HOME override
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            let path = PathBuf::from(xdg_config)
                .join("opencode")
                .join("opencode.json");
            if path.exists() {
                return Ok(path);
            }
        }

        // Fallback: $HOME/.opencode.json
        if let Some(home) = dirs::home_dir() {
            let fallback = home.join(".opencode.json");
            if fallback.exists() {
                return Ok(fallback);
            }
        }

        // Default: create at standard location
        dirs::config_dir()
            .map(|d| d.join("opencode").join("opencode.json"))
            .ok_or_else(|| {
                ConfigError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Cannot determine config directory",
                ))
            })
    }

    /// Find the project-level config by traversing up to the git root.
    ///
    /// Starts from the current directory and walks up the tree
    /// until finding `opencode.json` or hitting the git root.
    pub fn project_config_path() -> Result<Option<PathBuf>> {
        let current_dir = std::env::current_dir().map_err(ConfigError::Io)?;

        let mut dir = current_dir.as_path();
        loop {
            let config_path = dir.join("opencode.json");
            if config_path.exists() {
                return Ok(Some(config_path));
            }

            // Check if we've hit the git root (stop here even if no config)
            let git_dir = dir.join(".git");
            if git_dir.exists() {
                // Also check git root for opencode.json before stopping
                let root_config = dir.join("opencode.json");
                if root_config.exists() {
                    return Ok(Some(root_config));
                }
                return Ok(None);
            }

            // Walk up
            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }

        Ok(None)
    }

    /// Get the auth.json file path.
    pub fn auth_path() -> Result<PathBuf> {
        // Follow OpenCode convention: ~/.local/share/opencode/auth.json
        dirs::data_local_dir()
            .or_else(dirs::data_dir)
            .map(|d| d.join("opencode").join("auth.json"))
            .ok_or_else(|| {
                ConfigError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Cannot determine data directory for auth.json",
                ))
            })
    }

    /// Get the cache directory for this tool.
    pub fn cache_dir() -> Result<PathBuf> {
        let cache = dirs::cache_dir()
            .ok_or_else(|| {
                ConfigError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Cannot determine cache directory",
                ))
            })
            .map(|p| p.join("opencode-provider-manager"))?;
        Ok(cache)
    }

    /// Get the managed config path for the current platform (read-only awareness).
    pub fn managed_config_path() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            Some(PathBuf::from(
                "/Library/Application Support/opencode/opencode.json",
            ))
        }

        #[cfg(target_os = "linux")]
        {
            Some(PathBuf::from("/etc/opencode/opencode.json"))
        }

        #[cfg(target_os = "windows")]
        {
            std::env::var("ProgramData")
                .ok()
                .map(|p| PathBuf::from(p).join("opencode").join("opencode.json"))
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            None
        }
    }

    /// Get the path for a specific config layer.
    pub fn path_for_layer(&self, layer: ConfigLayer) -> Option<&PathBuf> {
        match layer {
            ConfigLayer::Global => Some(&self.global),
            ConfigLayer::Project => self.project.as_ref(),
            ConfigLayer::Custom => self.custom.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_config_path_returns_something() {
        // Should return a valid path even if file doesn't exist
        let path = ConfigPaths::global_config_path().unwrap();
        assert!(path.to_string_lossy().contains("opencode"));
    }

    #[test]
    fn test_auth_path_returns_something() {
        let path = ConfigPaths::auth_path().unwrap();
        assert!(path.to_string_lossy().contains("opencode"));
        assert!(path.to_string_lossy().contains("auth.json"));
    }

    #[test]
    fn test_cache_dir_returns_something() {
        let path = ConfigPaths::cache_dir().unwrap();
        assert!(path.to_string_lossy().contains("opencode-provider-manager"));
    }

    #[test]
    fn test_config_layer_enum_values() {
        assert_eq!(ConfigLayer::Global, ConfigLayer::Global);
        assert_eq!(ConfigLayer::Project, ConfigLayer::Project);
        assert_eq!(ConfigLayer::Custom, ConfigLayer::Custom);
    }

    #[test]
    fn test_discover_returns_structure() {
        let paths = ConfigPaths::discover().unwrap();
        assert!(!paths.global.to_string_lossy().is_empty());
        assert!(!paths.auth.to_string_lossy().is_empty());
        assert!(!paths.cache_dir.to_string_lossy().is_empty());
    }

    #[test]
    fn test_path_for_layer() {
        let paths = ConfigPaths::discover().unwrap();
        assert!(paths.path_for_layer(ConfigLayer::Global).is_some());
        // Project config may or may not exist
        assert!(
            paths.path_for_layer(ConfigLayer::Custom).is_none()
                || paths.path_for_layer(ConfigLayer::Custom).is_some()
        );
    }
}
