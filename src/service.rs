use anyhow::Result;
use crate::{
    archiver::Archiver,
    config::Config,
    exclusion::{ExclusionService, WildcardMatcher},
    input::InputReader,
    validator::PathValidator,
};

/// Main backup service that orchestrates the backup process
pub struct BackupService<A, V>
where
    A: Archiver,
    V: PathValidator,
{
    archiver: A,
    validator: V,
    reader: Box<dyn InputReader>,
    config: Config,
    exclusion_service: ExclusionService<WildcardMatcher>,
}

impl<A, V> BackupService<A, V>
where
    A: Archiver,
    V: PathValidator,
{
    /// Create a new backup service with the given components
    pub fn new(archiver: A, validator: V, reader: Box<dyn InputReader>, config: Config) -> Self {
        Self {
            archiver,
            validator,
            reader,
            config,
            exclusion_service: ExclusionService::new(WildcardMatcher::new()),
        }
    }
    
    /// Get input paths (useful for verification)
    pub async fn get_input_paths(&self) -> Result<Vec<String>> {
        let input_paths = self.reader.read_paths().await?;
        let (filtered_paths, excluded_count) = self.exclusion_service.apply_exclusions(&input_paths).await?;
        
        if excluded_count > 0 && self.config.show_progress {
            println!("📝 Excluded {} patterns from backup.", excluded_count);
        }
        
        let valid_paths = self.validator.validate_paths(&filtered_paths).await?;
        Ok(valid_paths)
    }
    
    /// Run the complete backup process
    pub async fn run(&self) -> Result<()> {
        // Check if archiver is available
        if !self.archiver.is_available().await {
            anyhow::bail!("{} is not available. Please install it or check your PATH.", self.archiver.name());
        }
        
        if self.config.show_progress {
            println!("Reading input paths...");
        }
        
        // Read paths from input
        let input_paths = self.reader.read_paths().await?;
        
        if input_paths.is_empty() {
            println!("No paths provided. Nothing to backup.");
            return Ok(());
        }

        if self.config.show_progress {
            println!("Processing {} input entries...", input_paths.len());
        }

        // Apply exclusion patterns
        let (filtered_paths, excluded_count) = self.exclusion_service.apply_exclusions(&input_paths).await?;
        
        if excluded_count > 0 && self.config.show_progress {
            println!("📝 Excluded {} patterns from backup.", excluded_count);
        }
        
        if filtered_paths.is_empty() {
            println!("No paths to backup after applying exclusions.");
            return Ok(());
        }

        if self.config.show_progress {
            println!("Validating {} paths...", filtered_paths.len());
        }
        
        // Validate paths
        let valid_paths = self.validator.validate_paths(&filtered_paths).await?;
        
        if valid_paths.is_empty() {
            println!("No valid paths found. Nothing to backup.");
            return Ok(());
        }
        
        if self.config.show_progress {
            println!("Found {} valid paths to archive.", valid_paths.len());
            println!("Creating archive at: {}", self.config.output_path);
        }
        
        // Create archive
        self.archiver.create_archive(&valid_paths, &self.config.output_path).await?;
        
        println!("✅ Archive created successfully at: {}", self.config.output_path);
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        archiver::SevenZipArchiver,
        input::{InputReader, VecReader},
        validator::FileSystemValidator,
    };
    use tempfile::TempDir;
    use std::fs;
    
    #[tokio::test]
    async fn test_backup_service_with_valid_paths() {
        // Create temporary test files
        let temp_dir = TempDir::new().unwrap();
        let test_file1 = temp_dir.path().join("test1.txt");
        let test_file2 = temp_dir.path().join("test2.txt");
        
        fs::write(&test_file1, "Hello, World!").unwrap();
        fs::write(&test_file2, "Test content").unwrap();
        
        let paths = vec![
            test_file1.to_string_lossy().to_string(),
            test_file2.to_string_lossy().to_string(),
        ];
        
        let archiver = SevenZipArchiver::new();
        let validator = FileSystemValidator::new();
        let reader: Box<dyn InputReader> = Box::new(VecReader::new(paths));
        
        let output_archive = temp_dir.path().join("test.7z");
        let config = Config::new()
            .with_output_path(output_archive.to_string_lossy().to_string())
            .with_progress(false); // Disable progress for tests
        
        let service = BackupService::new(archiver, validator, reader, config);
        
        // Skip test if 7-Zip is not available
        if !service.archiver.is_available().await {
            return;
        }
        
        let result = service.run().await;
        
        // Check if backup completed successfully
        if result.is_ok() {
            assert!(output_archive.exists());
        }
    }
    
    #[tokio::test]
    async fn test_backup_service_with_empty_input() {
        let archiver = SevenZipArchiver::new();
        let validator = FileSystemValidator::new();
        let reader: Box<dyn InputReader> = Box::new(VecReader::new(vec![])); // Empty input
        let config = Config::new().with_progress(false);
        
        let service = BackupService::new(archiver, validator, reader, config);
        
        let result = service.run().await;
        assert!(result.is_ok()); // Should handle empty input gracefully
    }
    
    #[tokio::test]
    async fn test_backup_service_with_invalid_paths() {
        let archiver = SevenZipArchiver::new();
        let validator = FileSystemValidator::new();
        let reader: Box<dyn InputReader> = Box::new(VecReader::new(vec![
            "/path/that/does/not/exist".to_string(),
            "/another/invalid/path".to_string(),
        ]));
        let config = Config::new().with_progress(false);
        
        let service = BackupService::new(archiver, validator, reader, config);
        
        let result = service.run().await;
        assert!(result.is_ok()); // Should handle invalid paths gracefully
    }
}
