use anyhow::{Context, Result};
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;
use walkdir::WalkDir;

/// Iterator that yields processed file paths following the correct algorithm order
pub struct PathProcessor {
    input_paths: Vec<String>,
    exclusion_patterns: Vec<String>,
    current_index: usize,
    current_walker: Option<Box<dyn Iterator<Item = walkdir::DirEntry> + Send>>,
    yielded_paths: HashSet<PathBuf>,
}

/// Trait for exclusion pattern matching
#[async_trait]
pub trait ExclusionMatcher: Send + Sync {
    /// Check if a path should be excluded based on the pattern
    fn matches(&self, path: &Path, pattern: &str) -> bool;
    
    /// Get a human-readable description of this matcher strategy
    fn description(&self) -> &'static str;
}

/// Wildcard-based exclusion matcher supporting * and ? patterns
pub struct WildcardMatcher {
    compiled_patterns: Vec<(String, Regex)>,
}

impl WildcardMatcher {
    pub fn new() -> Self {
        Self {
            compiled_patterns: Vec::new(),
        }
    }

    pub fn with_patterns(patterns: &[String]) -> Result<Self> {
        let mut compiled_patterns = Vec::new();
        
        for pattern in patterns {
            let regex_pattern = Self::wildcard_to_regex(pattern);
            let regex = Regex::new(&regex_pattern)
                .with_context(|| format!("Invalid exclusion pattern: {}", pattern))?;
            compiled_patterns.push((pattern.clone(), regex));
        }
        
        Ok(Self { compiled_patterns })
    }

    /// Convert a wildcard pattern to a regex pattern
    fn wildcard_to_regex(pattern: &str) -> String {
        let mut regex = String::new();
        regex.push('^');

        for c in pattern.chars() {
            match c {
                '*' => regex.push_str(".*"),
                '?' => regex.push('.'),
                '.' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '+' | '\\' => {
                    regex.push('\\');
                    regex.push(c);
                }
                c => regex.push(c),
            }
        }

        regex.push('$');
        regex
    }
}

impl Default for WildcardMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExclusionMatcher for WildcardMatcher {
    fn matches(&self, path: &Path, _pattern: &str) -> bool {
        // Normalize path for comparison (handle Windows/Unix differences)
        let path_str = path.to_string_lossy().to_lowercase().replace('\\', "/");
        
        // Check against all compiled patterns
        for (_original, regex) in &self.compiled_patterns {
            if regex.is_match(&path_str) {
                return true;
            }
        }
        
        false
    }

    fn description(&self) -> &'static str {
        "Wildcard pattern matcher (supports * and ? wildcards)"
    }
}

impl PathProcessor {
    /// Create a new path processor with input paths and exclusion patterns
    pub fn new(input_paths: Vec<String>, exclusion_patterns: Vec<String>) -> Result<Self> {
        Ok(Self {
            input_paths,
            exclusion_patterns,
            current_index: 0,
            current_walker: None,
            yielded_paths: HashSet::new(),
        })
    }

    /// Get the exclusion patterns
    pub fn exclusion_patterns(&self) -> &[String] {
        &self.exclusion_patterns
    }

    /// Extract exclusion patterns from input paths (paths starting with '!')
    pub fn extract_exclusion_patterns(paths: &[String]) -> (Vec<String>, Vec<String>) {
        let mut include_paths = Vec::new();
        let mut exclude_patterns = Vec::new();

        for path in paths {
            if let Some(pattern) = path.strip_prefix('!') {
                exclude_patterns.push(pattern.to_string());
            } else {
                include_paths.push(path.clone());
            }
        }

        (include_paths, exclude_patterns)
    }

    /// Convert a path to absolute path, handling both absolute and relative paths
    pub async fn to_absolute_path(path: &str) -> Result<PathBuf> {
        let path_buf = PathBuf::from(path);
        
        if path_buf.is_absolute() {
            Ok(path_buf)
        } else {
            let current_dir = std::env::current_dir()
                .context("Failed to get current directory")?;
            Ok(current_dir.join(path_buf))
        }
    }

    /// Check if a path should be excluded based on exclusion patterns
    fn should_exclude(&self, path: &Path, matcher: &dyn ExclusionMatcher) -> bool {
        for pattern in &self.exclusion_patterns {
            if matcher.matches(path, pattern) {
                return true;
            }
        }
        false
    }

    /// Process all input paths according to the improved algorithm
    /// Returns an iterator-like interface that yields paths one by one
    pub async fn process_paths<F>(&mut self, mut on_path: F, matcher: &dyn ExclusionMatcher) -> Result<Vec<PathBuf>>
    where
        F: FnMut(&PathBuf, ProcessingStatus),
    {
        let mut result_paths = Vec::new();

        for input_path in &self.input_paths.clone() {
            let absolute_path = Self::to_absolute_path(input_path).await?;
            
            // Step 1: Check against exclusion patterns (skip if matches)
            if self.should_exclude(&absolute_path, matcher) {
                on_path(&absolute_path, ProcessingStatus::Excluded);
                continue;
            }

            // Step 2: Validate the path (check if it exists)
            let metadata = match fs::metadata(&absolute_path).await {
                Ok(metadata) => metadata,
                Err(e) => {
                    on_path(&absolute_path, ProcessingStatus::Invalid(e.to_string()));
                    continue;
                }
            };

            // Step 3: Process based on whether it's a directory or file
            if metadata.is_dir() {
                // Step 3.2: If it's a directory, expand it
                self.process_directory(&absolute_path, &mut result_paths, &mut on_path, matcher).await?;
            } else {
                // Step 3.3: If it's a file, add it (if not already added)
                if self.yielded_paths.insert(absolute_path.clone()) {
                    on_path(&absolute_path, ProcessingStatus::Added);
                    result_paths.push(absolute_path);
                }
            }
        }

        Ok(result_paths)
    }

