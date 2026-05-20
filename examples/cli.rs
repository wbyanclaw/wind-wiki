//! Example CLI for llm-wiki-lib.
//!
//! Usage:
//!   cargo run --example cli -- ingest <file>
//!   cargo run --example cli -- query "what is X?"
//!   cargo run --example cli -- lint
//!   cargo run --example cli -- status
//!   cargo run --example cli -- graph [--json]
//!   cargo run --example cli -- rebuild --since <RFC3339> [--dry-run]
//!   cargo run --example cli -- init
//!   cargo run --example cli -- demo

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use llm_wiki_lib::{Config, Wiki};

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
    /// Build and display the wikilink knowledge graph.
    Graph {
        /// Output as JSON (machine-readable) instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Rebuild wiki entries for source files modified since a timestamp.
    Rebuild {
        /// ISO 8601 / RFC3339 timestamp (e.g. 2026-05-01T00:00:00Z).
        #[arg(long)]
        since: String,
        /// Show what would be rebuilt without rebuilding.
        #[arg(long)]
        dry_run: bool,
    },
    /// Initialise the wiki directory structure and default SYSTEM.md.
    Init,
    /// Run a built-in demo: ingest a sample, query it, show the graph.
    Demo,
}

fn parse_datetime(s: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                .map(|ndt| chrono::DateTime::from_naive_utc_and_offset(ndt, chrono::Utc))
        })
        .unwrap_or_else(|_| {
            // Try date only
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map(|ndt| {
                    chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                        ndt.and_hms_opt(0, 0, 0).unwrap(),
                        chrono::Utc,
                    )
                })
                .expect("invalid date format; use RFC3339 like 2026-05-01T00:00:00Z")
        })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("wind_wiki=info".parse().unwrap()))
        .init();

    let cli = Cli::parse();
    let config = Config::load().unwrap_or_default();

    match &cli.command {
        Command::Ingest { file } => {
            let wiki = Wiki::new(config).await?;
            let result = wiki.ingest(file.to_string_lossy().as_ref()).await?;
            println!(
                "✅ Ingested: {} → {} ({} chars → {} chars)",
                result.source, result.wiki_path, result.source_chars, result.wiki_chars
            );
        }

        Command::Query { question } => {
            let wiki = Wiki::new(config).await?;
            let result = wiki.query(question).await?;
            if result.no_wiki_content {
                println!("📭 知识库为空，请先 Ingest 文档。");
            } else {
                println!("## 回答\n\n{}", result.answer);
                println!("\n_来源: {} 个文件_", result.sources.len());
            }
        }

        Command::Lint => {
            let wiki = Wiki::new(config).await?;
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
            let wiki = Wiki::new_local(config)?;
            let status = wiki.status()?;
            println!(
                "📚 Wiki 状态\n  目录: {}\n  文件数: {}\n  总大小: {} bytes",
                status.wiki_dir.display(),
                status.file_count,
                status.total_bytes
            );
        }

        Command::Graph { json } => {
            let wiki = Wiki::new_local(config)?;
            let result = wiki.graph()?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print_graph_text(&result);
            }
        }

        Command::Rebuild { since, dry_run } => {
            let wiki = Wiki::new(config).await?;
            let since_dt = parse_datetime(since);
            let result = wiki.rebuild_since(since_dt, *dry_run).await?;
            if result.files.is_empty() {
                println!("📭 没有找到自 {} 以来修改的源文件。", result.since);
            } else {
                println!(
                    "📦 Rebuild（{}），自 {} 以来：",
                    if result.dry_run { "dry-run" } else { "live" },
                    result.since
                );
                for f in &result.files {
                    let icon = match f.status {
                        wind_wiki::rebuild::RebuildStatus::Rebuilt => "🔄",
                        wind_wiki::rebuild::RebuildStatus::Skipped => "⏭️ ",
                        wind_wiki::rebuild::RebuildStatus::Failed => "❌",
                    };
                    println!("{} {} | {}", icon, f.source, f.modified_at);
                }
                println!(
                    "\n共 {} 个文件，{} 重建，{} 跳过",
                    result.files.len(),
                    result.rebuilt_count,
                    result.skipped_count
                );
            }
        }

        Command::Init => {
            let wiki = Wiki::new_local(config)?;
            let result = wiki.init()?;
            println!("✅ Wiki 初始化完成！");
            for path in &result.created {
                println!("  创建: {}", path);
            }
            println!(
                "\n📁 Wiki 目录: {}\n📁 源文件目录: {}\n📄 SYSTEM.md: {}",
                result.wiki_dir, result.workspace_dir, result.system_md
            );
            println!("\n下一步：设置 API key（环境变量 WIND_WIKI_API_KEY），然后运行：");
            println!("  wind wiki ingest <源文件>    # 导入文档");
            println!("  wind wiki query \"你的问题\"    # 问答");
            println!("  wind wiki graph              # 查看知识图谱");
        }

        Command::Demo => {
            run_demo().await?;
        }
    }

    Ok(())
}

