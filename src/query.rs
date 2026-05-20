//! Query Pipeline — User question → Read wiki → LLM answer.
//!
//! 1. Read all (or relevant) wiki .md files
//! 2. Call LLM with question + wiki context
//! 3. Return synthesized answer

use anyhow::Result;
use regex;
use std::path::PathBuf;

use crate::llm::Message;
use crate::wiki::Wiki;

/// Result of a query.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryResult {
    pub ok: bool,
    pub question: String,
    pub answer: String,
    /// Paths of wiki files that were read to answer the question.
    pub sources: Vec<String>,
    /// Whether the wiki was empty.
    #[serde(default)]
    pub no_wiki_content: bool,
}

impl QueryResult {
    pub fn ok(question: String, answer: String, sources: Vec<PathBuf>) -> Self {
        Self {
            ok: true,
            question,
            answer,
            sources: sources
                .into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            no_wiki_content: false,
        }
    }

    pub fn empty(question: String) -> Self {
        Self {
            ok: true,
            question,
            answer: String::new(),
            sources: vec![],
            no_wiki_content: true,
        }
    }
}

/// Run the query pipeline.
pub async fn run(wiki: &Wiki, question: &str) -> Result<QueryResult> {
    let files = wiki.read_wiki_files()?;

    if files.is_empty() {
        return Ok(QueryResult::empty(question.to_string()));
    }

    // Combine all wiki content for context
    let context = build_context(&files);
    let system_md = wiki.config().system_md_content()?;
    let prompt = build_query_prompt(&system_md, question, &context);

    let llm = wiki.llm();
    let messages = &[Message::system(&prompt)];
    let answer = llm.chat(messages).await?;
    let answer = strip_think_tags(&answer);

    let sources: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    Ok(QueryResult::ok(question.to_string(), answer, sources))
}

/// Strip LLM thinking tags (<think>/</think>) from model responses.
fn strip_think_tags(s: &str) -> String {
    let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
    re.replace_all(s, "").to_string()
}

/// Build a combined context string from wiki files.
fn build_context(files: &[crate::wiki::WikiFile]) -> String {
    if files.is_empty() {
        return String::from("(知识库为空)");
    }

    let mut ctx = String::new();

    for file in files {
        let _filename = file
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        ctx.push_str("## 文件: \n\n");
        ctx.push_str(&file.content);
        ctx.push_str("\n\n---\n\n");
    }

    ctx
}

/// Build the LLM prompt for querying.
fn build_query_prompt(system_md: &str, question: &str, context: &str) -> String {
    format!(
        r#"# System Prompt

{system_md}

---

# Task

你是一个知识库助手。请根据以下知识库内容回答用户的问题。

## 知识库内容
{context}

---

## 用户问题

{question}

---

## 要求

1. 优先使用知识库中的信息回答
2. 如果知识库中没有相关信息，直接说明"知识库中没有相关内容"
3. 用中文回答
4. 如有参考，提及来源文件名
5. 简洁准确，不得编造信息
"#
    )
}
