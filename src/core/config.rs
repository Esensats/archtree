use std::env;

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
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct ConfigBuilder {
    output_path: Option<String>,
    show_progress: bool,
    seven_zip_path: Option<String>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn output_path(mut self, path: Option<&str>, try_env: bool) -> Self {
        if let Some(p) = path {
            if !p.trim().is_empty() {
                self.output_path = Some(p.to_string());
                return self;
            }
        }
        if try_env {
            if let Ok(env_path) = env::var("ARCHTREE_OUTPUT_PATH") {
                self.output_path = Some(env_path.trim().to_string());
            }
        }
        self
    }

    pub fn show_progress(mut self, show: bool) -> Self {
        self.show_progress = show;
        self
    }

    pub fn seven_zip_path(mut self, path: Option<&str>, try_env: bool) -> Self {
        if let Some(p) = path {
            if !p.trim().is_empty() {
                self.seven_zip_path = Some(p.to_string());
                return self;
            }
        }
        if try_env {
            if let Ok(env_path) = env::var("SEVEN_ZIP_PATH") {
                self.seven_zip_path = Some(env_path.trim().to_string());
            }
        }
        self
    }

    pub fn build(self) -> Result<Config, anyhow::Error> {
        let output_path = self
            .output_path
            .ok_or_else(|| anyhow::anyhow!("Output path must be set"))?
            .trim()
            .to_string();
        if output_path.is_empty() {
            anyhow::bail!("Output path cannot be empty");
        }
        Ok(Config {
            output_path,
            show_progress: self.show_progress,
            seven_zip_path: self.seven_zip_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Expect error if output path is not set
    #[test]
    fn test_default_config() {
        let config = Config::builder().build();

        assert!(config.is_err());
    }

    #[test]
    fn test_config_with_custom_values() {
        let config = Config::builder()
            .output_path(Some("custom.7z"), false)
            .show_progress(false)
            .seven_zip_path(Some("C:\\custom\\7z.exe"), false)
            .build()
            .expect("Failed to create custom config");

        assert_eq!(config.output_path, "custom.7z");
        assert!(!config.show_progress);
        assert_eq!(config.seven_zip_path.unwrap(), "C:\\custom\\7z.exe");
    }

    #[test]
    fn test_config_from_env() {
        // Set test environment variable
        unsafe {
            env::set_var("ARCHTREE_OUTPUT_PATH", "test-archive.7z");
            env::set_var("SEVEN_ZIP_PATH", "test-7z.exe");
        }

        let config = Config::builder()
            .output_path(None, true)
            .seven_zip_path(None, true)
            .build()
            .expect("Failed to create config from environment");

        assert_eq!(config.output_path, "test-archive.7z");
        assert_eq!(config.seven_zip_path.unwrap(), "test-7z.exe");

        // Clean up
        unsafe {
            env::remove_var("ARCHTREE_OUTPUT_PATH");
            env::remove_var("SEVEN_ZIP_PATH");
        }
    }
}
