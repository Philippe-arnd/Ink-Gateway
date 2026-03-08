// The MCP server compiles all shared modules but intentionally exposes only a
// subset of their functions. Suppress dead-code lint for this binary target.
#![allow(dead_code)]

mod book;
mod config;
mod context;
mod git;
mod init;
mod maintenance;
mod state;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

// ── JSON-RPC 2.0 types ──────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct RpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl RpcResponse {
    fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn err(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

// ── Tool schema ─────────────────────────────────────────────────────────────

fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "session_open",
                "description": "Open a writing session: pre-flight git sync, snapshot tag, draft branch, load all book context. Returns a full JSON payload ready for the writing engine.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": {
                            "type": "string",
                            "description": "Absolute path to the book repository"
                        }
                    },
                    "required": ["repo_path"]
                }
            },
            {
                "name": "session_close",
                "description": "Close a writing session: split current.md (validated prose → Full_Book.md, new prose → current.md), update Summary.md, write Changelog entry, push. Returns word counts and completion_ready flag.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": {
                            "type": "string",
                            "description": "Absolute path to the book repository"
                        },
                        "prose": {
                            "type": "string",
                            "description": "New prose for this session — reworked blocks and new continuation, wrapped in INK:REWORKED/INK:NEW markers"
                        },
                        "summary": {
                            "type": "string",
                            "description": "One-paragraph narrative summary of this session"
                        },
                        "human_edits": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Filenames the human edited between sessions (from session_open payload)"
                        }
                    },
                    "required": ["repo_path", "prose"]
                }
            },
            {
                "name": "complete",
                "description": "Attempt to finalise the book. If current.md contains pending INK instructions, returns needs_revision. If clean, appends to Full_Book.md, writes the COMPLETE marker, and pushes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": {
                            "type": "string",
                            "description": "Absolute path to the book repository"
                        }
                    },
                    "required": ["repo_path"]
                }
            },
            {
                "name": "advance_chapter",
                "description": "Advance to the next chapter. Verifies the next chapter outline file exists (returns needs_chapter_outline if missing), updates .ink-state.yml, and commits. Does NOT push.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": {
                            "type": "string",
                            "description": "Absolute path to the book repository"
                        }
                    },
                    "required": ["repo_path"]
                }
            },
            {
                "name": "init",
                "description": "Scaffold a new book repository with all required files and directories. Returns a JSON payload containing a `questions` array. IMPORTANT — after calling this tool you MUST: (1) Act as the interactive setup wizard: ask the user each question in order, show the `hint` as context, respect `options` for select-type questions, wait for each answer before moving on. (2) Once all answers are collected, extrapolate each brief answer into rich, detailed content — do not copy the raw answer verbatim; infer characters' deeper motivations, expand the world-building, flesh out the plot arc beats, enrich the prose style description, etc. (3) Read each template file first (they contain structured sections with `[...]` placeholders), then replace ONLY the placeholder sections with the extrapolated content. CRITICAL: preserve the template structure exactly — do not add, remove, or rename any section heading. File mapping: Q1–4 → update specific fields in Config.yml (language, target_length=pages×250, words_per_session=pages×250, chapter_count=ceil(target_length/3000)); Q5–6 → Soul.md; Q7–8 → Characters.md; Q9–11 → Outline.md; Q12 → Lore.md; Q13 → Chapter_01.md. (4) After all files are written: git -C <repo_path> add -A && git -C <repo_path> commit -m 'init: populate global material from author Q&A' && git -C <repo_path> push origin main. Notify the author the book is ready.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": {
                            "type": "string",
                            "description": "Absolute path to an existing git repository"
                        },
                        "title": {
                            "type": "string",
                            "description": "Book title (default: Untitled)"
                        },
                        "author": {
                            "type": "string",
                            "description": "Author name (default: Unknown)"
                        }
                    },
                    "required": ["repo_path"]
                }
            },
            {
                "name": "seed",
                "description": "Write CLAUDE.md and GEMINI.md bootstrap files to an empty repo so any AI agent can auto-detect the Ink Gateway framework and run init. Idempotent.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": {
                            "type": "string",
                            "description": "Absolute path to an existing git repository"
                        }
                    },
                    "required": ["repo_path"]
                }
            },
            {
                "name": "status",
                "description": "Return a lightweight read-only snapshot of the book's current state: chapter, word counts, lock status, and completion flags. No git operations — reads local files only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": {
                            "type": "string",
                            "description": "Absolute path to the book repository"
                        }
                    },
                    "required": ["repo_path"]
                }
            },
            {
                "name": "update_agents",
                "description": "Refresh AGENTS.md (and CLAUDE.md/GEMINI.md if present) with the latest engine instructions embedded in this ink-gateway-mcp build. Commits and pushes. Idempotent.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": {
                            "type": "string",
                            "description": "Absolute path to the book repository"
                        }
                    },
                    "required": ["repo_path"]
                }
            },
            {
                "name": "doctor",
                "description": "Validate the book repository: checks required files, Config.yml validity, git remote configuration and reachability, draft branch, and session lock state. Returns a list of named checks each with ok/detail. Run this before registering a cron job.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": {
                            "type": "string",
                            "description": "Absolute path to the book repository"
                        }
                    },
                    "required": ["repo_path"]
                }
            },
            {
                "name": "apply_format",
                "description": "Apply format patches to Full_Book.md (title, author, missing chapter headings). Accepts a 'patch' object with optional 'prepend' string and 'insert_headings' array of {before_anchor, heading}. Commits and pushes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": {
                            "type": "string",
                            "description": "Absolute path to the book repository"
                        },
                        "patch": {
                            "type": "object",
                            "description": "Format patch: optional 'prepend' string inserted after the managed header; optional 'insert_headings' array of {before_anchor, heading} objects"
                        }
                    },
                    "required": ["repo_path", "patch"]
                }
            }
        ]
    })
}

