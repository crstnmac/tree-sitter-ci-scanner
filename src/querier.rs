use tree_sitter::{Query, QueryCursor};
use anyhow::Result;

/// Match result from a tree-sitter query
#[derive(Debug, Clone)]
pub struct Match<'a> {
    pub captures: Vec<Capture<'a>>,
}

/// Capture from a tree-sitter query
#[derive(Debug, Clone)]
pub struct Capture<'a> {
    pub index: u32,
    pub name: String,
    pub node: tree_sitter::Node<'a>,
}

/// Run a tree-sitter query on a parsed syntax tree
///
/// # Arguments
///
/// * `tree` - The parsed syntax tree
/// * `source` - The original source code
/// * `query_str` - The tree-sitter query string
///
/// # Returns
///
/// A vector of matches found by the query
pub fn run_query<'a>(
    tree: &'a tree_sitter::Tree,
    source: &'a str,
    query_str: &str,
) -> Result<Vec<Match<'a>>> {
    let root_node = tree.root_node();
    let query = Query::new(&root_node.language(), query_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse query: {}", e))?;

    let mut cursor = QueryCursor::new();
    let mut matches = Vec::new();

    for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        let captures: Vec<Capture> = m
            .captures
            .iter()
            .map(|c| {
                let name = query.capture_names()[c.index as usize].to_string();
                Capture {
                    index: c.index,
                    name,
                    node: c.node,
                }
            })
            .collect();

        matches.push(Match { captures });
    }

    Ok(matches)
}

/// Run a query and return the first match
///
/// This is useful when you only care about the first occurrence of a pattern.
pub fn run_query_once<'a>(
    tree: &'a tree_sitter::Tree,
    source: &'a str,
    query_str: &str,
) -> Result<Option<Match<'a>>> {
    let matches = run_query(tree, source, query_str)?;
    Ok(matches.into_iter().next())
}

/// Get all nodes matching a specific capture name
///
/// # Arguments
///
/// * `tree` - The parsed syntax tree
/// * `source` - The original source code
/// * `query_str` - The tree-sitter query string
/// * `capture_name` - The name of the capture to filter by
///
/// # Returns
///
/// A vector of nodes matching the capture name
pub fn get_captures<'a>(
    tree: &'a tree_sitter::Tree,
    source: &'a str,
    query_str: &str,
    capture_name: &str,
) -> Result<Vec<tree_sitter::Node<'a>>> {
    let matches = run_query(tree, source, query_str)?;
    let mut nodes = Vec::new();

    for m in matches {
        for c in m.captures {
            if c.name == capture_name {
                nodes.push(c.node);
            }
        }
    }

    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_empty() {
        // This is a placeholder test - real tests would need actual tree-sitter setup
        // For now, just ensure the module compiles
    }
}
