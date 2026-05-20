# wind-wiki — LLM-powered Wiki SDK

A Rust crate for building AI-native knowledge bases. Ingest documents, query your wiki with natural language, and lint for health issues — all powered by LLMs.

## Overview

```
源文档 (PDF/MD/HTML/TXT)
    ↓ Ingest Pipeline
LLM (Claude/GPT) — 结构化摘要 + 知识沉淀
    ↓
wiki/*.md — AI 生成的知识 Markdown
    ↓ Query Pipeline  
用户提问
    ↓ LLM 读取 wiki 内容 → 生成回答
```

## Three Pipelines

### 1. Ingest Pipeline
将源文档转换为结构化知识页面：
```
1. 读取源文件（PDF / Markdown / HTML / TXT / JSON / CSV）
2. 提取纯文本内容
3. 调用 LLM，发送：
   - system_md（AI 行为准则）
   - 源文档内容（截断至 8000 字符）
   - 任务指令（生成 wiki 页面）
4. LLM 生成 Markdown，写入 wiki/ 目录
```

### 2. Query Pipeline
自然语言问答：
```
1. 读取 wiki/ 下所有 .md 文件
2. 将内容注入 LLM prompt
3. LLM 综合知识回答问题
4. 返回回答 + 来源文件列表
```

### 3. Lint Pipeline
健康检查（静态分析，无需 LLM）：
- 断链检测（`[[wikilink]]` 指向不存在的文件）
- 重复标题检测（文件内 + 跨文件）
- 空文件告警
- 缺少 `<!-- source: ... -->` 元数据
- 内容过短占位符

## Configuration

**TOML 文件** (`~/.config/wind/config.toml`):
```toml
[wiki]
[wiki.llm]
provider = "anthropic"        # 或 "openai"
model = "claude-haiku-4-20250514"
api_key = "sk-ant-..."
# 可选：兼容 API（如 OpenRouter）
# base_url = "https://openrouter.ai/api/v1"

[wiki.paths]
wiki   = "~/.local/share/wind/wiki"     # 知识库目录
workspace = "~/.local/share/wind/workspace" # 源文件目录
system_md = "~/.config/wind/SYSTEM.md"   # AI 行为准则
```

**环境变量**（优先级更高）:
```bash
export WIND_WIKI_API_KEY="sk-ant-..."
export WIND_WIKI_MODEL="claude-haiku-4-20250514"
export WIND_WIKI_PROVIDER="anthropic"
```

## Rust API

```rust
use wind_wiki::Wiki;

let wiki = Wiki::new(Default::default()).await?;

// Ingest: 文档 → wiki 知识
let result = wiki.ingest("docs/report.pdf").await?;
println!("生成: {}", result.wiki_path);

// Query: 自然语言问答
let answer = wiki.query("Q1营收如何？").await?;
println!("{}", answer.answer);

// Lint: 健康检查
let report = wiki.lint().await?;
println!("{}", report.summary);

// Status: 统计
let status = wiki.status()?;
println!("共 {} 个知识页面", status.file_count);
```

## CLI

```bash
# 安装 SDK 后运行示例 CLI
cargo run --example cli -- ingest docs/report.pdf
cargo run --example cli -- query "Q1营收如何"
cargo run --example cli -- lint
cargo run --example cli -- status

# 或直接使用 wind-cli wiki 子命令
wind wiki ingest docs/report.pdf
wind wiki query "Q1营收如何"
wind wiki lint
wind wiki status
```

## Dependencies

```
wind-wiki
├── reqwest      (HTTP → Anthropic/OpenAI API)
├── lopdf       (PDF 文本提取)
├── scraper     (HTML 文本提取)
├── tokio       (async runtime)
├── directories (跨平台路径)
└── chrono      (时间戳)
```

## Directory Structure

```
~/.local/share/wind/
├── workspace/      ← 用户源文件
│   ├── greeting.txt
│   └── notes/
└── wiki/          ← AI 生成的知识 Markdown
    ├── Q1摘要.md   ← Ingest 生成
    └── SYSTEM.md   ← AI 行为准则
```

## License

MIT OR Apache-2.0
