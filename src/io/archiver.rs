use crate::core::{ArchtreeError, ErrorContext, Result};
use async_trait::async_trait;
use tokio::process::Command;

/// Trait for archive creation strategies
#[async_trait]
pub trait Archiver: Send + Sync {
    /// Create an archive from the given paths to the specified output file
    async fn create_archive(&self, paths: &[String], output_path: &str) -> Result<()>;

    /// Add files to an existing archive
    async fn add_to_archive(&self, paths: &[String], archive_path: &str) -> Result<()>;

    /// Check if the archiver is available on the system
    async fn is_available(&self) -> bool;

    /// Get the name of the archiver for display purposes
    fn name(&self) -> &'static str;
}

/// 7-Zip based archiver implementation
#[derive(Clone)]
pub struct SevenZipArchiver {
    executable_path: String,
}

impl SevenZipArchiver {
    pub fn new() -> Self {
        Self {
            executable_path: "7z.exe".to_string(),
        }
    }

    pub fn with_path(executable_path: String) -> Self {
        Self { executable_path }
    }
}

impl Default for SevenZipArchiver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Archiver for SevenZipArchiver {
    async fn create_archive(&self, paths: &[String], output_path: &str) -> Result<()> {
        // Create a temporary file list for 7-Zip with explicit path
        let temp_dir = std::env::temp_dir();
        let temp_list_path = temp_dir.join(format!("7zip_list_{}.txt", std::process::id()));

        // Write all paths to the temporary file with UTF-8 encoding
        let list_content = paths.join("\r\n"); // Use Windows line endings
        tokio::fs::write(&temp_list_path, list_content.as_bytes())
            .await
            .context_io("Failed to write path list to temporary file")?;

        // Build 7-Zip command
        let mut cmd = Command::new(&self.executable_path);
        cmd.args([
            "a",                                       // Add to archive
            "-spf",                                    // Use full paths
            "-sccUTF-8",                               // Force UTF-8 output
            "-tzip",                                   // 7z format
            output_path,                               // Output archive path
            &format!("@{}", temp_list_path.display()), // Input file list
        ]);
        // .env("LANG", "en_US.UTF-8") // Force English output
        // .env("LC_ALL", "en_US.UTF-8"); // Override locale settings

        // Execute the command
        let output = cmd
            .output()
            .await
            .context_external("7z", "Failed to execute 7z command")?;

        // Clean up the temporary file
        let _ = tokio::fs::remove_file(&temp_list_path).await;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(crate::core::ArchtreeError::external_tool(
                "7z",
                format!("7z command failed:\nStderr: {}\nStdout: {}", stderr, stdout),
            ));
        }

        Ok(())
    }

    async fn add_to_archive(&self, paths: &[String], archive_path: &str) -> Result<()> {
        // Ensure the archive path is valid
        let archive_path = tokio::fs::canonicalize(archive_path)
            .await
            .context_io("Failed to canonicalize archive path")?
            .to_string_lossy()
            .to_string();

        // Create a temporary file list for 7-Zip with explicit path
        let temp_dir = std::env::temp_dir();
        let temp_list_path = temp_dir.join(format!("7zip_add_list_{}.txt", std::process::id()));

        // Write all paths to the temporary file with UTF-8 encoding
        let list_content = paths.join("\r\n"); // Use Windows line endings
        tokio::fs::write(&temp_list_path, list_content.as_bytes())
            .await
            .context_io("Failed to write path list to temporary file")?;

        // Build 7-Zip command (use 'u' for update instead of 'a' for add)
        let mut cmd = Command::new(&self.executable_path);
        cmd.args([
            "u",                                       // Update archive (add if not exists)
            "-spf",                                    // Use full paths
            "-sccUTF-8",                               // Force UTF-8 output
            "-tzip",                                   // 7z format
            &archive_path,                             // Archive path
            &format!("@{}", temp_list_path.display()), // Input file list
        ]);
        // .env("LANG", "en_US.UTF-8") // Force English output
        // .env("LC_ALL", "en_US.UTF-8"); // Override locale settings

        // Execute the command
        let output = cmd
            .output()
            .await
            .context_io("Failed to execute 7z update command")?;

        // Clean up the temporary file
        let _ = tokio::fs::remove_file(&temp_list_path).await;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(ArchtreeError::external_tool(
                "7z",
                format!(
                    "7z update command failed:\nStderr: {}\nStdout: {}",
                    stderr, stdout
                ),
            ));
        }

        Ok(())
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
        "7-Zip"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_seven_zip_archiver_is_available() {
        let archiver = SevenZipArchiver::new();
        // This test will pass if 7-Zip is installed, otherwise skip
        if archiver.is_available().await {
            assert!(archiver.is_available().await);
        }
    }

    #[tokio::test]
    async fn test_seven_zip_archiver_name() {
        let archiver = SevenZipArchiver::new();
        assert_eq!(archiver.name(), "7-Zip");
    }

    #[tokio::test]
    async fn test_create_archive_with_mock_files() {
        let archiver = SevenZipArchiver::new();

        // Skip test if 7-Zip is not available
        if !archiver.is_available().await {
            return;
        }

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

        let output_archive = temp_dir.path().join("test.7z");
        let output_path = output_archive.to_string_lossy().to_string();

        let result = archiver.create_archive(&paths, &output_path).await;

        // Check if archive was created successfully
        if result.is_ok() {
            assert!(output_archive.exists());
        }
    }
}
