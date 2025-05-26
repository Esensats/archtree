use anyhow::Result;
use std::env;
use std::path::PathBuf;

/// Configuration for the backup tool
#[derive(Debug, Clone)]
pub struct Config {
    /// Path where the archive will be created
    pub output_path: String,
    /// Whether to show progress during operations
    pub show_progress: bool,
    /// Path to the 7-Zip executable (if not in PATH)
    pub seven_zip_path: Option<String>,
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self {
            output_path: Self::default_output_path(),
            show_progress: true,
            seven_zip_path: None,
        }
    }

    /// Create configuration from environment variables and command line arguments
    pub fn from_env() -> Result<Self> {
        let mut config = Self::new();

        // Check for TEST_ARCHIVE_PATH environment variable (for compatibility with PowerShell version)
        if let Ok(test_path) = env::var("TEST_ARCHIVE_PATH") {
            config.output_path = test_path;
        }

        // Check for SEVEN_ZIP_PATH environment variable
        if let Ok(seven_zip_path) = env::var("SEVEN_ZIP_PATH") {
            config.seven_zip_path = Some(seven_zip_path);
        }

        Ok(config)
    }

    /// Get the default output path (Desktop/backup.7z)
    fn default_output_path() -> String {
        if let Ok(user_profile) = env::var("USERPROFILE") {
            PathBuf::from(user_profile)
                .join("Desktop")
                .join("backup.7z")
                .to_string_lossy()
                .to_string()
        } else {
            "backup.7z".to_string()
        }
    }

    /// Set a custom output path
    pub fn with_output_path(mut self, path: String) -> Self {
        self.output_path = path;
        self
    }

    /// Set whether to show progress
    pub fn with_progress(mut self, show_progress: bool) -> Self {
        self.show_progress = show_progress;
        self
    }

    /// Set a custom 7-Zip path
    pub fn with_seven_zip_path(mut self, path: String) -> Self {
        self.seven_zip_path = Some(path);
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = Config::new();
        assert!(config.output_path.ends_with("backup.7z"));
        assert!(config.show_progress);
        assert!(config.seven_zip_path.is_none());
    }

    #[test]
    fn test_config_with_custom_values() {
        let config = Config::new()
            .with_output_path("custom.7z".to_string())
            .with_progress(false)
            .with_seven_zip_path("C:\\custom\\7z.exe".to_string());

        assert_eq!(config.output_path, "custom.7z");
        assert!(!config.show_progress);
        assert_eq!(config.seven_zip_path.unwrap(), "C:\\custom\\7z.exe");
    }

    #[test]
    fn test_config_from_env() {
        // Set test environment variable
        unsafe {
            env::set_var("TEST_ARCHIVE_PATH", "test-archive.7z");
            env::set_var("SEVEN_ZIP_PATH", "test-7z.exe");
        }

        let config = Config::from_env().unwrap();
        assert_eq!(config.output_path, "test-archive.7z");
        assert_eq!(config.seven_zip_path.unwrap(), "test-7z.exe");

        // Clean up
        unsafe {
            env::remove_var("TEST_ARCHIVE_PATH");
            env::remove_var("SEVEN_ZIP_PATH");
        }
    }
}