// ── Tool dispatch ────────────────────────────────────────────────────────────

fn call_tool(name: &str, args: &Value) -> Result<Value, String> {
    let repo_path = args
        .get("repo_path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or("Missing required parameter: repo_path")?;

    match name {
        "session_open" => {
            let payload = context::session_open(&repo_path).map_err(|e| e.to_string())?;
            serde_json::to_value(payload).map_err(|e| e.to_string())
        }

        "session_close" => {
            let prose = args
                .get("prose")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: prose")?;
            let summary = args.get("summary").and_then(|v| v.as_str());
            let human_edits: Vec<String> = args
                .get("human_edits")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let payload = maintenance::close_session(&repo_path, prose, summary, &human_edits)
                .map_err(|e| e.to_string())?;
            serde_json::to_value(payload).map_err(|e| e.to_string())
        }

        "complete" => maintenance::complete_session(&repo_path).map_err(|e| e.to_string()),

        "advance_chapter" => maintenance::advance_chapter(&repo_path).map_err(|e| e.to_string()),

        "init" => {
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled");
            let author = args
                .get("author")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let payload = init::run_init(&repo_path, title, author).map_err(|e| e.to_string())?;
            serde_json::to_value(payload).map_err(|e| e.to_string())
        }

        "seed" => {
            let payload = init::run_seed(&repo_path).map_err(|e| e.to_string())?;
            serde_json::to_value(payload).map_err(|e| e.to_string())
        }

        "status" => maintenance::book_status(&repo_path).map_err(|e| e.to_string()),

        "update_agents" => init::update_agents(&repo_path).map_err(|e| e.to_string()),

        "doctor" => maintenance::doctor(&repo_path).map_err(|e| e.to_string()),

        "apply_format" => {
            let patch = args
                .get("patch")
                .cloned()
                .unwrap_or(serde_json::Value::Object(Default::default()));
            book::apply_format_patch(&repo_path, patch).map_err(|e| e.to_string())
        }

        _ => Err(format!("Unknown tool: {name}")),
    }
}

// ── Transport: newline-delimited JSON-RPC over stdio ────────────────────────

fn send(resp: &RpcResponse) {
    let line = serde_json::to_string(resp).expect("serialization cannot fail");
    println!("{line}");
    if let Err(e) = io::stdout().flush() {
        eprintln!("ink-gateway-mcp: stdout flush error: {e}");
    }
}

fn main() {
    // All logging goes to stderr so stdout remains clean JSON-RPC
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if l.trim().is_empty() => continue,
            Ok(l) => l,
            Err(e) => {
                eprintln!("ink-gateway-mcp: stdin error: {e}");
                break;
            }
        };

        let req: RpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                send(&RpcResponse::err(
                    Value::Null,
                    -32700,
                    format!("Parse error: {e}"),
                ));
                continue;
            }
        };

        let id = req.id.clone().unwrap_or(Value::Null);

        match req.method.as_str() {
            "initialize" => {
                send(&RpcResponse::ok(
                    id,
                    json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": { "tools": {} },
                        "serverInfo": {
                            "name": "ink-gateway",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                ));
            }

            // Notification — no response
            "notifications/initialized" => {}

            "tools/list" => {
                send(&RpcResponse::ok(id, tools_list()));
            }

            "tools/call" => {
                let params = req.params.as_ref().unwrap_or(&Value::Null);
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").unwrap_or(&Value::Null);

                let (content_text, is_error) = match call_tool(name, args) {
                    Ok(result) => (
                        serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string()),
                        false,
                    ),
                    Err(e) => (e, true),
                };

                send(&RpcResponse::ok(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": content_text }],
                        "isError": is_error
                    }),
                ));
            }

            _ => {
                send(&RpcResponse::err(
                    id,
                    -32601,
                    format!("Method not found: {}", req.method),
                ));
            }
        }
    }
}
