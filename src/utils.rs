use anyhow::Result;
use std::path::Path;

/// Get the file extension from a path
///
/// # Arguments
///
/// * `path` - The file path
///
/// # Returns
///
/// The file extension in lowercase, without the dot
pub fn get_file_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
}

/// Check if a path is a supported source file
///
/// # Arguments
///
/// * `path` - The file path
///
/// # Returns
///
/// True if the file has a supported extension
pub fn is_supported_file(path: &Path) -> bool {
    matches!(
        get_file_extension(path).as_deref(),
        Some("js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "py" | "html" | "htm")
    )
}

/// Normalize a path for consistent output
///
/// Converts to string and ensures forward slashes for consistency.
pub fn normalize_path(path: &Path) -> Result<String> {
    path.to_str()
        .map(|s| s.replace('\\', "/"))
        .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in path: {}", path.display()))
}

/// Extract a code snippet from source code
///
/// # Arguments
///
/// * `source` - The full source code
/// * `start_line` - Start line (1-indexed)
/// * `end_line` - End line (1-indexed)
///
/// # Returns
///
/// A string containing the lines between start and end (inclusive)
pub fn extract_snippet(source: &str, start_line: usize, end_line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();

    if lines.is_empty() {
        return String::new();
    }

    let start = start_line.saturating_sub(1);
    let end = end_line.min(lines.len());

    if start >= end {
        return String::new();
    }

    lines[start..end].join("\n")
}

/// Truncate a string to a maximum length
///
/// If the string is longer than max_len, it will be truncated with an ellipsis.
///
/// # Arguments
///
/// * `s` - The string to truncate
/// * `max_len` - Maximum length (including ellipsis)
///
/// # Returns
///
/// The truncated string
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }

    let mut truncated = s.chars().take(max_len.saturating_sub(3)).collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_file_extension() {
        assert_eq!(get_file_extension(Path::new("test.js")), Some("js".to_string()));
        assert_eq!(get_file_extension(Path::new("test.HTML")), Some("html".to_string()));
        assert_eq!(get_file_extension(Path::new("test")), None);
        assert_eq!(get_file_extension(Path::new(".hidden")), None);
    }

    #[test]
    fn test_is_supported_file() {
        assert!(is_supported_file(Path::new("test.js")));
        assert!(is_supported_file(Path::new("test.ts")));
        assert!(is_supported_file(Path::new("test.py")));
        assert!(is_supported_file(Path::new("test.html")));
        assert!(!is_supported_file(Path::new("test.txt")));
        assert!(!is_supported_file(Path::new("test")));
    }

    #[test]
    fn test_extract_snippet() {
        let source = "line1\nline2\nline3\nline4";
        assert_eq!(extract_snippet(source, 1, 2), "line1\nline2");
        assert_eq!(extract_snippet(source, 2, 3), "line2\nline3");
        assert_eq!(extract_snippet(source, 1, 10), source); // Beyond end
        assert_eq!(extract_snippet(source, 5, 10), "line4"); // Partial
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("a", 3), "a");
    }
}
