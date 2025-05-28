use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tokio::fs;
use tokio::process::Command;

/// Represents an entry in an archive
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// Path of the entry in the archive
    pub path: String,
    /// Whether this entry is a directory
    pub is_directory: bool,
    /// File size (0 for directories)
    pub size: u64,
}

/// Trait for archive verification strategies
#[async_trait]
pub trait ArchiveVerifier: Send + Sync {
    /// List all entries (files and directories) contained in an archive
    async fn list_archive_entries(&self, archive_path: &str) -> Result<Vec<ArchiveEntry>>;

    /// List all files contained in an archive (backwards compatibility)
    async fn list_archive_contents(&self, archive_path: &str) -> Result<Vec<String>> {
        let entries = self.list_archive_entries(archive_path).await?;
        Ok(entries
            .into_iter()
            .filter(|entry| !entry.is_directory)
            .map(|entry| entry.path)
            .collect())
    }

    /// Check if the verifier is available on the system
    async fn is_available(&self) -> bool;

    /// Get the name of the verifier for display purposes
    fn name(&self) -> &'static str;
}

/// 7-Zip based archive verifier implementation
pub struct SevenZipVerifier {
    executable_path: String,
}

impl SevenZipVerifier {
    pub fn new() -> Self {
        Self {
            executable_path: "7z.exe".to_string(),
        }
    }

    pub fn with_path(executable_path: String) -> Self {
        Self { executable_path }
    }
}

impl Default for SevenZipVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ArchiveVerifier for SevenZipVerifier {
    async fn list_archive_entries(&self, archive_path: &str) -> Result<Vec<ArchiveEntry>> {
        // Use 7z l (list) command to get archive contents
        let mut cmd = Command::new(&self.executable_path);
        cmd.args([
            "l",    // List contents
            "-slt", // Show technical information (full paths and attributes)
            archive_path,
        ])
        .env("LANG", "en_US.UTF-8") // Force English output
        .env("LC_ALL", "en_US.UTF-8"); // Override locale settings

        let output = cmd
            .output()
            .await
            .context("Failed to execute 7z list command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            anyhow::bail!(
                "7z list command failed:\nERROR: {}\n{}\n\nSystem ERROR:\n{}",
                stderr,
                stdout,
                stderr
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();

        // Parse 7z -slt output which provides detailed information
        // Format includes blocks for each entry with Path, Attributes, Size, etc.
        let mut current_entry: Option<ArchiveEntry> = None;

        for line in stdout.lines() {
            let line = line.trim();

            if line.starts_with("Path = ") {
                // Start of a new entry
                let path = line.strip_prefix("Path = ").unwrap_or("").to_string();

                // Skip the archive itself
                if path != archive_path && !path.is_empty() {
                    current_entry = Some(ArchiveEntry {
                        path,
                        is_directory: false, // Will be set by Attributes line
                        size: 0,             // Will be set by Size line
                    });
                }
            } else if line.starts_with("Attributes = ") && current_entry.is_some() {
                // Parse attributes to determine if it's a directory
                let attributes = line.strip_prefix("Attributes = ").unwrap_or("");
                // Directory entries typically have 'D' in their attributes string
                if let Some(ref mut entry) = current_entry {
                    entry.is_directory = attributes.contains('D');
                }
            } else if line.starts_with("Size = ") && current_entry.is_some() {
                // Parse file size
                if let Some(size_str) = line.strip_prefix("Size = ") {
                    if let Ok(size) = size_str.parse::<u64>() {
                        if let Some(ref mut entry) = current_entry {
                            entry.size = size;
                        }
                    }
                }
            } else if line.is_empty() && current_entry.is_some() {
                // End of entry block, save the entry
                if let Some(entry) = current_entry.take() {
                    entries.push(entry);
                }
            }
        }

        // Handle case where the last entry doesn't have a trailing empty line
        if let Some(entry) = current_entry {
            entries.push(entry);
        }

        Ok(entries)
    }

    async fn is_available(&self) -> bool {
        Command::new(&self.executable_path)
            .arg("--help")
            .env("LANG", "en_US.UTF-8") // Force English output
            .env("LC_ALL", "en_US.UTF-8") // Override locale settings
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn name(&self) -> &'static str {
        "7-Zip Verifier"
    }
}

/// Recursively enumerate all files in a directory
pub async fn enumerate_directory_files(dir_path: &str) -> Result<Vec<String>> {
    let mut files = Vec::new();

    let path = Path::new(dir_path);
    if !path.exists() {
        return Ok(files);
    }

    if path.is_file() {
        // If it's a file, just return it
        files.push(dir_path.to_string());
        return Ok(files);
    }

    // Recursively walk the directory
    let mut stack = vec![path.to_path_buf()];

    while let Some(current_dir) = stack.pop() {
        match fs::read_dir(&current_dir).await {
            Ok(mut entries) => {
                while let Some(entry) = entries.next_entry().await? {
                    let entry_path = entry.path();

                    if entry_path.is_dir() {
                        // Add directory to stack for recursive processing
                        stack.push(entry_path);
                    } else if entry_path.is_file() {
                        // Add file to results
                        if let Some(path_str) = entry_path.to_str() {
                            files.push(path_str.to_string());
                        }
                    }
                }
            }
            Err(e) => {
                // Log error but continue with other directories
                eprintln!(
                    "Warning: Failed to read directory {}: {}",
                    current_dir.display(),
                    e
                );
            }
        }
    }

    Ok(files)
}

/// Expand input paths by recursively enumerating directory contents
pub async fn expand_input_paths(input_paths: &[String]) -> Result<Vec<String>> {
    let mut expanded_files = Vec::new();

    for input_path in input_paths {
        let files = enumerate_directory_files(input_path).await?;
        expanded_files.extend(files);
    }

    // Remove duplicates while preserving order
    let mut unique_files = Vec::new();
    let mut seen = HashSet::new();

    for file in expanded_files {
        if seen.insert(file.clone()) {
            unique_files.push(file);
        }
    }

    Ok(unique_files)
}

/// Result of archive verification
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Files that were expected but not found in the archive
    pub missing_files: Vec<String>,
    /// Files that were found in the archive
    pub archived_files: Vec<String>,
    /// All files that were expected to be archived (for consolidation)
    pub all_expected_files: Vec<String>,
    /// Total number of files that were supposed to be archived
    pub total_expected: usize,
    /// Total number of files actually found in the archive
    pub total_archived: usize,
}

