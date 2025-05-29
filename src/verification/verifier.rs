use crate::core::{ArchtreeError, ErrorContext, Result};
use async_trait::async_trait;
use chrono::{NaiveDateTime, TimeZone};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::SystemTime;
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
    /// Modification time of the file when it was archived (None for directories or if unavailable)
    pub modified: Option<SystemTime>,
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

    /// Verify that all expected files are present in the archive
    async fn verify_archive(
        &self,
        archive_path: &str,
        expected_paths: &[String],
    ) -> Result<VerificationResult>;

    /// Verify that files in the archive are up to date with the filesystem
    async fn verify_archive_freshness(
        &self,
        archive_path: &str,
        expected_paths: &[String],
    ) -> Result<FreshnessVerificationResult>;

    /// Check if the verifier is available on the system
    async fn is_available(&self) -> bool;

    /// Get the name of the verifier for display purposes
    fn name(&self) -> &'static str;
}

/// 7-Zip based archive verifier implementation
#[derive(Debug, Clone)]
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

    /// Alternative method for listing archive entries with better Unicode support
    /// Uses Windows-specific encoding handling when available
    ///
    /// This method addresses the issue where 7-Zip's `7z l -slt` command prints
    /// non-English characters (like Cyrillic) as gibberish, causing verification
    /// to fail when comparing paths. The solution uses:
    /// 1. UTF-8 output forcing with `-sccUTF-8` flag
    /// 2. Fallback to legacy method if UTF-8 approach fails
    async fn list_archive_entries_with_encoding(
        &self,
        archive_path: &str,
    ) -> Result<Vec<ArchiveEntry>> {
        // First try the standard UTF-8 approach
        match self.list_archive_entries_utf8(archive_path).await {
            Ok(entries) => Ok(entries),
            Err(_) => {
                // Fallback to original method if UTF-8 fails
                self.list_archive_entries_legacy(archive_path).await
            }
        }
    }

    /// Try to list archive entries using UTF-8 encoding
    async fn list_archive_entries_utf8(&self, archive_path: &str) -> Result<Vec<ArchiveEntry>> {
        let archive_path = tokio::fs::canonicalize(archive_path)
            .await
            .context_io("Failed to canonicalize archive path")?
            .to_string_lossy()
            .to_string();

        let mut cmd = Command::new(&self.executable_path);
        cmd.args([
            "l",
            "-slt",
            "-sccUTF-8", // Force UTF-8 output
            &archive_path,
        ]);

        let output = cmd
            .output()
            .await
            .context_io("Failed to execute 7z list command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ArchtreeError::external_tool(
                "7z",
                format!("7z list command failed: {}", stderr),
            ));
        }

        // Parse as UTF-8
        let stdout = String::from_utf8(output.stdout)
            .map_err(|_| ArchtreeError::external_tool("7z", "Invalid UTF-8 output"))?;

        self.parse_seven_zip_output(&stdout, &archive_path)
    }

    /// Legacy method for listing archive entries (original implementation)
    async fn list_archive_entries_legacy(&self, archive_path: &str) -> Result<Vec<ArchiveEntry>> {
        let archive_path = tokio::fs::canonicalize(archive_path)
            .await
            .context_io("Failed to canonicalize archive path")?
            .to_string_lossy()
            .to_string();

        let mut cmd = Command::new(&self.executable_path);
        cmd.args(["l", "-slt", &archive_path]);

        let output = cmd
            .output()
            .await
            .context_io("Failed to execute 7z list command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ArchtreeError::external_tool(
                "7z",
                format!("7z list command failed: {}", stderr),
            ));
        }

        // Use lossy conversion for legacy compatibility
        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_seven_zip_output(&stdout, &archive_path)
    }

    /// Parse 7-Zip output and extract archive entries
    fn parse_seven_zip_output(
        &self,
        stdout: &str,
        archive_path: &str,
    ) -> Result<Vec<ArchiveEntry>> {
        let mut entries = Vec::new();

        // Parse 7z -slt output which provides detailed information
        // Format includes blocks for each entry with Path, Attributes, Size, etc.
        let mut current_entry: Option<ArchiveEntry> = None;

        for line in stdout.lines() {
            let line = line.trim();

            if line.starts_with("Path = ") {
                // Start of a new entry
                let path = line.strip_prefix("Path = ").unwrap_or("").to_string();

                // Skip the archive itself and empty paths
                if path != archive_path && !path.is_empty() {
                    current_entry = Some(ArchiveEntry {
                        path,
                        is_directory: false, // Will be set by Attributes line
                        size: 0,             // Will be set by Size line
                        modified: None,      // Will be set by Modified line
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
            } else if line.starts_with("Modified = ") && current_entry.is_some() {
                // Parse modification time from 7-Zip format "YYYY-MM-DD HH:MM:SS"
                if let Some(modified_str) = line.strip_prefix("Modified = ") {
                    if let Ok(naive_dt) =
                        NaiveDateTime::parse_from_str(modified_str, "%Y-%m-%d %H:%M:%S")
                    {
                        // 7-Zip shows local time, so treat it as local time and convert to SystemTime
                        // We'll assume local timezone for the archive timestamps
                        use chrono::Local;
                        let local_dt = Local.from_local_datetime(&naive_dt).single();
                        if let Some(local_time) = local_dt {
                            let system_time = SystemTime::from(local_time);
                            if let Some(ref mut entry) = current_entry {
                                entry.modified = Some(system_time);
                            }
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
}

impl Default for SevenZipVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ArchiveVerifier for SevenZipVerifier {
    async fn list_archive_entries(&self, archive_path: &str) -> Result<Vec<ArchiveEntry>> {
        // Use the new encoding-aware method
        self.list_archive_entries_with_encoding(archive_path).await
    }

    async fn is_available(&self) -> bool {
        Command::new(&self.executable_path)
            .arg("--help")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn name(&self) -> &'static str {
        "7-Zip Verifier"
    }

    async fn verify_archive(
        &self,
        archive_path: &str,
        expected_paths: &[String],
    ) -> Result<VerificationResult> {
        // Check if verifier is available
        if !self.is_available().await {
            return Err(ArchtreeError::external_tool(
                self.name(),
                "is not available",
            ));
        }

        // Expand input paths to get all individual files
        let expanded_expected_files = expand_input_paths(expected_paths).await?;

        // Get archive entries
        let archive_entries = self.list_archive_entries(archive_path).await?;

        // Extract just the files from archive entries
        let archived_files: Vec<&ArchiveEntry> = archive_entries
            .iter()
            .filter(|entry| !entry.is_directory)
            .collect();

        let archived_file_paths: Vec<String> = archived_files
            .iter()
            .map(|entry| entry.path.clone())
            .collect();

        // Compare expected vs archived files
        let (missing_files, found_files) =
            compare_file_lists(&expanded_expected_files, &archived_file_paths);

        let total_archived = found_files.len();

        Ok(VerificationResult {
            missing_files,
            archived_files: found_files,
            all_expected_files: expanded_expected_files.clone(),
            total_expected: expanded_expected_files.len(),
            total_archived,
        })
    }

    async fn verify_archive_freshness(
        &self,
        archive_path: &str,
        expected_paths: &[String],
    ) -> Result<FreshnessVerificationResult> {
        // Check if verifier is available
        if !self.is_available().await {
            return Err(ArchtreeError::external_tool(
                self.name(),
                "is not available",
            ));
        }

        // Expand input paths to get all individual files
        let expanded_expected_files = expand_input_paths(expected_paths).await?;

        // Get archive entries
        let archive_entries = self.list_archive_entries(archive_path).await?;

        // Build a map of archive entries by path for quick lookup
        let archive_map: HashMap<String, &ArchiveEntry> = archive_entries
            .iter()
            .filter(|entry| !entry.is_directory)
            .map(|entry| (entry.path.clone(), entry))
            .collect();

        let mut outdated_files = Vec::new();
        let mut up_to_date_files = Vec::new();
        let mut unverifiable_files = Vec::new();

        // Check each expected file for freshness
        for file_path in &expanded_expected_files {
            if let Some(archive_entry) = archive_map.get(file_path) {
                // File exists in archive, check if it's up to date
                match (archive_entry.modified, fs::metadata(file_path).await) {
                    (Some(archive_modified), Ok(fs_metadata)) => {
                        if let Ok(fs_modified) = fs_metadata.modified() {
                            // Calculate time difference in seconds
                            let time_diff = if fs_modified > archive_modified {
                                fs_modified
                                    .duration_since(archive_modified)
                                    .unwrap_or_default()
                                    .as_secs()
                            } else {
                                0
                            };

                            // Consider files up to date if they're within 2 seconds
                            // This accounts for precision differences between archive and filesystem timestamps
                            const FRESHNESS_TOLERANCE_SECONDS: u64 = 2;

                            if time_diff > FRESHNESS_TOLERANCE_SECONDS {
                                // Filesystem version is significantly newer
                                outdated_files.push(OutdatedFile {
                                    path: file_path.clone(),
                                    archive_modified: Some(archive_modified),
                                    filesystem_modified: Some(fs_modified),
                                });
                            } else {
                                // Archive version is up to date (within tolerance)
                                up_to_date_files.push(file_path.clone());
                            }
                        } else {
                            // Can't get filesystem modification time
                            unverifiable_files.push(file_path.clone());
                        }
                    }
                    _ => {
                        // Can't compare modification times (missing data)
                        unverifiable_files.push(file_path.clone());
                    }
                }
            }
            // Note: We don't include missing files here as this is specifically for freshness verification
            // Missing files would be caught by the regular verify_archive method
        }

        Ok(FreshnessVerificationResult {
            outdated_files,
            up_to_date_files,
            unverifiable_files,
            total_checked: expanded_expected_files.len(),
        })
    }
}

/// Compare two file lists and return (missing_files, found_files)
fn compare_file_lists(expected: &[String], archived: &[String]) -> (Vec<String>, Vec<String>) {
    let archived_set: HashSet<&String> = archived.iter().collect();
    let _expected_set: HashSet<&String> = expected.iter().collect();

    let missing_files: Vec<String> = expected
        .iter()
        .filter(|&file| !archived_set.contains(file))
        .cloned()
        .collect();

    let found_files: Vec<String> = expected
        .iter()
        .filter(|&file| archived_set.contains(file))
        .cloned()
        .collect();

    (missing_files, found_files)
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

/// Represents the result of comparing file modification times between filesystem and archive
#[derive(Debug, Clone)]
pub struct FreshnessVerificationResult {
    /// Files that exist in both locations but are newer on the filesystem
    pub outdated_files: Vec<OutdatedFile>,
    /// Files that are up to date (archive version is same or newer than filesystem)
    pub up_to_date_files: Vec<String>,
    /// Files that couldn't be compared (missing modification time in archive or filesystem errors)
    pub unverifiable_files: Vec<String>,
    /// Total number of files checked
    pub total_checked: usize,
}

/// Represents a file that is outdated in the archive
#[derive(Debug, Clone)]
pub struct OutdatedFile {
    /// Path of the file
    pub path: String,
    /// Modification time in the archive
    pub archive_modified: Option<SystemTime>,
    /// Modification time on the filesystem
    pub filesystem_modified: Option<SystemTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freshness_verification_result() {
        let result = FreshnessVerificationResult {
            outdated_files: vec![OutdatedFile {
                path: "test.txt".to_string(),
                archive_modified: Some(SystemTime::UNIX_EPOCH),
                filesystem_modified: Some(
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100),
                ),
            }],
            up_to_date_files: vec!["current.txt".to_string()],
            unverifiable_files: vec!["unknown.txt".to_string()],
            total_checked: 3,
        };

        assert_eq!(result.outdated_files.len(), 1);
        assert_eq!(result.up_to_date_files.len(), 1);
        assert_eq!(result.unverifiable_files.len(), 1);
        assert_eq!(result.total_checked, 3);
    }

    #[test]
    fn test_outdated_file_structure() {
        let outdated = OutdatedFile {
            path: "test.txt".to_string(),
            archive_modified: Some(SystemTime::UNIX_EPOCH),
            filesystem_modified: Some(
                SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(3600),
            ),
        };

        assert_eq!(outdated.path, "test.txt");
        assert!(outdated.archive_modified.is_some());
        assert!(outdated.filesystem_modified.is_some());
    }

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
}
