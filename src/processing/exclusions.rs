use crate::core::{ArchtreeError, Result};
use async_trait::async_trait;
use regex::Regex;
use std::path::Path;

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
            let regex = Regex::new(&regex_pattern).map_err(|e| {
                ArchtreeError::path_processing_with_source(
                    format!("Invalid exclusion pattern: {}", pattern),
                    Some(pattern.clone()),
                    e,
                )
            })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wildcard_matcher() {
        let patterns = vec!["*.tmp".to_string(), "cache/*".to_string()];
        let matcher = WildcardMatcher::with_patterns(&patterns).unwrap();

        assert!(matcher.matches(Path::new("file.tmp"), ""));
        assert!(matcher.matches(Path::new("cache/data.json"), ""));
        assert!(!matcher.matches(Path::new("file.txt"), ""));
    }

    #[test]
    fn test_wildcard_to_regex() {
        assert_eq!(WildcardMatcher::wildcard_to_regex("*.txt"), "^.*\\.txt$");
        assert_eq!(
            WildcardMatcher::wildcard_to_regex("test?.log"),
            "^test.\\.log$"
        );
        assert_eq!(WildcardMatcher::wildcard_to_regex("cache/*"), "^cache/.*$");
    }
}