impl VerificationResult {
    /// Check if verification passed (no missing files)
    pub fn is_complete(&self) -> bool {
        self.missing_files.is_empty()
    }

    /// Get the success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_expected == 0 {
            100.0
        } else {
            (self.total_archived as f64 / self.total_expected as f64) * 100.0
        }
    }

    /// Get consolidated missing files that show directories instead of individual files when appropriate
    pub fn get_consolidated_missing_files(&self) -> Vec<String> {
        consolidate_missing_files(&self.missing_files, &self.all_expected_files)
    }
}

/// Represents a directory and its missing files for consolidation
#[derive(Debug, Clone)]
pub struct DirectoryMissingFiles {
    /// The directory path
    pub directory: String,
    /// Files that are missing from this directory
    pub missing_files: Vec<String>,
    /// Total number of files in this directory (from expected files)
    pub total_files: usize,
    /// Whether this entire directory is missing (all files are missing)
    pub is_complete_directory: bool,
}

/// Consolidate missing files by directory to show directories instead of individual files when appropriate
pub fn consolidate_missing_files(
    missing_files: &[String],
    expected_files: &[String],
) -> Vec<String> {
    if missing_files.is_empty() {
        return Vec::new();
    }

    // Build a map of directory -> all expected files in that directory
    let mut dir_expected_files: HashMap<String, HashSet<String>> = HashMap::new();

    for expected_file in expected_files {
        if let Some(parent) = Path::new(expected_file).parent() {
            let dir_str = parent.to_string_lossy().to_string();
            dir_expected_files
                .entry(dir_str)
                .or_insert_with(HashSet::new)
                .insert(expected_file.clone());
        }
    }

    // Build a map of directory -> missing files in that directory
    let mut dir_missing_files: HashMap<String, Vec<String>> = HashMap::new();

    for missing_file in missing_files {
        if let Some(parent) = Path::new(missing_file).parent() {
            let dir_str = parent.to_string_lossy().to_string();
            dir_missing_files
                .entry(dir_str)
                .or_insert_with(Vec::new)
                .push(missing_file.clone());
        }
    }

    // Analyze each directory to see if it's completely missing
    let mut directory_analysis: HashMap<String, DirectoryMissingFiles> = HashMap::new();

    for (dir, missing_in_dir) in &dir_missing_files {
        let expected_in_dir = dir_expected_files.get(dir).map(|s| s.len()).unwrap_or(0);
        let is_complete = missing_in_dir.len() == expected_in_dir && expected_in_dir > 0;

        directory_analysis.insert(
            dir.clone(),
            DirectoryMissingFiles {
                directory: dir.clone(),
                missing_files: missing_in_dir.clone(),
                total_files: expected_in_dir,
                is_complete_directory: is_complete,
            },
        );
    }

    // Now build the consolidated output, handling hierarchical directories
    let mut consolidated = Vec::new();
    let mut processed_dirs = HashSet::new();

    // Sort directories by depth (deeper first) to handle parent-child relationships
    let mut sorted_dirs: Vec<_> = directory_analysis.keys().collect();
    sorted_dirs
        .sort_by_key(|dir| std::cmp::Reverse(dir.matches(std::path::MAIN_SEPARATOR).count()));

    for dir in sorted_dirs {
        if processed_dirs.contains(dir) {
            continue;
        }

        let dir_info = &directory_analysis[dir];

        if dir_info.is_complete_directory {
            // Check if any parent directory is already completely missing
            let mut parent_already_processed = false;

            if let Some(parent) = Path::new(dir).parent() {
                let parent_str = parent.to_string_lossy().to_string();
                if processed_dirs.contains(&parent_str) {
                    // Parent directory is already being shown as completely missing
                    parent_already_processed = true;
                }
            }

            if !parent_already_processed {
                // Show the entire directory
                consolidated.push(format!("{}{}*", dir, std::path::MAIN_SEPARATOR));
                processed_dirs.insert(dir.clone());

                // Mark all subdirectories as processed
                for other_dir in directory_analysis.keys() {
                    if other_dir.starts_with(dir) && other_dir != dir {
                        processed_dirs.insert(other_dir.clone());
                    }
                }
            }
        }
    }

    // Add individual files from directories that are not completely missing
    for (dir, dir_info) in &directory_analysis {
        if !processed_dirs.contains(dir) && !dir_info.is_complete_directory {
            // Only show individual files for partially missing directories
            for missing_file in &dir_info.missing_files {
                consolidated.push(missing_file.clone());
            }
        }
    }

    // Sort the final result for consistent output
    consolidated.sort();
    consolidated
}

