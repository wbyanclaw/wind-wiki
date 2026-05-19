//! Init Pipeline — Bootstrap the wiki directory structure.
//!
//! Creates:
//! - wiki/ directory
//! - workspace/ directory
//! - SYSTEM.md (with default content) if not exists

use anyhow::Result;

use crate::wiki::Wiki;

/// Result of an init run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InitResult {
    pub ok: bool,
    pub wiki_dir: String,
    pub workspace_dir: String,
    pub system_md: String,
    pub created: Vec<String>,
}

/// Default SYSTEM.md content.
const DEFAULT_SYSTEM_MD: &str = r#"# Wiki AI 行为准则

你是一个专业的知识库助手，擅长将源文档转化为结构化、精确的 Wiki 页面。

## 写作原则

1. **准确性**：只陈述原文中有明确依据的内容，不推测、不编造
2. **结构化**：使用 Markdown 标题层级组织内容
3. **可链接**：识别相关主题，用 `[[主题名]]` 格式添加内链
4. **简洁性**：每个 Wiki 页面聚焦单一主题，不堆砌无关信息

## Wikilink 使用规范

- 链接到相关主题时使用 `[[主题名]]`
- 主题名使用简短、清晰的文件名风格
- 如果不确定某个主题是否存在，宁可不链接也不要创建断链
- 每个页面至少链接到 1 个相关主题

## 格式要求

- 每个页面以 `<!-- source: filename -->` 元数据注释开头
- 包含 `<!-- generated: timestamp -->` 时间戳
- 使用中文撰写
"#;

/// Run the init pipeline.
pub fn run(wiki: &Wiki) -> Result<InitResult> {
    let config = wiki.config();
    let wiki_dir = config.wiki_dir()?;
    let workspace_dir = config.workspace_dir();
    let system_md_path = config.system_md_path();

    let mut created = Vec::new();

    // Create directories
    if !wiki_dir.exists() {
        std::fs::create_dir_all(&wiki_dir)?;
        created.push(wiki_dir.to_string_lossy().to_string());
    }

    if !workspace_dir.exists() {
        std::fs::create_dir_all(&workspace_dir)?;
        created.push(workspace_dir.to_string_lossy().to_string());
    }

    // Create default SYSTEM.md
    let _system_md_created = if !system_md_path.exists() {
        std::fs::write(&system_md_path, DEFAULT_SYSTEM_MD)?;
        created.push(system_md_path.to_string_lossy().to_string());
        true
    } else {
        false
    };

    Ok(InitResult {
        ok: true,
        wiki_dir: wiki_dir.to_string_lossy().to_string(),
        workspace_dir: workspace_dir.to_string_lossy().to_string(),
        system_md: system_md_path.to_string_lossy().to_string(),
        created,
    })
}
