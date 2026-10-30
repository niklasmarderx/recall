//! Minimal Model Context Protocol server over stdio.
//!
//! Implements just enough of MCP (JSON-RPC 2.0, newline-delimited) for a
//! coding agent to discover and call the memory tools.

use crate::store::{Memory, Store};
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

/// Run the MCP server, reading requests from stdin and writing replies to stdout.
pub fn serve(store: Store) -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut out = stdout.lock();
    let mut line = String::new();

    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break; // EOF: client closed the pipe
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(response) = handle(&store, &request) {
            out.write_all(serde_json::to_string(&response)?.as_bytes())?;
            out.write_all(b"\n")?;
            out.flush()?;
        }
    }
    Ok(())
}

fn handle(store: &Store, request: &Value) -> Option<Value> {
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let id = request.get("id").cloned();

    match method {
        "initialize" => Some(ok(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "recall", "version": env!("CARGO_PKG_VERSION") }
            }),
        )),
        "notifications/initialized" => None,
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tool_defs() }))),
        "tools/call" => Some(handle_tool_call(store, id, request)),
        _ => id.map(|id| err(Some(id), -32601, "Method not found")),
    }
}

fn handle_tool_call(store: &Store, id: Option<Value>, request: &Value) -> Value {
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let outcome: Result<String> = match name {
        "memory_add" => {
            let content = args
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if content.trim().is_empty() {
                Err(anyhow::anyhow!("`content` is required"))
            } else {
                let tags = args.get("tags").and_then(Value::as_str).unwrap_or("");
                let source = args.get("source").and_then(Value::as_str).unwrap_or("");
                store
                    .add(&content, tags, source)
                    .map(|id| format!("Stored memory #{id}."))
            }
        }
        "memory_search" => {
            let query = args.get("query").and_then(Value::as_str).unwrap_or("");
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(5) as usize;
            store.search(query, limit).map(|m| format_memories(&m))
        }
        "memory_list" => {
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(10) as usize;
            store.list(limit).map(|m| format_memories(&m))
        }
        "memory_delete" => {
            let mid = args.get("id").and_then(Value::as_i64).unwrap_or(-1);
            store.delete(mid).map(|removed| {
                if removed {
                    format!("Deleted memory #{mid}.")
                } else {
                    format!("No memory #{mid}.")
                }
            })
        }
        other => Err(anyhow::anyhow!("unknown tool: {other}")),
    };

    match outcome {
        Ok(text) => ok(id, json!({ "content": [{ "type": "text", "text": text }] })),
        Err(e) => ok(
            id,
            json!({ "content": [{ "type": "text", "text": format!("Error: {e}") }], "isError": true }),
        ),
    }
}

fn tool_defs() -> Value {
    json!([
        {
            "name": "memory_add",
            "description": "Store a new memory (a fact, preference, or note) for long-term recall.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "The memory text to store." },
                    "tags": { "type": "string", "description": "Optional comma-separated tags." },
                    "source": { "type": "string", "description": "Optional origin, e.g. project or agent name." }
                },
                "required": ["content"]
            }
        },
        {
            "name": "memory_search",
            "description": "Search stored memories by keyword and return the most relevant matches.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "The search query." },
                    "limit": { "type": "integer", "description": "Maximum number of results (default 5)." }
                },
                "required": ["query"]
            }
        },
        {
            "name": "memory_list",
            "description": "List the most recently stored memories.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Maximum number of results (default 10)." }
                }
            }
        },
        {
            "name": "memory_delete",
            "description": "Delete a stored memory by its id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer", "description": "The id of the memory to delete." }
                },
                "required": ["id"]
            }
        }
    ])
}

fn format_memories(memories: &[Memory]) -> String {
    if memories.is_empty() {
        return "No memories found.".to_string();
    }
    memories
        .iter()
        .map(|m| {
            let tags = if m.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", m.tags)
            };
            format!("#{}{}: {}", m.id, tags, m.content)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn ok(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result })
}

fn err(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "error": { "code": code, "message": message } })
}
