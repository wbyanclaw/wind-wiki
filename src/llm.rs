//! LLM client abstraction — supports Anthropic and OpenAI.

use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::{LlmConfig, LlmProvider};

/// A chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", content = "content")]
#[serde(rename_all = "lowercase")]
pub enum Message {
    User(String),
    Assistant(String),
    System(String),
}

impl Message {
    pub fn user(s: impl Into<String>) -> Self {
        Self::User(s.into())
    }
    pub fn assistant(s: impl Into<String>) -> Self {
        Self::Assistant(s.into())
    }
    pub fn system(s: impl Into<String>) -> Self {
        Self::System(s.into())
    }
}

/// LLM client for making chat completions.
#[derive(Clone)]
pub struct LlmClient {
    config: LlmConfig,
    http: Client,
}

impl LlmClient {
    /// Create a new client from config.
    pub fn new(config: LlmConfig) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client must build");
        Self { config, http }
    }

    /// Send a chat completion request and return the assistant's reply.
    pub async fn chat(&self, messages: &[Message]) -> Result<String> {
        match self.config.provider {
            LlmProvider::Anthropic => self.anthropic_chat(messages).await,
            LlmProvider::OpenAi => self.openai_chat(messages).await,
            LlmProvider::Unknown => bail!("unknown LLM provider"),
        }
    }

    // ── Anthropic ────────────────────────────────────────────────

    async fn anthropic_chat(&self, messages: &[Message]) -> Result<String> {
        let body = AnthropicRequest {
            model: &self.config.model,
            max_tokens: 1024,
            messages: messages
                .iter()
                .map(|m| AnthropicMessage {
                    role: match m {
                        Message::User(_) => "user",
                        Message::Assistant(_) => "assistant",
                        Message::System(_) => "user", // inject as user for Claude
                    },
                    content: match m {
                        Message::User(s) | Message::System(s) => s.clone(),
                        Message::Assistant(s) => s.clone(),
                    },
                })
                .collect(),
        };

        let base_url = self
            .config
            .base_url
            .as_deref()
            .unwrap_or("https://api.anthropic.com");

        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

        let response = self
            .http
            .post(&url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            bail!("Anthropic API error {}: {}", status, text);
        }

        let parsed: AnthropicResponse = serde_json::from_str(&text)
            .context("failed to parse Anthropic response")?;

        Ok(parsed.content.first().and_then(|c| c.text.clone()).unwrap_or_default())
    }

    // ── OpenAI ───────────────────────────────────────────────────

    async fn openai_chat(&self, messages: &[Message]) -> Result<String> {
        #[derive(Serialize)]
        struct OpenAiRequest<'a> {
            model: &'a str,
            messages: Vec<OpenAiMessage<'a>>,
        }
        #[derive(Serialize)]
        struct OpenAiMessage<'a> {
            role: &'a str,
            content: &'a str,
        }

        let body = OpenAiRequest {
            model: &self.config.model,
            messages: messages
                .iter()
                .map(|m| {
                    let (role, content) = match m {
                        Message::User(s) | Message::System(s) => ("user", s.as_str()),
                        Message::Assistant(s) => ("assistant", s.as_str()),
                    };
                    OpenAiMessage { role, content }
                })
                .collect(),
        };

        let base_url = self
            .config
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com");

        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

        let response = self
            .http
            .post(&url)
            .header("authorization", format!("Bearer {}", self.config.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            bail!("OpenAI API error {}: {}", status, text);
        }

        #[derive(Deserialize)]
        struct OpenAiResponse {
            choices: Vec<OpenAiChoice>,
        }
        #[derive(Deserialize)]
        struct OpenAiChoice {
            message: OpenAiMessageReply,
        }
        #[derive(Deserialize)]
        struct OpenAiMessageReply {
            content: String,
        }

        let parsed: OpenAiResponse = serde_json::from_str(&text)
            .context("failed to parse OpenAI response")?;

        Ok(parsed
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }
}

// ── Anthropic types ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(default)]
    text: Option<String>,
}

