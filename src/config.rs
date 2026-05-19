//! Configuration for llm-wiki.
//!
//! Loads from `~/.config/wind/config.toml` with sensible defaults.
//! Supports both Anthropic and OpenAI providers.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{Context, Result};

/// LLM provider type.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    OpenAi,
    #[default]
    Unknown,
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anthropic => write!(f, "anthropic"),
            Self::OpenAi => write!(f, "openai"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl From<&str> for LlmProvider {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "anthropic" => Self::Anthropic,
            "openai" | "open_ai" => Self::OpenAi,
            _ => Self::Unknown,
        }
    }
}

/// LLM configuration for the wiki engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Provider name: "anthropic" or "openai"
    pub provider: LlmProvider,
    /// Model name, e.g. "claude-haiku-4-20250514" or "gpt-4o-mini"
    pub model: String,
    /// API key (sk-ant-... or sk-...)
    pub api_key: String,
    /// Optional base URL for compatible APIs (e.g. OpenRouter)
    #[serde(default)]
    pub base_url: Option<String>,
}

impl LlmConfig {
    /// Validate that the config has required fields.
    pub fn validate(&self) -> Result<()> {
        if self.api_key.is_empty() {
            anyhow::bail!("LLM api_key is required");
        }
        if self.model.is_empty() {
            anyhow::bail!("LLM model is required");
        }
        if matches!(self.provider, LlmProvider::Unknown) {
            anyhow::bail!("LLM provider must be 'anthropic' or 'openai'");
        }
        Ok(())
    }
}

/// Paths used by llm-wiki.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Paths {
    /// Directory containing source files (user workspace).
    /// Defaults to `~/.local/share/wind/workspace/`
    #[serde(default = "default_workspace_path")]
    pub workspace: PathBuf,

    /// Directory where wiki Markdown files are stored.
    /// Defaults to `~/.local/share/wind/wiki/`
    #[serde(default = "default_wiki_path")]
    pub wiki: PathBuf,

    /// SYSTEM.md — AI behavior rules.
    /// Defaults to `~/.local/share/wind/SYSTEM.md`
    #[serde(default = "default_system_md_path")]
    pub system_md: PathBuf,
}

fn default_workspace_path() -> PathBuf {
    data_dir().join("workspace")
}

fn default_wiki_path() -> PathBuf {
    data_dir().join("wiki")
}

fn default_system_md_path() -> PathBuf {
    config_dir().join("SYSTEM.md")
}

fn data_dir() -> PathBuf {
    ProjectDirs::from("com", "wind-cli", "wind")
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("~/.local/share/wind"))
}

fn config_dir() -> PathBuf {
    ProjectDirs::from("com", "wind-cli", "wind")
        .map(|p| p.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("~/.config/wind"))
}

/// Top-level config file (`[wiki]` section).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// LLM settings for the wiki engine.
    #[serde(default)]
    pub llm: Option<LlmConfig>,

    /// Directory paths.
    #[serde(default)]
    pub paths: Paths,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            llm: None,
            paths: Paths {
                workspace: default_workspace_path(),
                wiki: default_wiki_path(),
                system_md: default_system_md_path(),
            },
        }
    }
}

/// Raw config.toml structure for loading the [wiki] section.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct RawWikiConfig {
    #[serde(default)]
    llm: Option<LlmConfig>,
    #[serde(default)]
    paths: Option<Paths>,
}

/// Full config.toml parsed directly.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct RawConfig {
    #[serde(default)]
    wiki: Option<RawWikiConfig>,
}

impl Config {
    /// Load config from `~/.config/wind/config.toml`, or return defaults.
    pub fn load() -> Result<Self> {
        let path = config_file_path();

        if !path.exists() {
            return Ok(Config::default());
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;

        let raw: RawConfig = toml::from_str(&content)
            .with_context(|| format!("failed to parse config at {}", path.display()))?;

        let wiki_section = raw.wiki.unwrap_or_default();
        Ok(Config {
            llm: wiki_section.llm,
            paths: wiki_section.paths.unwrap_or_default(),
        })
    }

    /// Resolve the effective LLM config, using env vars as fallback.
    pub fn resolved_llm(&self) -> Result<LlmConfig> {
        if let Some(ref llm) = self.llm {
            return llm.validate().map(|_| llm.clone());
        }

        // Try environment variables
        let provider = std::env::var("WIND_WIKI_PROVIDER")
            .unwrap_or_else(|_| "anthropic".to_string());
        let model = std::env::var("WIND_WIKI_MODEL")
            .unwrap_or_else(|_| "claude-haiku-4-20250514".to_string());
        let api_key = std::env::var("WIND_WIKI_API_KEY")
            .with_context(|| "WIND_WIKI_API_KEY not set and [wiki.llm] not in config")?;

        let config = LlmConfig {
            provider: LlmProvider::from(provider.as_str()),
            model,
            api_key,
            base_url: std::env::var("WIND_WIKI_BASE_URL").ok(),
        };

        config.validate()?;
        Ok(config)
    }

    /// Get the wiki directory, creating it if needed.
    pub fn wiki_dir(&self) -> Result<PathBuf> {
        let dir = &self.paths.wiki;
        if !dir.exists() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("failed to create wiki dir: {}", dir.display()))?;
        }
        Ok(dir.clone())
    }

    /// Get the workspace directory.
    pub fn workspace_dir(&self) -> PathBuf {
        self.paths.workspace.clone()
    }

    /// Get the SYSTEM.md content, or a sensible default.
    pub fn system_md_content(&self) -> Result<String> {
        if self.paths.system_md.exists() {
            Ok(std::fs::read_to_string(&self.paths.system_md)?)
        } else {
            Ok(DEFAULT_SYSTEM_MD.to_string())
        }
    }

    /// Get the SYSTEM.md path.
    pub fn system_md_path(&self) -> PathBuf {
        self.paths.system_md.clone()
    }
}

fn config_file_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Default SYSTEM.md content when none is provided.
const DEFAULT_SYSTEM_MD: &str = r#"# WinWork Wiki — System Prompt

你是一个专业的知识库编辑助手。

## 工作原则
1. **准确性**：所有信息必须基于提供的源文档，不得编造。
2. **简洁性**：用简洁的中文描述，避免冗余。
3. **可追溯**：每个知识条目需注明来源文件。

## 格式规范
- 使用 Markdown 格式
- 每个文件对应一个主题
- 文件名使用中文描述主题
- 包含 `<!-- source: filename -->` 元数据注释
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_str() {
        assert!(matches!(LlmProvider::from("anthropic"), LlmProvider::Anthropic));
        assert!(matches!(LlmProvider::from("ANTHROPIC"), LlmProvider::Anthropic));
        assert!(matches!(LlmProvider::from("openai"), LlmProvider::OpenAi));
        assert!(matches!(LlmProvider::from("xyz"), LlmProvider::Unknown));
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.llm.is_none());
        let _ = config.paths.wiki.exists(); // may or may not exist
    }
}
