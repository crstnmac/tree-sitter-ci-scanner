use tree_sitter::{Parser, Language};
use anyhow::{Result, Context};

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageType {
    JavaScript,
    TypeScript,
    Python,
    HTML,
    // Add more languages as needed
}

impl LanguageType {
    /// Get the language name as a string
    pub fn name(&self) -> &'static str {
        match self {
            LanguageType::JavaScript => "javascript",
            LanguageType::TypeScript => "typescript",
            LanguageType::Python => "python",
            LanguageType::HTML => "html",
        }
    }

    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "js" | "jsx" | "mjs" | "cjs" => Some(LanguageType::JavaScript),
            "ts" | "tsx" => Some(LanguageType::TypeScript),
            "py" => Some(LanguageType::Python),
            "html" | "htm" => Some(LanguageType::HTML),
            _ => None,
        }
    }

    /// Get the tree-sitter language for this type
    pub fn get_tree_sitter_language(&self) -> Language {
        match self {
            LanguageType::JavaScript => tree_sitter_javascript::language(),
            LanguageType::TypeScript => tree_sitter_typescript::language_typescript(),
            LanguageType::Python => tree_sitter_python::language(),
            LanguageType::HTML => tree_sitter_html::language(),
        }
    }
}

/// Wrapper for tree-sitter parser
pub struct CodeParser {
    parser: Parser,
    language_type: LanguageType,
}

impl CodeParser {
    /// Create a new parser for the specified language
    pub fn new(language_type: LanguageType) -> Result<Self> {
        let mut parser = Parser::new();
        let language = language_type.get_tree_sitter_language();

        parser
            .set_language(&language)
            .context("Failed to set parser language")?;

        Ok(Self {
            parser,
            language_type,
        })
    }

    /// Parse source code into a syntax tree
    pub fn parse(&mut self, source: &str) -> Result<tree_sitter::Tree> {
        self.parser
            .parse(source, None)
            .context("Failed to parse source code")
    }

    /// Get the language type
    pub fn language_type(&self) -> LanguageType {
        self.language_type
    }

    /// Parse a file from disk
    pub fn parse_file(&mut self, path: &std::path::Path) -> Result<(tree_sitter::Tree, String)> {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let tree = self.parse(&source)?;
        Ok((tree, source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_extension() {
        assert_eq!(LanguageType::from_extension("js"), Some(LanguageType::JavaScript));
        assert_eq!(LanguageType::from_extension("ts"), Some(LanguageType::TypeScript));
        assert_eq!(LanguageType::from_extension("py"), Some(LanguageType::Python));
        assert_eq!(LanguageType::from_extension("html"), Some(LanguageType::HTML));
        assert_eq!(LanguageType::from_extension("unknown"), None);
    }

    #[test]
    fn test_language_name() {
        assert_eq!(LanguageType::JavaScript.name(), "javascript");
        assert_eq!(LanguageType::TypeScript.name(), "typescript");
        assert_eq!(LanguageType::Python.name(), "python");
        assert_eq!(LanguageType::HTML.name(), "html");
    }
}
