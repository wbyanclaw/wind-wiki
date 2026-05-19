//! wind-wiki — LLM-powered Wiki SDK
//!
//! Provides three pipelines:
//! - **Ingest**: Source file → LLM edit → Wiki Markdown
//! - **Query**: Question → LLM search wiki → Answer
//! - **Lint**: Audit wiki health → Fix deadlinks / contradictions
//!
//! ## Quick start
//!
//! ```no_run
//! use wind_wiki::Wiki;
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
pub mod wiki;
pub mod llm;
pub mod ingest;
pub mod query;
pub mod lint;
pub mod graph;
pub mod rebuild;
pub mod init;

pub use config::{Config, LlmProvider};
pub use wiki::{Wiki, Status};
pub use ingest::IngestResult;
pub use query::QueryResult;
pub use lint::LintResult;
pub use graph::{GraphResult, GraphNode, GraphEdge};
pub use rebuild::RebuildResult;
pub use init::InitResult;
