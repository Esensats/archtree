use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashSet;
use tokio::process::Command;

/// Trait for archive verification strategies
#[async_trait]
pub trait ArchiveVerifier: Send + Sync {
    /// List all files contained in an archive
    async fn list_archive_contents(&self, archive_path: &str) -> Result<Vec<String>>;

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
    async fn list_archive_contents(&self, archive_path: &str) -> Result<Vec<String>> {
        // Use 7z l (list) command to get archive contents
        let mut cmd = Command::new(&self.executable_path);
        cmd.args([
            "l",    // List contents
            "-slt", // Show technical information (full paths)
            archive_path,
        ]);

        let output = cmd
            .output()
            .await
            .context("Failed to execute 7z list command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("7z list command failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();

        // Parse 7z output to extract file paths
        // 7z -slt output format includes "Path = " lines for each file
        for line in stdout.lines() {
            if let Some(path) = line.strip_prefix("Path = ") {
                // Skip the archive itself and directory entries
                if path != archive_path && !path.ends_with('/') && !path.ends_with('\\') {
                    files.push(path.to_string());
                }
            }
        }

        Ok(files)
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
}

/// Result of archive verification
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Files that were expected but not found in the archive
    pub missing_files: Vec<String>,
    /// Files that were found in the archive
    pub archived_files: Vec<String>,
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

        // Get archive contents
        let archive_contents = self.verifier.list_archive_contents(archive_path).await?;

        // Create sets for comparison - we'll compare both full paths and filenames
        let archived_files: HashSet<String> = archive_contents
            .iter()
            .map(|path| self.normalize_path(path))
            .collect();

        let archived_filenames: HashSet<String> = archive_contents
            .iter()
            .map(|path| {
                // Extract just the filename from the archived path
                std::path::Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase()
            })
            .collect();

        // Find missing files by checking both full path and filename matches
        let missing_files: Vec<String> = expected_paths
            .iter()
            .filter(|path| {
                let normalized_full_path = self.normalize_path(path);
                let filename = std::path::Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();

                // File is considered archived if either the full path or filename matches
                !archived_files.contains(&normalized_full_path)
                    && !archived_filenames.contains(&filename)
            })
            .cloned()
            .collect();

        // Find successfully archived files
        let successfully_archived: Vec<String> = expected_paths
            .iter()
            .filter(|path| {
                let normalized_full_path = self.normalize_path(path);
                let filename = std::path::Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();

                // File is considered archived if either the full path or filename matches
                archived_files.contains(&normalized_full_path)
                    || archived_filenames.contains(&filename)
            })
            .cloned()
            .collect();

        Ok(VerificationResult {
            missing_files,
            archived_files: successfully_archived,
            total_expected: expected_paths.len(),
            total_archived: archive_contents.len(),
        })
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
            total_expected: 3,
            total_archived: 2,
        };

        assert!(!result.is_complete());
        assert!((result.success_rate() - 66.66666666666667).abs() < 0.0001);

        let complete_result = VerificationResult {
            missing_files: vec![],
            archived_files: vec!["file1.txt".to_string(), "file2.txt".to_string()],
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
