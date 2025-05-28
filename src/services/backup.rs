use crate::core::{Config, Result, ArchtreeError, ErrorContext};
use crate::io::{Archiver, InputReader};
use crate::processing::{PathProcessor, ProcessingStatus, WildcardMatcher};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Backup service using the improved path processing algorithm
pub struct BackupService<A>
where
    A: Archiver,
{
    archiver: A,
    reader: Box<dyn InputReader>,
    config: Config,
    /// Cached processed paths to avoid recomputation during verification
    processed_paths: OnceLock<Vec<PathBuf>>,
}

impl<A> BackupService<A>
where
    A: Archiver,
{
    /// Create a new backup service with the given components
    pub fn new(archiver: A, reader: Box<dyn InputReader>, config: Config) -> Self {
        Self {
            archiver,
            reader,
            config,
            processed_paths: OnceLock::new(),
        }
    }

    /// Get processed paths as strings (for verification compatibility)
    pub async fn get_input_paths(&self) -> Result<Vec<String>> {
        if let Some(cached_paths) = self.processed_paths.get() {
            return Ok(cached_paths.iter().map(|p| p.to_string_lossy().to_string()).collect());
        }

        let processed_paths = self.process_input_paths().await?;
        let string_paths = processed_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let _ = self.processed_paths.set(processed_paths);
        Ok(string_paths)
    }

    /// Process input paths using the improved algorithm
    async fn process_input_paths(&self) -> Result<Vec<PathBuf>> {
        let input_paths = self.reader.read_paths().await
            .context_io("Failed to read input paths")?;

        if input_paths.is_empty() {
            return Err(ArchtreeError::config("No input paths provided"));
        }

        // Extract exclusion patterns from input
        let (include_paths, exclude_patterns) = PathProcessor::extract_exclusion_patterns(&input_paths);

        if !exclude_patterns.is_empty() && self.config.show_progress {
            println!("Found {} exclusion patterns:", exclude_patterns.len());
            for pattern in &exclude_patterns {
                println!("  🚫 {}", pattern);
            }
        }

        if include_paths.is_empty() {
            return Err(ArchtreeError::config("No include paths found after filtering exclusions"));
        }

        // Create path processor and matcher
        let mut processor = PathProcessor::new(include_paths, exclude_patterns)
            .context_config("Failed to create path processor")?;
        let matcher = WildcardMatcher::with_patterns(processor.exclusion_patterns())
            .context_config("Failed to create wildcard matcher")?;

        // Track statistics for reporting
        let mut added_count = 0;
        let mut excluded_count = 0;
        let mut invalid_count = 0;

        // Process paths using the improved algorithm
        let processed_paths = processor
            .process_paths(
                |path, status| match status {
                    ProcessingStatus::Added => {
                        added_count += 1;
                        if self.config.show_progress {
                            println!("✓ {}", path.display());
                        }
                    }
                    ProcessingStatus::Excluded => {
                        excluded_count += 1;
                        if self.config.show_progress {
                            println!("🚫 Excluded: {}", path.display());
                        }
                    }
                    ProcessingStatus::Invalid(ref error) => {
                        invalid_count += 1;
                        if self.config.show_progress {
                            eprintln!("⚠️  Invalid path: {} ({})", path.display(), error);
                        }
                    }
                },
                &matcher,
            )
            .await
            .context_config("Failed to process paths")?;

        // Report final statistics
        if self.config.show_progress {
            println!("\n📊 Processing Summary:");
            println!("  ✓ Added: {} files", added_count);
            if excluded_count > 0 {
                println!("  🚫 Excluded: {} files", excluded_count);
            }
            if invalid_count > 0 {
                println!("  ⚠️  Invalid: {} paths", invalid_count);
            }
            println!("  📁 Total for archive: {} files", processed_paths.len());
        }

        Ok(processed_paths)
    }

    /// Run the complete backup process
    pub async fn run(&self) -> Result<()> {
        // Check if archiver is available
        if !self.archiver.is_available().await {
            return Err(ArchtreeError::external_tool(
                self.archiver.name(),
                format!("{} is not available on this system", self.archiver.name())
            ));
        }

        if self.config.show_progress {
            println!("🚀 Starting backup process...");
        }

        // Process paths using the new algorithm
        let processed_paths = self.process_input_paths().await?;

        if processed_paths.is_empty() {
            return Err(ArchtreeError::config("No valid paths found to archive"));
        }

        // Cache the processed paths for potential later use (e.g., verification)
        let _ = self.processed_paths.set(processed_paths.clone());

        if self.config.show_progress {
            println!("\n📦 Creating archive: {}", self.config.output_path);
        }

        // Convert paths to strings for archiver compatibility
        let string_paths: Vec<String> = processed_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        // Create archive
        self.archiver
            .create_archive(&string_paths, &self.config.output_path)
            .await
            .context_io("Failed to create archive")?;

        if self.config.show_progress {
            println!("✅ Archive created successfully: {}", self.config.output_path);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{SevenZipArchiver, VecReader};
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_backup_service_with_valid_paths() {
        // Create temporary test files
        let temp_dir = TempDir::new().unwrap();
        let test_file1 = temp_dir.path().join("test1.txt");
        let test_file2 = temp_dir.path().join("test2.txt");
        let test_file3 = temp_dir.path().join("test3.tmp");

        fs::write(&test_file1, "Hello, World!").unwrap();
        fs::write(&test_file2, "Test content").unwrap();
        fs::write(&test_file3, "Temp file").unwrap();

        let paths = vec![
            test_file1.to_string_lossy().to_string(),
            test_file2.to_string_lossy().to_string(),
            test_file3.to_string_lossy().to_string(),
            "!*.tmp".to_string(),
        ];

        let archiver = SevenZipArchiver::new();
        let reader = Box::new(VecReader::new(paths));
        let config = Config::builder()
            .output_path(Some("test.7z"), false)
            .show_progress(false)
            .build()
            .unwrap();

        let service = BackupService::new(archiver, reader, config);
        let input_paths = service.get_input_paths().await.unwrap();

        // Should have test1.txt and test2.txt, but not test3.tmp (excluded)
        assert_eq!(input_paths.len(), 2);
        assert!(input_paths.iter().any(|p| p.contains("test1.txt")));
        assert!(input_paths.iter().any(|p| p.contains("test2.txt")));
        assert!(!input_paths.iter().any(|p| p.contains("test3.tmp")));
    }

    #[tokio::test]
    async fn test_relative_path_conversion() {
        // Create temporary test structure
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("relative_test.txt");
        fs::write(&test_file, "Content").unwrap();

        // Change to temp directory and use relative path
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let paths = vec!["relative_test.txt".to_string()];
        let archiver = SevenZipArchiver::new();
        let reader = Box::new(VecReader::new(paths));
        let config = Config::builder()
            .output_path(Some("test.7z"), false)
            .show_progress(false)
            .build()
            .unwrap();

        let service = BackupService::new(archiver, reader, config);
        let input_paths = service.get_input_paths().await.unwrap();

        // Should have absolute path
        assert_eq!(input_paths.len(), 1);
        assert!(input_paths[0].contains("relative_test.txt"));
        assert!(PathBuf::from(&input_paths[0]).is_absolute());

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }
}