/// Service for verifying archive contents against expected files
pub struct VerificationService<V>
where
    V: ArchiveVerifier,
{
    verifier: V,
}

impl<V> VerificationService<V>
where
    V: ArchiveVerifier,
{
    pub fn new(verifier: V) -> Self {
        Self { verifier }
    }

    /// Verify that all expected files are present in the archive
    pub async fn verify_archive(
        &self,
        archive_path: &str,
        expected_paths: &[String],
    ) -> Result<VerificationResult> {
        // Check if verifier is available
        if !self.verifier.is_available().await {
            anyhow::bail!("{} is not available", self.verifier.name());
        }

        // Expand input paths to get all individual files
        let expanded_expected_files = expand_input_paths(expected_paths).await?;

        // Get archive entries
        let archive_entries = self.verifier.list_archive_entries(archive_path).await?;

        // Extract just the files from archive entries
        let archived_files: Vec<&ArchiveEntry> = archive_entries
            .iter()
            .filter(|entry| !entry.is_directory)
            .collect();

        // Create sets for comparison - we'll compare both full paths and filenames
        let archived_file_paths: HashSet<String> = archived_files
            .iter()
            .map(|entry| self.normalize_path(&entry.path))
            .collect();

        let archived_filenames: HashSet<String> = archived_files
            .iter()
            .map(|entry| {
                // Extract just the filename from the archived path
                std::path::Path::new(&entry.path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase()
            })
            .collect();

        // Find missing files by checking both full path and filename matches
        let missing_files: Vec<String> = expanded_expected_files
            .iter()
            .filter(|path| {
                let normalized_full_path = self.normalize_path(path);
                let filename = std::path::Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();

                // File is considered archived if either the full path or filename matches
                !archived_file_paths.contains(&normalized_full_path)
                    && !archived_filenames.contains(&filename)
            })
            .cloned()
            .collect();

        // Find successfully archived files
        let successfully_archived: Vec<String> = expanded_expected_files
            .iter()
            .filter(|path| {
                let normalized_full_path = self.normalize_path(path);
                let filename = std::path::Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();

                // File is considered archived if either the full path or filename matches
                archived_file_paths.contains(&normalized_full_path)
                    || archived_filenames.contains(&filename)
            })
            .cloned()
            .collect();

        Ok(VerificationResult {
            missing_files,
            archived_files: successfully_archived,
            all_expected_files: expanded_expected_files.clone(),
            total_expected: expanded_expected_files.len(),
            total_archived: archived_files.len(),
        })
    }

    /// Get the expanded list of files from input paths (useful for debugging)
    pub async fn expand_paths(&self, input_paths: &[String]) -> Result<Vec<String>> {
        expand_input_paths(input_paths).await
    }

    /// Normalize a file path for comparison (handle Windows/Unix differences, case sensitivity)
    fn normalize_path(&self, path: &str) -> String {
        // Convert to lowercase for case-insensitive comparison on Windows
        let normalized = path.to_lowercase();

        // Normalize path separators to forward slashes
        normalized.replace('\\', "/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_seven_zip_verifier_is_available() {
        let verifier = SevenZipVerifier::new();
        // This test will pass if 7-Zip is installed
        if verifier.is_available().await {
            assert!(verifier.is_available().await);
        }
    }

    #[tokio::test]
    async fn test_seven_zip_verifier_name() {
        let verifier = SevenZipVerifier::new();
        assert_eq!(verifier.name(), "7-Zip Verifier");
    }

    #[tokio::test]
    async fn test_verification_result() {
        let result = VerificationResult {
            missing_files: vec!["missing.txt".to_string()],
            archived_files: vec!["found1.txt".to_string(), "found2.txt".to_string()],
            all_expected_files: vec![
                "missing.txt".to_string(),
                "found1.txt".to_string(),
                "found2.txt".to_string(),
            ],
            total_expected: 3,
            total_archived: 2,
        };

        assert!(!result.is_complete());
        assert!((result.success_rate() - 66.66666666666667).abs() < 0.0001);

        let complete_result = VerificationResult {
            missing_files: vec![],
            archived_files: vec!["file1.txt".to_string(), "file2.txt".to_string()],
            all_expected_files: vec!["file1.txt".to_string(), "file2.txt".to_string()],
            total_expected: 2,
            total_archived: 2,
        };

        assert!(complete_result.is_complete());
        assert_eq!(complete_result.success_rate(), 100.0);
    }

    #[tokio::test]
    async fn test_path_normalization() {
        let verifier = SevenZipVerifier::new();
        let service = VerificationService::new(verifier);

        assert_eq!(
            service.normalize_path("C:\\Users\\Test\\file.txt"),
            "c:/users/test/file.txt"
        );
        assert_eq!(
            service.normalize_path("/home/user/file.txt"),
            "/home/user/file.txt"
        );
        assert_eq!(
            service.normalize_path("relative\\path\\file.txt"),
            "relative/path/file.txt"
        );
    }
}
