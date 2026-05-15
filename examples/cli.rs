//! Example CLI for wind-wiki.
//!
//! Usage:
//!   cargo run --example cli -- ingest <file>
//!   cargo run --example cli -- query "what is X?"
//!   cargo run --example cli -- lint
//!   cargo run --example cli -- status

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use wind_wiki::{Config, Wiki};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Ingest a source file into the wiki.
    Ingest {
        /// Path to the source file.
        file: PathBuf,
    },
    /// Query the wiki with a question.
    Query {
        /// The question to ask.
        question: String,
    },
    /// Lint the wiki for health issues.
    Lint,
    /// Show wiki status.
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("wind_wiki=info".parse()?))
        .init();

    let cli = Cli::parse();

    let config = Config::load().unwrap_or_default();
    let wiki = Wiki::new(config).await?;

    match cli.command {
        Command::Ingest { file } => {
            let result = wiki.ingest(file.to_string_lossy().as_ref()).await?;
            println!(
                "✅ Ingested: {} → {} ({} chars → {} chars)",
                result.source, result.wiki_path, result.source_chars, result.wiki_chars
            );
        }
        Command::Query { question } => {
            let result = wiki.query(&question).await?;
            if result.no_wiki_content {
                println!("📭 知识库为空，请先 Ingest 文档。");
            } else {
                println!("## 回答\n\n{}", result.answer);
                println!("\n_来源: {} 个文件_", result.sources.len());
            }
        }
        Command::Lint => {
            let result = wiki.lint().await?;
            println!("## Lint 结果\n");
            println!("文件数: {}", result.file_count);
            println!("{}", result.summary);
            if !result.issues.is_empty() {
                println!("\n### 问题列表");
                for issue in &result.issues {
                    let severity = match issue.severity {
                        wind_wiki::lint::IssueSeverity::Error => "❌",
                        wind_wiki::lint::IssueSeverity::Warning => "⚠️",
                        wind_wiki::lint::IssueSeverity::Info => "ℹ️",
                    };
                    println!("{} [{}] {}", severity, issue.file, issue.message);
                }
            }
        }
        Command::Status => {
            let status = wiki.status()?;
            println!(
                "📚 Wiki 状态\n  目录: {}\n  文件数: {}\n  总大小: {} bytes",
                status.wiki_dir.display(),
                status.file_count,
                status.total_bytes
            );
        }
    }

    Ok(())
}
