use anyhow::Error as AnyhowError;
use std::fmt;

/// Structured error types for the archtree application
#[derive(Debug)]
pub enum ArchtreeError {
    /// Configuration related errors
    Config {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    /// Input/Output related errors (file reading, archive creation, etc.)
    Io {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    /// Path processing related errors
    PathProcessing {
        message: String,
        path: Option<String>,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    /// Archive verification related errors
    Verification {
        message: String,
        archive_path: Option<String>,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    /// External tool related errors (7-Zip not found, etc.)
    ExternalTool {
        tool: String,
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    /// Generic errors that don't fit other categories
    Other {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl fmt::Display for ArchtreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchtreeError::Config { message, .. } => {
                write!(f, "Configuration error: {}", message)
            }
            ArchtreeError::Io { message, .. } => {
                write!(f, "I/O error: {}", message)
            }
            ArchtreeError::PathProcessing { message, path, .. } => {
                if let Some(path) = path {
                    write!(f, "Path processing error for '{}': {}", path, message)
                } else {
                    write!(f, "Path processing error: {}", message)
                }
            }
            ArchtreeError::Verification { message, archive_path, .. } => {
                if let Some(archive) = archive_path {
                    write!(f, "Verification error for '{}': {}", archive, message)
                } else {
                    write!(f, "Verification error: {}", message)
                }
            }
            ArchtreeError::ExternalTool { tool, message, .. } => {
                write!(f, "External tool error ({}): {}", tool, message)
            }
            ArchtreeError::Other { message, .. } => {
                write!(f, "Error: {}", message)
            }
        }
    }
}

impl std::error::Error for ArchtreeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ArchtreeError::Config { source, .. }
            | ArchtreeError::Io { source, .. }
            | ArchtreeError::PathProcessing { source, .. }
            | ArchtreeError::Verification { source, .. }
            | ArchtreeError::ExternalTool { source, .. }
            | ArchtreeError::Other { source, .. } => {
                source.as_ref().map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
            }
        }
    }
}

impl ArchtreeError {
    /// Create a configuration error
    pub fn config<S: Into<String>>(message: S) -> Self {
        Self::Config {
            message: message.into(),
            source: None,
        }
    }

    /// Create a configuration error with source
    pub fn config_with_source<S: Into<String>, E: std::error::Error + Send + Sync + 'static>(
        message: S,
        source: E,
    ) -> Self {
        Self::Config {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create an I/O error
    pub fn io<S: Into<String>>(message: S) -> Self {
        Self::Io {
            message: message.into(),
            source: None,
        }
    }

    /// Create an I/O error with source
    pub fn io_with_source<S: Into<String>, E: std::error::Error + Send + Sync + 'static>(
        message: S,
        source: E,
    ) -> Self {
        Self::Io {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a path processing error
    pub fn path_processing<S: Into<String>, P: Into<String>>(message: S, path: Option<P>) -> Self {
        Self::PathProcessing {
            message: message.into(),
            path: path.map(|p| p.into()),
            source: None,
        }
    }

    /// Create a path processing error with source
    pub fn path_processing_with_source<
        S: Into<String>,
        P: Into<String>,
        E: std::error::Error + Send + Sync + 'static,
    >(
        message: S,
        path: Option<P>,
        source: E,
    ) -> Self {
        Self::PathProcessing {
            message: message.into(),
            path: path.map(|p| p.into()),
            source: Some(Box::new(source)),
        }
    }

    /// Create a verification error
    pub fn verification<S: Into<String>, A: Into<String>>(
        message: S,
        archive_path: Option<A>,
    ) -> Self {
        Self::Verification {
            message: message.into(),
            archive_path: archive_path.map(|a| a.into()),
            source: None,
        }
    }

    /// Create an external tool error
    pub fn external_tool<T: Into<String>, S: Into<String>>(tool: T, message: S) -> Self {
        Self::ExternalTool {
            tool: tool.into(),
            message: message.into(),
            source: None,
        }
    }

    /// Create an external tool error with source
    pub fn external_tool_with_source<
        T: Into<String>,
        S: Into<String>,
        E: std::error::Error + Send + Sync + 'static,
    >(
        tool: T,
        message: S,
        source: E,
    ) -> Self {
        Self::ExternalTool {
            tool: tool.into(),
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

// Allow conversion from anyhow::Error for compatibility
impl From<AnyhowError> for ArchtreeError {
    fn from(error: AnyhowError) -> Self {
        Self::Other {
            message: error.to_string(),
            source: Some(error.into()),
        }
    }
}

impl From<std::io::Error> for ArchtreeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io {
            message: error.to_string(),
            source: Some(Box::new(error)),
        }
    }
}

/// Custom Result type for the application
pub type Result<T> = std::result::Result<T, ArchtreeError>;

/// Extension trait to add context to errors
pub trait ErrorContext<T> {
    /// Add context to convert into ArchtreeError
    fn context_config<S: Into<String>>(self, message: S) -> Result<T>;
    fn context_io<S: Into<String>>(self, message: S) -> Result<T>;
    fn context_path<S: Into<String>, P: Into<String>>(self, message: S, path: P) -> Result<T>;
    fn context_verification<S: Into<String>, A: Into<String>>(
        self,
        message: S,
        archive: A,
    ) -> Result<T>;
    fn context_external<T2: Into<String>, S: Into<String>>(self, tool: T2, message: S) -> Result<T>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context_config<S: Into<String>>(self, message: S) -> Result<T> {
        self.map_err(|e| ArchtreeError::config_with_source(message, e))
    }

    fn context_io<S: Into<String>>(self, message: S) -> Result<T> {
        self.map_err(|e| ArchtreeError::io_with_source(message, e))
    }

    fn context_path<S: Into<String>, P: Into<String>>(self, message: S, path: P) -> Result<T> {
        self.map_err(|e| ArchtreeError::path_processing_with_source(message, Some(path), e))
    }

    fn context_verification<S: Into<String>, A: Into<String>>(
        self,
        message: S,
        archive: A,
    ) -> Result<T> {
        self.map_err(|_e| ArchtreeError::verification(message, Some(archive)))
    }

    fn context_external<T2: Into<String>, S: Into<String>>(self, tool: T2, message: S) -> Result<T> {
        self.map_err(|e| ArchtreeError::external_tool_with_source(tool, message, e))
    }
}