/// Print the graph result as human-readable text.
fn print_graph_text(result: &wind_wiki::GraphResult) {
    if result.nodes.is_empty() {
        println!("📭 知识库为空，暂无图谱。");
        return;
    }

    println!("🕸️  Wiki 知识图谱\n");
    println!(
        "  {} 个节点，{} 条边\n",
        result.node_count, result.edge_count
    );

    // Show most-connected pages first (highest backlinks)
    println!("### 核心页面（最多反向链接）");
    for node in result.nodes.iter().filter(|n| n.backlinks > 0).take(10) {
        println!("  📄 {} ← {} 个页面链接", node.name, node.backlinks);
    }

    // Show orphan links (linked but don't exist)
    if !result.orphans.is_empty() {
        println!("\n⚠️  断链（被链接但文件不存在）");
        for orphan in &result.orphans {
            println!("  ❌ [[{}]]", orphan);
        }
    }

    // Show edges
    println!("\n### 所有链接关系");
    for edge in &result.edges {
        println!("  [[{}]] → [[{}]]", edge.from, edge.to);
    }
}

/// Run an interactive demo with a built-in example.
async fn run_demo() -> anyhow::Result<()> {
    use std::io::Write;

    println!("🎯 llm-wiki-lib 演示开始！\n");
    println!("本演示将在临时目录中创建一个示例 wiki，无需 API key。\n");

    // Create a temp workspace with sample files
    let temp_dir = tempfile::tempdir()?;
    let workspace = temp_dir.path().join("workspace");
    let wiki_dir = temp_dir.path().join("wiki");
    std::fs::create_dir_all(&workspace)?;
    std::fs::create_dir_all(&wiki_dir)?;

    // Write sample source file
    let sample_content = r#"# Q1 2026 季度报告

## 营收
Q1 总营收为 1.2 亿元，同比增长 30%。

## 产品
核心产品 A 贡献了 70% 的营收。
新产品 B 处于市场推广期。

## 市场
国内市场份额达到 15%。
海外市场开始起步，主要在欧洲。
"#;
    let source_path = workspace.join("Q1_报告.md");
    std::fs::write(&source_path, sample_content)?;

    // Write SYSTEM.md
    let system_md = temp_dir.path().join("SYSTEM.md");
    std::fs::write(
        &system_md,
        r#"你是一个专业的知识库助手。
- 每个页面用 [[主题名]] 链接相关主题
- 使用中文撰写
"#,
    )?;

    println!("📄 已创建示例源文件: {}", source_path.display());
    println!("   内容: Q1 季度营收报告（营收/产品/市场）\n");

    print!("🤖 是否使用模拟 LLM 运行 ingest？(y/n) ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim() == "y" || input.trim().is_empty() {
        // Simulate ingest by just copying the source to wiki with wikilinks
        let wiki_content = format!(
            r#"<!-- source: Q1_报告.md -->
<!-- generated: {} -->

# Q1 2026 季度报告

## 营收
Q1 总营收为 1.2 亿元，同比增长 30%。

## 产品
核心产品 A 贡献了 70% 的营收。
新产品 B 处于市场推广期。

## 市场
国内市场份额达到 15%。
海外市场开始起步，主要在欧洲。

## 相关主题
[[Q1产品分析]] [[市场竞争格局]]
"#,
            chrono::Utc::now().to_rfc3339()
        );
        let wiki_path = wiki_dir.join("Q1_2026_季度报告.md");
        std::fs::write(&wiki_path, wiki_content)?;

        // Create one linked page
        let linked_content = r#"<!-- source: synthetic -->
# Q1产品分析

核心产品 A Q1 贡献了 70% 营收，处于成熟期。
新产品 B 处于推广期，预计 Q2 增长。

## 相关主题
[[Q1_2026_季度报告]]
"#;
        std::fs::write(wiki_dir.join("Q1产品分析.md"), linked_content)?;

        println!("✅ 模拟 Ingest 完成！\n");
    }

    // Show graph
    let config = Config {
        paths: wind_wiki::config::Paths {
            workspace,
            wiki: wiki_dir,
            system_md,
        },
        ..Default::default()
    };
    let wiki = Wiki::new_local(config)?;
    let graph = wiki.graph()?;
    print_graph_text(&graph);

    println!("\n🎉 演示完成！");
    println!("运行 `wind wiki init` 开始你的知识库之旅。");

    Ok(())
}
