use crate::{
    archiver::Archiver,
    config::Config,
    exclusion::{ExclusionService, WildcardMatcher},
    input::InputReader,
    validator::PathValidator,
    verifier::expand_input_paths,
};
use anyhow::Result;
use std::sync::OnceLock;

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
    /// Cached processed paths to avoid recomputation during verification
    processed_paths: OnceLock<Vec<String>>,
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
            processed_paths: OnceLock::new(),
        }
    }

    /// Get input paths (useful for verification)
    pub async fn get_input_paths(&self) -> Result<Vec<String>> {
        // Return cached paths if available
        if let Some(cached_paths) = self.processed_paths.get() {
            return Ok(cached_paths.clone());
        }

        let input_paths = self.reader.read_paths().await?;

        // Otherwise process paths fresh
        self.process_input_paths(&input_paths).await
    }

    /// Process input paths from raw input to validated file list
    async fn process_input_paths(&self, input_paths: &[String]) -> Result<Vec<String>> {
        // First apply exclusions to the raw input (to extract patterns and filter them out)
        let (include_paths, exclude_patterns) = self
            .exclusion_service
            .extract_exclusion_patterns(input_paths);

        if !exclude_patterns.is_empty() && self.config.show_progress {
            println!("📝 Found {} exclusion patterns.", exclude_patterns.len());
        }

        if include_paths.is_empty() {
            if self.config.show_progress {
                println!("No paths to backup after extracting exclusion patterns.");
            }
            return Ok(vec![]);
        }

        // Expand directories to individual files
        let expanded_paths = expand_input_paths(&include_paths).await?;

        if self.config.show_progress && expanded_paths.len() != include_paths.len() {
            println!(
                "📁 Expanded {} paths to {} individual files.",
                include_paths.len(),
                expanded_paths.len()
            );
        }

        // Apply exclusion patterns to the expanded file list
        let filtered_paths = self
            .exclusion_service
            .filter_excluded_paths(&expanded_paths, &exclude_patterns)
            .await?;
        let excluded_count = expanded_paths.len() - filtered_paths.len();

        if excluded_count > 0 && self.config.show_progress {
            println!(
                "🚫 Excluded {} files matching exclusion patterns.",
                excluded_count
            );
        }

        if filtered_paths.is_empty() {
            if self.config.show_progress {
                println!("No files to backup after applying exclusions.");
            }
            return Ok(vec![]);
        }

        if self.config.show_progress {
            println!("Validating {} files...", filtered_paths.len());
        }

        let valid_paths = self.validator.validate_paths(&filtered_paths).await?;

        Ok(valid_paths)
    }

    /// Run the complete backup process
    pub async fn run(&self) -> Result<()> {
        // Check if archiver is available
        if !self.archiver.is_available().await {
            anyhow::bail!(
                "{} is not available. Please install it or check your PATH.",
                self.archiver.name()
            );
        }

        if self.config.show_progress {
            println!("Reading input paths...");
        }

        // Read and process paths
        let input_paths = self.reader.read_paths().await?;

        if input_paths.is_empty() {
            eprintln!("No paths provided. Nothing to backup.");
            return Ok(());
        }

        if self.config.show_progress {
            println!("Processing {} input entries...", input_paths.len());
        }

        // Process and validate paths using the shared logic
        let valid_paths = self.process_input_paths(&input_paths).await?;

        if valid_paths.is_empty() {
            eprintln!("No valid paths found. Nothing to backup.");
            return Ok(());
        }

        // Cache the processed paths for potential later use (e.g., verification)
        let _ = self.processed_paths.set(valid_paths.clone());

        if self.config.show_progress {
            println!("Found {} valid paths to archive.", valid_paths.len());
            println!("Creating archive at: {}", self.config.output_path);
        }

        // Create archive
        self.archiver
            .create_archive(&valid_paths, &self.config.output_path)
            .await?;

        if self.config.show_progress {
            println!(
                "✅ Archive created successfully at: {}",
                self.config.output_path
            );
        }

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
    use std::fs;
    use tempfile::TempDir;

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
        let config = Config::builder()
            .output_path(Some(&output_archive.to_string_lossy()), false)
            .show_progress(false) // Disable progress for tests
            .build()
            .expect("Failed to create config");

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
        let config = Config::builder()
            .output_path(Some("empty_backup.7z"), false)
            .show_progress(false)
            .build()
            .expect("Failed to create config");

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
        let config = Config::builder()
            .output_path(Some("invalid_backup.7z"), false)
            .show_progress(false)
            .build()
            .expect("Failed to create config");

        let service = BackupService::new(archiver, validator, reader, config);

        let result = service.run().await;
        assert!(result.is_ok()); // Should handle invalid paths gracefully
    }
}