    /// Process a directory recursively using walkdir
    async fn process_directory<F>(
        &mut self,
        dir_path: &Path,
        result_paths: &mut Vec<PathBuf>,
        on_path: &mut F,
        matcher: &dyn ExclusionMatcher,
    ) -> Result<()>
    where
        F: FnMut(&PathBuf, ProcessingStatus),
    {
        // Use walkdir for efficient directory traversal
        for entry in WalkDir::new(dir_path).into_iter() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    eprintln!("Warning: Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path().to_path_buf();
            
            // Skip if it's a directory (we only want files)
            if entry.file_type().is_dir() {
                continue;
            }

            // Apply exclusion patterns to each file
            if self.should_exclude(&path, matcher) {
                on_path(&path, ProcessingStatus::Excluded);
                continue;
            }

            // Add file if not already added
            if self.yielded_paths.insert(path.clone()) {
                on_path(&path, ProcessingStatus::Added);
                result_paths.push(path);
            }
        }

        Ok(())
    }
}

/// Status of path processing for callback reporting
#[derive(Debug, Clone)]
pub enum ProcessingStatus {
    /// Path was added to the result
    Added,
    /// Path was excluded by exclusion patterns
    Excluded,
    /// Path was invalid (doesn't exist or inaccessible)
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_to_absolute_path() {
        // Test absolute path
        let abs_path = if cfg!(windows) {
            r"C:\Windows\System32"
        } else {
            "/usr/bin"
        };
        let result = PathProcessor::to_absolute_path(abs_path).await.unwrap();
        assert_eq!(result, PathBuf::from(abs_path));

        // Test relative path
        let rel_path = "test_file.txt";
        let result = PathProcessor::to_absolute_path(rel_path).await.unwrap();
        assert!(result.is_absolute());
        assert!(result.ends_with("test_file.txt"));
    }

    #[tokio::test]
    async fn test_exclusion_patterns() {
        let (include, exclude) = PathProcessor::extract_exclusion_patterns(&[
            "file1.txt".to_string(),
            "!*.tmp".to_string(),
            "dir/file2.txt".to_string(),
            "!cache/*".to_string(),
        ]);

        assert_eq!(include, vec!["file1.txt", "dir/file2.txt"]);
        assert_eq!(exclude, vec!["*.tmp", "cache/*"]);
    }

    #[tokio::test]
    async fn test_wildcard_matcher() {
        let patterns = vec!["*.tmp".to_string(), "cache/*".to_string()];
        let matcher = WildcardMatcher::with_patterns(&patterns).unwrap();

        assert!(matcher.matches(Path::new("file.tmp"), ""));
        assert!(matcher.matches(Path::new("cache/data.json"), ""));
        assert!(!matcher.matches(Path::new("file.txt"), ""));
    }

    #[tokio::test]
    async fn test_path_processor() {
        // Create temporary test structure
        let temp_dir = TempDir::new().unwrap();
        let test_file1 = temp_dir.path().join("test1.txt");
        let test_file2 = temp_dir.path().join("test2.tmp");
        let sub_dir = temp_dir.path().join("subdir");
        let sub_file = sub_dir.join("test3.txt");

        fs::write(&test_file1, "content1").unwrap();
        fs::write(&test_file2, "content2").unwrap();
        fs::create_dir(&sub_dir).unwrap();
        fs::write(&sub_file, "content3").unwrap();

        let input_paths = vec![
            temp_dir.path().to_string_lossy().to_string(),
            "!*.tmp".to_string(),
        ];

        let (include_paths, exclude_patterns) = PathProcessor::extract_exclusion_patterns(&input_paths);
        let mut processor = PathProcessor::new(include_paths, exclude_patterns).unwrap();
        let matcher = WildcardMatcher::with_patterns(&processor.exclusion_patterns).unwrap();

        let mut statuses = Vec::new();
        let result_paths = processor.process_paths(
            |path, status| {
                statuses.push((path.clone(), status));
            },
            &matcher,
        ).await.unwrap();

        // Should have test1.txt and test3.txt, but not test2.tmp (excluded)
        assert_eq!(result_paths.len(), 2);
        assert!(result_paths.iter().any(|p| p.ends_with("test1.txt")));
        assert!(result_paths.iter().any(|p| p.ends_with("test3.txt")));
        assert!(!result_paths.iter().any(|p| p.ends_with("test2.tmp")));
    }
}
