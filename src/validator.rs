use anyhow::Result;
use async_trait::async_trait;

/// Trait for path validation strategies
#[async_trait]
pub trait PathValidator: Send + Sync {
    /// Validate a collection of paths and return only the valid ones
    async fn validate_paths(&self, paths: &[String]) -> Result<Vec<String>>;
    
    /// Check if a single path exists and is accessible
    async fn is_valid_path(&self, path: &str) -> bool;
}

/// File system based path validator
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
    async fn validate_paths(&self, paths: &[String]) -> Result<Vec<String>> {
        let mut valid_paths = Vec::new();
        
        for path in paths {
            if self.is_valid_path(path).await {
                valid_paths.push(path.clone());
            } else {
                eprintln!("Warning: Skipping missing path: {}", path);
            }
        }
        
        Ok(valid_paths)
    }
    
    async fn is_valid_path(&self, path: &str) -> bool {
        tokio::fs::metadata(path).await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    
    #[tokio::test]
    async fn test_validate_existing_paths() {
        let validator = FileSystemValidator::new();
        let temp_dir = TempDir::new().unwrap();
        
        // Create a test file
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        
        let paths = vec![
            test_file.to_string_lossy().to_string(),
            temp_dir.path().to_string_lossy().to_string(),
        ];
        
        let valid_paths = validator.validate_paths(&paths).await.unwrap();
        assert_eq!(valid_paths.len(), 2);
    }
    
    #[tokio::test]
    async fn test_validate_mixed_paths() {
        let validator = FileSystemValidator::new();
        let temp_dir = TempDir::new().unwrap();
        
        // Create a test file
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        
        let paths = vec![
            test_file.to_string_lossy().to_string(),
            "/path/that/does/not/exist".to_string(),
            temp_dir.path().to_string_lossy().to_string(),
        ];
        
        let valid_paths = validator.validate_paths(&paths).await.unwrap();
        assert_eq!(valid_paths.len(), 2); // Only the existing file and directory
    }
    
    #[tokio::test]
    async fn test_is_valid_path() {
        let validator = FileSystemValidator::new();
        let temp_dir = TempDir::new().unwrap();
        
        // Test existing directory
        assert!(validator.is_valid_path(&temp_dir.path().to_string_lossy()).await);
        
        // Test non-existing path
        assert!(!validator.is_valid_path("/path/that/does/not/exist").await);
    }
}
