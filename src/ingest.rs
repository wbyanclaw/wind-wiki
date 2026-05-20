//! Ingest Pipeline — Source file → LLM → Wiki Markdown.
//!
//! 1. Read source file (PDF, Markdown, HTML, TXT, etc.)
//! 2. Extract plain text content
//! 3. Call LLM with system prompt + source content
//! 4. Write wiki entry to `wiki/`

use anyhow::Result;
use std::path::Path;

use crate::llm::Message;
use crate::wiki::Wiki;

/// Result of an ingest operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IngestResult {
    /// Whether the ingest succeeded.
    pub ok: bool,
    /// Path to the generated wiki file.
    pub wiki_path: String,
    /// Title derived from the source file.
    pub title: String,
    /// Source file that was ingested.
    pub source: String,
    /// Number of characters in the extracted text.
    pub source_chars: usize,
    /// Number of characters in the generated wiki content.
    pub wiki_chars: usize,
    /// Token usage estimate (input + output).
    #[serde(default)]
    pub tokens_estimate: Option<(u32, u32)>,
}

impl IngestResult {
    pub fn ok(
        wiki_path: String,
        title: String,
        source: String,
        source_chars: usize,
        wiki_chars: usize,
    ) -> Self {
        Self {
            ok: true,
            wiki_path,
            title,
            source,
            source_chars,
            wiki_chars,
            tokens_estimate: None,
        }
    }
}

/// Run the ingest pipeline.
pub async fn run(wiki: &Wiki, source_path: &str) -> Result<IngestResult> {
    let source = wiki.resolve_source(source_path);

    if !source.exists() {
        anyhow::bail!("source file not found: {}", source.display());
    }

    // 1. Extract text from the source file
    let (content, extension) = extract_text(&source)?;
    let title = derive_title(&source, &content);
    let source_chars = content.chars().count();

    // 2. Build the prompt
    let system_md = wiki.config().system_md_content()?;
    let prompt = build_ingest_prompt(&system_md, &title, &content, &extension);

    // 3. Call LLM
    let llm = wiki.llm();
    let messages = &[Message::system(&prompt)];
    let response = llm.chat(messages).await?;

    let wiki_chars = response.chars().count();

    // 4. Write wiki entry
    let wiki_path = wiki.write_wiki_entry(&title, &response)?;

    Ok(IngestResult::ok(
        wiki_path.to_string_lossy().to_string(),
        title,
        source.to_string_lossy().to_string(),
        source_chars,
        wiki_chars,
    ))
}

/// Extract plain text from a source file based on extension.
fn extract_text(path: &Path) -> Result<(String, String)> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("txt")
        .to_lowercase();

    let content = match ext.as_str() {
        "md" | "markdown" | "txt" | "text" => std::fs::read_to_string(path)?,
        "pdf" => extract_pdf(path)?,
        "html" | "htm" => extract_html(path)?,
        "json" => extract_json(path)?,
        "csv" => extract_csv(path)?,
        _ => {
            // Try reading as text first
            std::fs::read_to_string(path).unwrap_or_else(|_| {
                format!("[binary file: {} — cannot extract text]", path.display())
            })
        }
    };

    Ok((content, ext))
}

/// Extract text from a PDF using the `lopdf` crate.
fn extract_pdf(path: &Path) -> Result<String> {
    let doc = lopdf::Document::load(path)?;
    let mut text = String::new();

    // Iterate through all pages and extract text
    let pages = doc.get_pages();
    for (page_num, _) in pages {
        if let Ok(page_text) = doc.extract_text(&[page_num]) {
            text.push_str(&page_text);
            text.push('\n');
        }
    }

    if text.trim().is_empty() {
        anyhow::bail!("PDF contains no extractable text: {}", path.display());
    }

    Ok(text)
}

/// Extract text from HTML.
fn extract_html(path: &Path) -> Result<String> {
    let html = std::fs::read_to_string(path)?;
    let document = scraper::Html::parse_document(&html);
    let selector = scraper::Selector::parse("body")
        .map_err(|e| anyhow::anyhow!("invalid selector 'body': {}", e))?;
    let body = document
        .select(&selector)
        .next()
        .unwrap_or_else(|| document.root_element());

    let text = body.text().collect::<Vec<_>>().join("\n");
    Ok(text)
}

/// Extract text from JSON (pretty-print as text).
fn extract_json(path: &Path) -> Result<String> {
    let raw = std::fs::read_to_string(path)?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)?;
    Ok(serde_json::to_string_pretty(&parsed)?)
}

/// Extract text from CSV (concatenate all cells).
fn extract_csv(path: &Path) -> Result<String> {
    let raw = std::fs::read_to_string(path)?;
    let mut result = String::new();
    for line in raw.lines() {
        result.push_str(line);
        result.push('\n');
    }
    Ok(result)
}

/// Derive a wiki title from the source filename and content.
fn derive_title(path: &Path, content: &str) -> String {
    // Try to extract first H1 from Markdown
    if let Some(first_line) = content.lines().next() {
        let trimmed = first_line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            return rest.trim().to_string();
        }
    }

    // Fall back to filename
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("未命名")
        .to_string()
}

/// Build the LLM prompt for ingestion.
fn build_ingest_prompt(system_md: &str, title: &str, content: &str, _ext: &str) -> String {
    // Truncate content if too long (first 8000 chars to stay within token budget)
    let truncated = if content.chars().count() > 8000 {
        content.chars().take(8000).collect::<String>() + "\n\n[... 内容已截断 ...]"
    } else {
        content.to_string()
    };

    format!(
        r#"# System Prompt

{system_md}

---

# Task

请为以下源文档创建一个 Wiki 页面。

## 源文档标题
{title}

## 源文档内容
{truncated}

---

## 输出要求

请用中文撰写 Wiki 页面内容，包括：
1. **概述**：文档的核心主题
2. **关键要点**：提取最重要的 3-5 个要点
3. **详细信息**：基于原文的详细内容
4. **相关说明**：任何补充信息

格式要求：
- 使用 Markdown 格式
- 包含 `<!-- source: filename -->` 元数据注释
- 简洁准确，不得编造原文没有的信息
- **Wikilinks**：在"相关说明"或正文适当位置，添加 `[[相关主题]]` 格式的内链
  - 如果有其他已知 wiki 主题与本文档相关，用 `[[主题名]]` 链接
  - 链接目标使用简短、清晰的文件名风格（如 `Q1营收摘要`、`市场竞争分析`）
  - 每个页面至少添加 1-3 个相关 wikilinks
"#
    )
}
