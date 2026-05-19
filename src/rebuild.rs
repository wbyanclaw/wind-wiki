//! Rebuild Pipeline — Re-ingest source files modified since a given timestamp.
//!
//! Usage: `wind wiki rebuild --since <RFC3339 timestamp>`
//! Dry run: `wind wiki rebuild --since <timestamp> --dry-run`

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

use crate::ingest;
use crate::wiki::Wiki;

/// Result of a rebuild run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RebuildResult {
    pub ok: bool,
    pub since: String,
    pub dry_run: bool,
    /// Files that were (or would be) rebuilt.
    pub files: Vec<RebuildFile>,
    pub rebuilt_count: usize,
    pub skipped_count: usize,
}

/// A single file in a rebuild run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RebuildFile {
    pub source: String,
    pub wiki_path: String,
    pub modified_at: String,
    pub status: RebuildStatus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RebuildStatus {
    Rebuilt,
    Skipped,
    Failed,
}

/// Run the rebuild pipeline.
pub async fn run(
    wiki: &Wiki,
    since: DateTime<Utc>,
    dry_run: bool,
) -> Result<RebuildResult> {
    let workspace = wiki.config().workspace_dir();

    // Collect source files
    let sources = collect_source_files(&workspace)?;

    let mut files = Vec::new();
    let mut rebuilt_count = 0usize;
    let mut skipped_count = 0usize;

    for source_path in sources {
        let modified = file_modified(&source_path)?;
        if modified < since {
            skipped_count += 1;
            files.push(RebuildFile {
                source: source_path.to_string_lossy().to_string(),
                wiki_path: String::new(),
                modified_at: modified.to_rfc3339(),
                status: RebuildStatus::Skipped,
            });
            continue;
        }

        let source_str = source_path.to_string_lossy().to_string();

        if dry_run {
            rebuilt_count += 1;
            files.push(RebuildFile {
                source: source_str,
                wiki_path: String::from("[dry-run — would rebuild]"),
                modified_at: modified.to_rfc3339(),
                status: RebuildStatus::Rebuilt,
            });
            continue;
        }

        // Actually re-ingest
        match ingest::run(wiki, &source_str).await {
            Ok(result) => {
                rebuilt_count += 1;
                files.push(RebuildFile {
                    source: source_str,
                    wiki_path: result.wiki_path,
                    modified_at: modified.to_rfc3339(),
                    status: RebuildStatus::Rebuilt,
                });
            }
            Err(e) => {
                files.push(RebuildFile {
                    source: source_str,
                    wiki_path: format!("ERROR: {}", e),
                    modified_at: modified.to_rfc3339(),
                    status: RebuildStatus::Failed,
                });
            }
        }
    }

    Ok(RebuildResult {
        ok: true,
        since: since.to_rfc3339(),
        dry_run,
        files,
        rebuilt_count,
        skipped_count,
    })
}

/// Collect all supported source files in the workspace directory.
fn collect_source_files(workspace: &Path) -> Result<Vec<std::path::PathBuf>> {
    let extensions = ["pdf", "md", "markdown", "txt", "html", "htm", "json", "csv"];

    let mut sources = Vec::new();
    for entry in walkdir::WalkDir::new(workspace)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| extensions.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
        {
            sources.push(path.to_path_buf());
        }
    }

    Ok(sources)
}

fn file_modified(path: &Path) -> Result<DateTime<Utc>> {
    let meta = std::fs::metadata(path)?;
    let modified = meta.modified()?;
    Ok(modified.into())
}
