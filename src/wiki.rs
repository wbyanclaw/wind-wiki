//! Core Wiki struct — manages wiki directory and provides pipeline entry points.

use std::path::PathBuf;
use walkdir::WalkDir;

use crate::config::Config;
use crate::ingest::{self, IngestResult};
use crate::lint::{self, LintResult};
use crate::llm::LlmClient;
use crate::query::{self, QueryResult};

/// Wiki — main entry point for the LLM Wiki SDK.
///
/// ```no_run
/// # async fn run() -> anyhow::Result<()> {
/// use wind_wiki::Wiki;
///
/// let wiki = Wiki::new(Default::default()).await?;
/// let status = wiki.status()?;
/// println!("Wiki has {} entries", status.file_count);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Wiki {
    config: Config,
    llm: LlmClient,
}

impl Wiki {
    /// Create a new Wiki instance, loading config and initialising the LLM client.
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let llm_config = config.resolved_llm()?;
        let llm = LlmClient::new(llm_config);

        // Ensure wiki directory exists
        config.wiki_dir()?;

        Ok(Self { config, llm })
    }

    /// Create from environment variables only (no config file).
    pub async fn from_env() -> anyhow::Result<Self> {
        Self::new(Config::default()).await
    }

    // ── Pipelines ──────────────────────────────────────────────

    /// Ingest a source file into the wiki.
    ///
    /// Reads the file, extracts text, calls the LLM to generate wiki content,
    /// and writes the result to `wiki/`.
    ///
    /// `source_path` is relative to the workspace or absolute.
    pub async fn ingest(&self, source_path: &str) -> anyhow::Result<IngestResult> {
        ingest::run(self, source_path).await
    }

    /// Query the wiki with a natural-language question.
    ///
    /// Reads relevant wiki files and asks the LLM to synthesise an answer.
    pub async fn query(&self, question: &str) -> anyhow::Result<QueryResult> {
        query::run(self, question).await
    }

    /// Lint the wiki for health issues (deadlinks, duplicates, stale content).
    pub async fn lint(&self) -> anyhow::Result<LintResult> {
        lint::run(self).await
    }

    // ── Status ─────────────────────────────────────────────────

    /// Return statistics about the wiki directory.
    pub fn status(&self) -> anyhow::Result<Status> {
        let wiki_dir = &self.config.paths.wiki;

        if !wiki_dir.exists() {
            return Ok(Status {
                wiki_dir: wiki_dir.clone(),
                file_count: 0,
                total_bytes: 0,
                last_modified: None,
                issues: vec![],
            });
        }

        let mut file_count = 0u32;
        let mut total_bytes = 0u64;
        let mut last_modified: Option<chrono::DateTime<chrono::Utc>> = None;

        for entry in WalkDir::new(wiki_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
        {
            if let Ok(meta) = entry.metadata() {
                file_count += 1;
                total_bytes += meta.len();
                if let Ok(modified) = meta.modified() {
                    let dt: chrono::DateTime<chrono::Utc> = modified.into();
                    last_modified = last_modified
                        .map(|l| if dt > l { dt } else { l })
                        .or(Some(dt));
                }
            }
        }

        Ok(Status {
            wiki_dir: wiki_dir.clone(),
            file_count,
            total_bytes,
            last_modified,
            issues: vec![],
        })
    }

    // ── Internal accessors ─────────────────────────────────────

    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    pub(crate) fn llm(&self) -> &LlmClient {
        &self.llm
    }

    /// Resolve a source file path relative to the workspace.
    pub(crate) fn resolve_source(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.config.workspace_dir().join(path)
        }
    }

    /// Read all .md files in the wiki directory.
    pub(crate) fn read_wiki_files(&self) -> anyhow::Result<Vec<WikiFile>> {
        let wiki_dir = &self.config.paths.wiki;
        if !wiki_dir.exists() {
            return Ok(vec![]);
        }

        let mut files = Vec::new();
        for entry in WalkDir::new(wiki_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
        {
            let path = entry.path();
            let content = std::fs::read_to_string(path)?;
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| t.into());

            files.push(WikiFile {
                path: path.to_path_buf(),
                content,
                modified,
            });
        }

        files.sort_by_key(|f| f.modified);
        Ok(files)
    }

    /// Write a wiki entry to disk.
    pub(crate) fn write_wiki_entry(
        &self,
        title: &str,
        content: &str,
    ) -> anyhow::Result<PathBuf> {
        let safe_name = sanitize_filename(title);
        let wiki_dir = self.config.wiki_dir()?;
        let path = wiki_dir.join(format!("{}.md", safe_name));

        // Add source metadata header
        let with_header = format!(
            "<!-- source: ingested -->\n<!-- generated: {} -->\n\n{}",
            chrono::Utc::now().to_rfc3339(),
            content
        );

        std::fs::write(&path, with_header)?;
        Ok(path)
    }
}

/// Sanitize a string for use as a filename.
fn sanitize_filename(name: &str) -> String {
    let s = name.trim();
    let re = regex::Regex::new(r"[^\p{L}\p{N}\-_ ]").unwrap();
    let s = re.replace_all(s, "_");
    let re2 = regex::Regex::new(r"_+").unwrap();
    let s = re2.replace_all(&s, "_");
    s.split_whitespace()
        .take(80)
        .collect::<Vec<_>>()
        .join("_")
}

/// A single wiki Markdown file.
#[derive(Debug, Clone)]
pub struct WikiFile {
    pub path: PathBuf,
    pub content: String,
    pub modified: Option<chrono::DateTime<chrono::Utc>>,
}

/// Wiki statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Status {
    pub wiki_dir: PathBuf,
    pub file_count: u32,
    pub total_bytes: u64,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Q1 摘要 2024"), "Q1_摘要_2024");
        assert_eq!(sanitize_filename("Hello/World"), "Hello_World");
        assert_eq!(sanitize_filename("  多个  空格  "), "多个_空格");
    }
}
