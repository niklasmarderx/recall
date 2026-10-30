# recall

**Local-first long-term memory for AI coding agents, exposed over the Model Context Protocol (MCP).**

[![CI](https://github.com/niklasmarderx/recall/actions/workflows/ci.yml/badge.svg)](https://github.com/niklasmarderx/recall/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Coding agents forget everything between sessions. `recall` gives them a persistent,
local memory: a single Rust binary that stores facts, preferences and notes in a
SQLite database and lets any MCP-capable agent — Claude Code, Cursor, Codex — write
to it and search it. No cloud, no API keys, no vendor lock-in. Your memory stays on
your machine and is shared across every agent you use.

## Why

- **Local-first.** Everything lives in one SQLite file. Nothing leaves your machine.
- **Vendor-neutral.** One memory, shared by every agent that speaks MCP.
- **Fast and small.** A single statically-linked Rust binary with no runtime.
- **Real search.** SQLite FTS5 full-text search with BM25 ranking out of the box.

## Install

```sh
git clone https://github.com/niklasmarderx/recall
cd recall
cargo install --path .
```

## Use it as a CLI

```sh
recall add "Niklas prefers Rust for systems work" --tags prefs --source profile
recall search "rust"
recall list --limit 20
recall delete 3
```

The database lives at `~/.recall/memory.db` by default; override it with `--db <path>`.

## Use it as an MCP server

`recall serve` speaks MCP over stdio. Register it with your agent — for Claude Code,
add this to your MCP config:

```json
{
  "mcpServers": {
    "recall": { "command": "recall", "args": ["serve"] }
  }
}
```

The agent then gets four tools: `memory_add`, `memory_search`, `memory_list` and
`memory_delete`. It can remember things you tell it and recall them in any later
session, in any tool.

## How it works

Memories are rows in a SQLite table, mirrored into an FTS5 virtual table via
triggers. Search builds a safe prefix query and ranks results with BM25. The MCP
layer is a dependency-free JSON-RPC 2.0 loop over stdin/stdout, so there is nothing
to audit beyond the standard library and `serde`.

## Roadmap

- **Semantic search** — optional local embeddings (via `fastembed`) with cosine
  ranking, blended with the lexical BM25 score.
- **Scoping** — per-project and per-source namespaces.
- **Decay & summarisation** — age out or compress stale memories automatically.
- **Import/export** — portable JSON dumps.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
