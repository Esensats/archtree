use anyhow::Result;
use async_trait::async_trait;

/// Trait for exclusion pattern matching strategies
#[async_trait]
pub trait ExclusionMatcher: Send + Sync {
    /// Check if a path should be excluded based on the pattern
    async fn matches(&self, path: &str, pattern: &str) -> bool;

    /// Get a human-readable description of this matcher strategy
    fn description(&self) -> &'static str;
}

/// Simple wildcard-based exclusion matcher
/// Supports basic wildcards like * and ?
pub struct WildcardMatcher;

impl WildcardMatcher {
    pub fn new() -> Self {
        Self
    }

    /// Convert a wildcard pattern to a regex pattern
    fn wildcard_to_regex(&self, pattern: &str) -> String {
        let mut regex = String::new();
        regex.push('^');

        let chars: Vec<char> = pattern.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            match chars[i] {
                '*' => regex.push_str(".*"),
                '?' => regex.push('.'),
                '.' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '+' | '\\' => {
                    regex.push('\\');
                    regex.push(chars[i]);
                }
                c => regex.push(c),
            }
            i += 1;
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
    async fn matches(&self, path: &str, pattern: &str) -> bool {
        // Normalize paths for comparison (handle Windows/Unix differences)
        let normalized_path = path.to_lowercase().replace('\\', "/");
        let normalized_pattern = pattern.to_lowercase().replace('\\', "/");

        // Convert wildcard pattern to regex
        let regex_pattern = self.wildcard_to_regex(&normalized_pattern);

        // Use simple pattern matching for now (could use regex crate for more complex patterns)
        if let Ok(regex) = regex::Regex::new(&regex_pattern) {
            regex.is_match(&normalized_path)
        } else {
            // Fallback to simple string matching if regex fails
            normalized_path.contains(&normalized_pattern)
        }
    }

    fn description(&self) -> &'static str {
        "Wildcard pattern matcher (supports * and ? wildcards)"
    }
}

/// Future: GitIgnore-style pattern matcher
/// This will support more advanced patterns like .gitignore files
pub struct GitIgnoreMatcher;

impl GitIgnoreMatcher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitIgnoreMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExclusionMatcher for GitIgnoreMatcher {
    async fn matches(&self, _path: &str, _pattern: &str) -> bool {
        // TODO: Implement gitignore-style pattern matching
        // This could use the `ignore` crate for full gitignore compatibility
        false
    }

    fn description(&self) -> &'static str {
        "GitIgnore-style pattern matcher (future enhancement)"
    }
}

/// Service for managing exclusion patterns and applying them to paths
pub struct ExclusionService<M>
where
    M: ExclusionMatcher,
{
    matcher: M,
}

