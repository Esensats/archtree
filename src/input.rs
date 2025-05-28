use anyhow::{Context, Result};
use async_trait::async_trait;
use std::io::{self, BufRead};

/// Trait for reading input paths
#[async_trait]
pub trait InputReader: Send + Sync {
    /// Read paths from the input source
    async fn read_paths(&self) -> Result<Vec<String>>;
}

/// Reader that reads from standard input
pub struct StdinReader;

impl StdinReader {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StdinReader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputReader for StdinReader {
    async fn read_paths(&self) -> Result<Vec<String>> {
        let stdin = io::stdin();
        let mut paths = Vec::new();

        for line in stdin.lock().lines() {
            let line = line.context("Failed to read line from stdin")?;
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                paths.push(trimmed.to_string());
            }
        }

        Ok(paths)
    }
}

/// Reader that reads from a file
pub struct FileReader {
    file_path: String,
}

impl FileReader {
    pub fn new(file_path: &str) -> Self {
        Self {
            file_path: file_path.to_string(),
        }
    }
}

#[async_trait]
impl InputReader for FileReader {
    async fn read_paths(&self) -> Result<Vec<String>> {
        let content = tokio::fs::read_to_string(&self.file_path)
            .await
            .context(format!("Failed to read file: {}", self.file_path))?;

        let paths = content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(paths)
    }
}

/// Reader that takes paths from a vector (useful for testing)
pub struct VecReader {
    paths: Vec<String>,
}

impl VecReader {
    pub fn new(paths: Vec<String>) -> Self {
        Self { paths }
    }
}

#[async_trait]
impl InputReader for VecReader {
    async fn read_paths(&self) -> Result<Vec<String>> {
        Ok(self.paths.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_file_reader() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "C:\\path\\one").unwrap();
        writeln!(temp_file, "C:\\path\\two").unwrap();
        writeln!(temp_file, "").unwrap(); // Empty line should be filtered
        writeln!(temp_file, "  C:\\path\\three  ").unwrap(); // Should be trimmed

        let reader = FileReader::new(&temp_file.path().to_string_lossy());
        let paths = reader.read_paths().await.unwrap();

        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], "C:\\path\\one");
        assert_eq!(paths[1], "C:\\path\\two");
        assert_eq!(paths[2], "C:\\path\\three");
    }

    #[tokio::test]
    async fn test_vec_reader() {
        let input_paths = vec![
            "C:\\Users\\test\\Documents".to_string(),
            "D:\\Projects".to_string(),
        ];

        let reader = VecReader::new(input_paths.clone());
        let paths = reader.read_paths().await.unwrap();

        assert_eq!(paths, input_paths);
    }
}
