//! llm-wiki-lib — LLM-powered Wiki SDK
//!
//! Provides three pipelines:
//! - **Ingest**: Source file → LLM edit → Wiki Markdown
//! - **Query**: Question → LLM search wiki → Answer
//! - **Lint**: Audit wiki health → Fix deadlinks / contradictions
//!
//! ## Quick start
//!
//! ```no_run
//! use llm_wiki_lib::Wiki;
//!
//! # async fn run() -> anyhow::Result<()> {
//! let wiki = Wiki::new(Default::default()).await?;
//!
//! // Ingest a source document
//! wiki.ingest("docs/report.pdf").await?;
//!
//! // Query the knowledge base
//! let answer = wiki.query("What was Q1 revenue?").await?;
//! println!("{}", answer.answer);
//!
//! // Lint for health issues
//! let report = wiki.lint().await?;
//! println!("{:#?}", report);
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod graph;
pub mod ingest;
pub mod init;
pub mod lint;
pub mod llm;
pub mod query;
pub mod rebuild;
pub mod wiki;

pub use config::{Config, LlmProvider};
pub use graph::{GraphEdge, GraphNode, GraphResult};
pub use ingest::IngestResult;
pub use init::InitResult;
pub use lint::LintResult;
pub use query::QueryResult;
pub use rebuild::RebuildResult;
pub use wiki::{Status, Wiki};