impl<M> ExclusionService<M>
where
    M: ExclusionMatcher,
{
    pub fn new(matcher: M) -> Self {
        Self { matcher }
    }

    /// Extract exclusion patterns from input paths (paths starting with '!')
    pub fn extract_exclusion_patterns(&self, paths: &[String]) -> (Vec<String>, Vec<String>) {
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

    /// Filter out paths that match any exclusion pattern
    pub async fn filter_excluded_paths(
        &self,
        paths: &[String],
        exclude_patterns: &[String],
    ) -> Result<Vec<String>> {
        let mut filtered_paths = Vec::new();

        for path in paths {
            let mut should_exclude = false;

            for pattern in exclude_patterns {
                if self.matcher.matches(path, pattern).await {
                    should_exclude = true;
                    break;
                }
            }

            if !should_exclude {
                filtered_paths.push(path.clone());
            }
        }

        Ok(filtered_paths)
    }

    /// Apply exclusion patterns to a list of paths
    /// Returns (filtered_paths, excluded_count)
    pub async fn apply_exclusions(&self, paths: &[String]) -> Result<(Vec<String>, usize)> {
        let (include_paths, exclude_patterns) = self.extract_exclusion_patterns(paths);

        if exclude_patterns.is_empty() {
            return Ok((include_paths, 0));
        }

        let original_count = include_paths.len();
        let filtered_paths = self
            .filter_excluded_paths(&include_paths, &exclude_patterns)
            .await?;
        let excluded_count = original_count - filtered_paths.len();

        Ok((filtered_paths, excluded_count))
    }

    /// Get information about the current matcher
    pub fn matcher_info(&self) -> &'static str {
        self.matcher.description()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wildcard_matcher_basic() {
        let matcher = WildcardMatcher::new();

        // Test exact match
        assert!(matcher.matches("file.txt", "file.txt").await);
        assert!(!matcher.matches("other.txt", "file.txt").await);

        // Test wildcard *
        assert!(matcher.matches("file.txt", "*.txt").await);
        assert!(matcher.matches("document.pdf", "*.pdf").await);
        assert!(!matcher.matches("file.txt", "*.pdf").await);

        // Test wildcard ?
        assert!(matcher.matches("file1.txt", "file?.txt").await);
        assert!(matcher.matches("file2.txt", "file?.txt").await);
        assert!(!matcher.matches("file10.txt", "file?.txt").await);
    }

    #[tokio::test]
    async fn test_wildcard_matcher_paths() {
        let matcher = WildcardMatcher::new();

        // Test path patterns
        assert!(matcher.matches("C:\\temp\\file.txt", "*/temp/*").await);
        assert!(matcher.matches("/home/user/file.txt", "*/user/*").await);
        assert!(
            matcher
                .matches("C:\\Windows\\System32\\file.dll", "*system32*")
                .await
        );
    }

    #[tokio::test]
    async fn test_exclusion_service_extract_patterns() {
        let service = ExclusionService::new(WildcardMatcher::new());

        let paths = vec![
            "C:\\file1.txt".to_string(),
            "!*.tmp".to_string(),
            "C:\\file2.txt".to_string(),
            "!*cache*".to_string(),
            "C:\\file3.txt".to_string(),
        ];

        let (include_paths, exclude_patterns) = service.extract_exclusion_patterns(&paths);

        assert_eq!(include_paths.len(), 3);
        assert_eq!(exclude_patterns.len(), 2);
        assert_eq!(exclude_patterns[0], "*.tmp");
        assert_eq!(exclude_patterns[1], "*cache*");
    }

    #[tokio::test]
    async fn test_exclusion_service_filter() {
        let service = ExclusionService::new(WildcardMatcher::new());

        let paths = vec![
            "C:\\important.txt".to_string(),
            "C:\\temp.tmp".to_string(),
            "C:\\cache\\data.txt".to_string(),
            "C:\\document.pdf".to_string(),
        ];

        let exclude_patterns = vec!["*.tmp".to_string(), "*cache*".to_string()];

        let filtered = service
            .filter_excluded_paths(&paths, &exclude_patterns)
            .await
            .unwrap();

        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"C:\\important.txt".to_string()));
        assert!(filtered.contains(&"C:\\document.pdf".to_string()));
    }

    #[tokio::test]
    async fn test_exclusion_service_apply() {
        let service = ExclusionService::new(WildcardMatcher::new());

        let paths = vec![
            "C:\\important.txt".to_string(),
            "!*.tmp".to_string(),
            "C:\\temp.tmp".to_string(),
            "!*cache*".to_string(),
            "C:\\cache\\data.txt".to_string(),
            "C:\\document.pdf".to_string(),
        ];

        let (filtered, excluded_count) = service.apply_exclusions(&paths).await.unwrap();

        assert_eq!(filtered.len(), 2);
        assert_eq!(excluded_count, 2);
        assert!(filtered.contains(&"C:\\important.txt".to_string()));
        assert!(filtered.contains(&"C:\\document.pdf".to_string()));
    }

    #[tokio::test]
    async fn test_no_exclusions() {
        let service = ExclusionService::new(WildcardMatcher::new());

        let paths = vec!["C:\\file1.txt".to_string(), "C:\\file2.txt".to_string()];

        let (filtered, excluded_count) = service.apply_exclusions(&paths).await.unwrap();

        assert_eq!(filtered.len(), 2);
        assert_eq!(excluded_count, 0);
    }
}
