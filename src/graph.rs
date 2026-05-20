//! Graph Pipeline — Build and query the wikilink knowledge graph.
//!
//! Provides:
//! - `graph()`: Build a full wikilink graph with backlinks
//! - `GraphResult`: JSON-serializable graph structure

use anyhow::Result;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use walkdir::WalkDir;

use crate::wiki::Wiki;

/// A node in the wikilink graph (a wiki file).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphNode {
    /// Filename without .md extension (the wikilink target name).
    pub name: String,
    /// Full path to the file.
    pub path: String,
    /// Titles/headings found in the file.
    pub headings: Vec<String>,
    /// Number of outgoing links from this file.
    pub outlinks: usize,
    /// Number of files that link to this file.
    pub backlinks: usize,
}

/// An edge in the wikilink graph.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
}

/// Result of the graph pipeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphResult {
    pub ok: bool,
    pub node_count: usize,
    pub edge_count: usize,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// Files that are linked to but don't exist (broken links).
    #[serde(default)]
    pub orphans: Vec<String>,
}

impl GraphResult {
    pub fn empty() -> Self {
        Self {
            ok: true,
            node_count: 0,
            edge_count: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
            orphans: Vec::new(),
        }
    }
}

/// Run the graph pipeline: build the full wikilink graph with backlinks.
pub fn run(wiki: &Wiki) -> Result<GraphResult> {
    let wiki_dir = wiki.config().wiki_dir()?;

    if !wiki_dir.exists() {
        return Ok(GraphResult::empty());
    }

    // Collect all wiki files
    let files: Vec<_> = WalkDir::new(&wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .filter(|e| e.path().file_name().is_some_and(|n| n != "SYSTEM.md"))
        .collect();

    if files.is_empty() {
        return Ok(GraphResult::empty());
    }

    let wikilink_re = Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
    let heading_re = Regex::new(r"^#\s+(.+)$").unwrap();

    // Build node list and extract wikilinks
    let mut name_to_path: HashMap<String, String> = HashMap::new();
    let mut outlinks: HashMap<String, Vec<String>> = HashMap::new();

    for entry in &files {
        let path = entry.path();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let content = std::fs::read_to_string(path)?;
        let links: Vec<String> = wikilink_re
            .captures_iter(&content)
            .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .collect();

        name_to_path.insert(name.clone(), path.to_string_lossy().to_string());
        outlinks.insert(name, links);
    }

    // Build backlinks index
    let mut backlinks: HashMap<String, Vec<String>> = HashMap::new();
    for (from, links) in &outlinks {
        for to in links.iter() {
            backlinks.entry(to.clone()).or_default().push(from.clone());
        }
    }

    // Build edges and orphans
    let mut edges = Vec::new();
    let all_target_names: HashSet<String> = name_to_path.keys().cloned().collect();
    let mut orphans = Vec::new();

    for (from, links) in &outlinks {
        for to in links.iter() {
            edges.push(GraphEdge {
                from: from.clone(),
                to: to.clone(),
            });
            if !all_target_names.contains(to.as_str()) {
                orphans.push(to.clone());
            }
        }
    }

    // Build nodes
    let mut nodes = Vec::new();
    for entry in &files {
        let path = entry.path();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let content = std::fs::read_to_string(path)?;

        let headings: Vec<String> = heading_re
            .captures_iter(&content)
            .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .collect();

        let out_count = outlinks.get(&name).map(|v| v.len()).unwrap_or(0);
        let back_count = backlinks.get(&name).map(|v| v.len()).unwrap_or(0);

        nodes.push(GraphNode {
            name,
            path: path.to_string_lossy().to_string(),
            headings,
            outlinks: out_count,
            backlinks: back_count,
        });
    }

    // Sort nodes by backlink count (most connected first)
    nodes.sort_by_key(|n| std::cmp::Reverse(n.backlinks));

    Ok(GraphResult {
        ok: true,
        node_count: nodes.len(),
        edge_count: edges.len(),
        nodes,
        edges,
        orphans,
    })
}
