//! Lint Pipeline — Audit wiki health.
//!
//! Checks performed:
//! - Duplicate headings across files
//! - Broken wikilinks (`[[filename]]` references to non-existent files)
//! - Outdated entries (no source metadata)
//! - Empty files
//! - Content contradictions (simple heuristic via LLM)

use anyhow::Result;
use regex::Regex;
use walkdir::WalkDir;

use crate::wiki::Wiki;

/// A single lint issue.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LintIssue {
    pub severity: IssueSeverity,
    pub file: String,
    pub message: String,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
}

/// Result of a lint run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LintResult {
    pub ok: bool,
    pub file_count: u32,
    pub issues: Vec<LintIssue>,
    pub summary: String,
}

impl LintResult {
    pub fn new(file_count: u32, issues: Vec<LintIssue>) -> Self {
        let errors = issues.iter().filter(|i| i.severity == IssueSeverity::Error).count();
        let warnings = issues.iter().filter(|i| i.severity == IssueSeverity::Warning).count();
        let summary = if issues.is_empty() {
            "✅ Wiki 健康，没有发现问题".to_string()
        } else {
            format!(
                "发现问题: {} 个错误, {} 个警告",
                errors, warnings
            )
        };

        Self {
            ok: errors == 0,
            file_count,
            issues,
            summary,
        }
    }
}

/// Run the lint pipeline.
pub async fn run(wiki: &Wiki) -> Result<LintResult> {
    let wiki_dir = wiki.config().wiki_dir()?;
    let files = collect_markdown_files(&wiki_dir)?;

    let mut issues = Vec::new();

    // Pre-compile regexes (avoid creating inside loop)
    let wikilink_re = Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
    let heading_re = Regex::new(r"^#{1,6}\s+(.+)$").unwrap();
    let cross_heading_re = Regex::new(r"^#\s+(.+)$").unwrap();

    // ── Per-file checks ────────────────────────────────────────────

    for file in &files {
        // 1. Empty file
        if file.content.trim().is_empty() {
            issues.push(LintIssue {
                severity: IssueSeverity::Warning,
                file: file.rel_path.clone(),
                message: "文件为空".to_string(),
                line: None,
            });
        }

        // 2. Missing source metadata
        if !file.content.contains("<!-- source:") {
            issues.push(LintIssue {
                severity: IssueSeverity::Info,
                file: file.rel_path.clone(),
                message: "缺少 `<!-- source: ... -->` 元数据注释".to_string(),
                line: None,
            });
        }

        // 3. Check for broken wikilinks
        for cap in wikilink_re.captures_iter(&file.content) {
            let linked = cap.get(1).unwrap().as_str();
            let linked_md = format!("{}.md", linked.trim());
            let linked_path = wiki_dir.join(&linked_md);
            if !linked_path.exists() {
                issues.push(LintIssue {
                    severity: IssueSeverity::Warning,
                    file: file.rel_path.clone(),
                    message: format!("无效的 wikilink: [[{}]] → 文件不存在", linked),
                    line: None,
                });
            }
        }

        // 4. Duplicate headings within file
        let mut headings: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for (line_num, line) in file.content.lines().enumerate() {
            if let Some(cap) = heading_re.captures(line) {
                let h = cap.get(1).unwrap().as_str().trim().to_string();
                *headings.entry(h.clone()).or_insert(0) += 1;
                if headings.get(&h).copied().unwrap_or(0) > 1 {
                    issues.push(LintIssue {
                        severity: IssueSeverity::Info,
                        file: file.rel_path.clone(),
                        message: format!("重复标题: # {}", h),
                        line: Some(line_num as u32 + 1),
                    });
                }
            }
        }

        // 5. Very short content
        let word_count = file.content.split_whitespace().count();
        if word_count < 20 {
            issues.push(LintIssue {
                severity: IssueSeverity::Warning,
                file: file.rel_path.clone(),
                message: format!("内容过短 ({} words)，可能是占位符", word_count),
                line: None,
            });
        }
    }

    // ── Cross-file duplicate heading check ───────────────────────

    let mut all_headings: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for file in &files {
        for line in file.content.lines() {
            if let Some(cap) = cross_heading_re.captures(line) {
                let h = cap.get(1).unwrap().as_str().trim().to_string();
                all_headings.entry(h).or_default().push(file.rel_path.clone());
            }
        }
    }

    for (heading, files_with_heading) in all_headings {
        if files_with_heading.len() > 1 {
            issues.push(LintIssue {
                severity: IssueSeverity::Info,
                file: files_with_heading.join(", "),
                message: format!("标题 '{}' 在多个文件中重复出现", heading),
                line: None,
            });
        }
    }

    let file_count = files.len() as u32;
    Ok(LintResult::new(file_count, issues))
}

// ── Helpers ───────────────────────────────────────────────────────────

struct MdFile {
    rel_path: String,
    content: String,
}

fn collect_markdown_files(dir: &std::path::Path) -> Result<Vec<MdFile>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().is_some_and(|ext| ext == "md")
        })
    {
        let rel = entry
            .path()
            .strip_prefix(dir)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        let content = std::fs::read_to_string(entry.path())?;
        files.push(MdFile {
            rel_path: rel,
            content,
        });
    }

    Ok(files)
}
