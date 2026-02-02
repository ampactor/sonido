//! Error types for configuration operations.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during configuration operations.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Failed to read a file
    #[error("failed to read file '{path}': {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to write a file
    #[error("failed to write file '{path}': {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse TOML
    #[error("failed to parse TOML: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Failed to serialize TOML
    #[error("failed to serialize TOML: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    /// Preset not found
    #[error("preset not found: {0}")]
    PresetNotFound(String),

    /// Unknown effect type
    #[error("unknown effect type: {0}")]
    UnknownEffect(String),

    /// Invalid parameter
    #[error("invalid parameter '{param}' for effect '{effect}': {reason}")]
    InvalidParameter {
        effect: String,
        param: String,
        reason: String,
    },

    /// Validation errors
    #[error("validation failed: {0}")]
    Validation(#[from] crate::validation::ValidationError),

    /// Failed to create directory
    #[error("failed to create directory '{path}': {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl ConfigError {
    /// Create a read file error.
    pub fn read_file(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        ConfigError::ReadFile {
            path: path.into(),
            source,
        }
    }

    /// Create a write file error.
    pub fn write_file(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        ConfigError::WriteFile {
            path: path.into(),
            source,
        }
    }

    /// Create a create directory error.
    pub fn create_dir(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        ConfigError::CreateDir {
            path: path.into(),
            source,
        }
    }
}
