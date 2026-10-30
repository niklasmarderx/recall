//! `recall` command-line interface and MCP entry point.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use recall::mcp;
use recall::store::Store;

#[derive(Parser)]
#[command(
    name = "recall",
    version,
    about = "Local-first long-term memory for AI coding agents (MCP server + CLI)"
)]
struct Cli {
    /// Path to the SQLite database (default: ~/.recall/memory.db)
    #[arg(long, global = true)]
    db: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Store a new memory
    Add {
        /// The memory text
        content: String,
        /// Comma-separated tags
        #[arg(long, default_value = "")]
        tags: String,
        /// Where this memory came from (project, agent, ...)
        #[arg(long, default_value = "")]
        source: String,
    },
    /// Search memories by keyword
    Search {
        /// The search query
        query: String,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// List recent memories
    List {
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Delete a memory by id
    Delete {
        /// The memory id
        id: i64,
    },
    /// Run as an MCP server over stdio
    Serve,
}

fn default_db() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".recall")
        .join("memory.db")
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db = cli.db.unwrap_or_else(default_db);
    let store = Store::open(&db)?;

    match cli.command {
        Command::Add {
            content,
            tags,
            source,
        } => {
            let id = store.add(&content, &tags, &source)?;
            println!("Stored memory #{id}.");
        }
        Command::Search { query, limit } => {
            let memories = store.search(&query, limit)?;
            if memories.is_empty() {
                println!("No memories found.");
            }
            for m in memories {
                println!("#{} [{}]: {}", m.id, m.tags, m.content);
            }
        }
        Command::List { limit } => {
            for m in store.list(limit)? {
                println!("#{} [{}]: {}", m.id, m.tags, m.content);
            }
        }
        Command::Delete { id } => {
            if store.delete(id)? {
                println!("Deleted memory #{id}.");
            } else {
                println!("No memory #{id}.");
            }
        }
        Command::Serve => mcp::serve(store)?,
    }
    Ok(())
}
