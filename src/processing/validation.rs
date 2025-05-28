use crate::core::{ArchtreeError, Result};
use async_trait::async_trait;
use std::path::Path;
use tokio::fs;

/// Trait for path validation strategies
#[async_trait]
pub trait PathValidator: Send + Sync {
    /// Validate if a path exists and is accessible
    async fn validate(&self, path: &Path) -> Result<bool>;

    /// Validate multiple paths and return only the valid ones
    async fn validate_paths(&self, paths: &[String]) -> Result<Vec<String>> {
        let mut valid_paths = Vec::new();
        
        for path_str in paths {
            let path = Path::new(path_str);
            if self.validate(path).await? {
                valid_paths.push(path_str.clone());
            }
        }
        
        Ok(valid_paths)
    }

    /// Get a human-readable description of this validator strategy  
    fn description(&self) -> &'static str;
}

/// File system-based path validator
pub struct FileSystemValidator;

impl FileSystemValidator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileSystemValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PathValidator for FileSystemValidator {
    async fn validate(&self, path: &Path) -> Result<bool> {
        match fs::metadata(path).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(ArchtreeError::path_processing_with_source(
                "Failed to validate path",
                Some(path.to_string_lossy().to_string()),
                e,
            )),
        }
    }

    fn description(&self) -> &'static str {
        "File system validator (checks if path exists and is accessible)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_system_validator() {
        let validator = FileSystemValidator::new();
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        
        // Create test file
        fs::write(&test_file, "test content").unwrap();
        
        // Should validate existing file
        assert!(validator.validate(&test_file).await.unwrap());
        
        // Should not validate non-existent file
        let non_existent = temp_dir.path().join("non_existent.txt");
        assert!(!validator.validate(&non_existent).await.unwrap());
    }
}
