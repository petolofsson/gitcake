use std::{fs, path::PathBuf, sync::Arc};

use gitcake_core::{
    models::task::{TaskStatus, TaskType},
    repo::TaskRepo,
};
use rmcp::{
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;

// ── config ────────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct Config {
    repo_path: Option<String>,
}

fn load_repo() -> Result<TaskRepo, String> {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gitcake")
        .join("config.toml");

    let config: Config = if config_path.exists() {
        let text = fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
        toml::from_str(&text).unwrap_or_default()
    } else {
        Config::default()
    };

    let path = config
        .repo_path
        .ok_or("No repo configured. Run gitcake to set one up.")?;

    TaskRepo::open(&path).map_err(|e| e.to_string())
}

// ── MCP server ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct GitcakeMcp {
    repo: Arc<TaskRepo>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

// ── tool parameter types ──────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct ListSlicesArgs {
    /// List backlog slices instead of personal slices
    backlog: Option<bool>,
    /// Filter by status: open, in-progress, or done
    status: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct CreateSliceArgs {
    /// Slice title
    title: String,
    /// Slice type: task, bug, or incident (default: task)
    r#type: Option<String>,
    /// Assign to this username
    assignee: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct IdArg {
    /// 8-character hex slice ID
    id: String,
}

#[derive(Deserialize, JsonSchema)]
struct AssignSliceArgs {
    /// 8-character hex slice ID
    id: String,
    /// Username to assign to
    username: String,
}

// ── tools ─────────────────────────────────────────────────────────────────────

#[tool_router]
impl GitcakeMcp {
    fn new(repo: TaskRepo) -> Self {
        Self {
            repo: Arc::new(repo),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "List slices. Returns personal slices by default; pass backlog=true for the shared backlog. Optionally filter by status (open, in-progress, done).")]
    fn list_slices(&self, Parameters(args): Parameters<ListSlicesArgs>) -> Result<CallToolResult, McpError> {
        let (tasks, _) = if args.backlog.unwrap_or(false) {
            self.repo.list_backlog_tasks()
        } else {
            self.repo.list_tasks()
        }
        .map_err(mcp_err)?;

        let tasks: Vec<_> = match args.status.as_deref() {
            None => tasks,
            Some("open") => tasks.into_iter().filter(|t| t.status == TaskStatus::Open).collect(),
            Some("in-progress") => tasks.into_iter().filter(|t| t.status == TaskStatus::InProgress).collect(),
            Some("done") => tasks.into_iter().filter(|t| t.status == TaskStatus::Done).collect(),
            Some(s) => return Err(McpError::invalid_params(format!("unknown status: {s}"), None)),
        };

        let json = serde_json::to_string_pretty(&tasks).map_err(|e| mcp_err_str(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Create a new slice. Returns the created slice ID.")]
    fn create_slice(&self, Parameters(args): Parameters<CreateSliceArgs>) -> Result<CallToolResult, McpError> {
        let task_type = match args.r#type.as_deref().unwrap_or("task") {
            "task" => TaskType::Task,
            "bug" => TaskType::Bug,
            "incident" => TaskType::Incident,
            t => return Err(McpError::invalid_params(format!("unknown type: {t}"), None)),
        };
        let task = self.repo.create_task(args.title, task_type, None).map_err(mcp_err)?;
        if let Some(username) = args.assignee {
            self.repo.assign_task(&task.id, Some(username)).map_err(mcp_err)?;
        }
        Ok(CallToolResult::success(vec![Content::text(format!("created {}", task.id))]))
    }

    #[tool(description = "Set a slice in-progress.")]
    fn start_slice(&self, Parameters(args): Parameters<IdArg>) -> Result<CallToolResult, McpError> {
        self.repo.set_task_in_progress(&args.id).map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![Content::text(format!("{}: in-progress", args.id))]))
    }

    #[tool(description = "Mark a slice done.")]
    fn done_slice(&self, Parameters(args): Parameters<IdArg>) -> Result<CallToolResult, McpError> {
        self.repo.mark_task_done(&args.id).map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![Content::text(format!("{}: done", args.id))]))
    }

    #[tool(description = "Assign a slice to a user.")]
    fn assign_slice(&self, Parameters(args): Parameters<AssignSliceArgs>) -> Result<CallToolResult, McpError> {
        self.repo
            .assign_task(&args.id, Some(args.username.clone()))
            .map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "{}: assigned to {}",
            args.id, args.username
        ))]))
    }

    #[tool(description = "List all user folders in the gitcake repo.")]
    fn list_users(&self) -> Result<CallToolResult, McpError> {
        let users = self.repo.list_users().map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![Content::text(users.join("\n"))]))
    }

    #[tool(description = "Commit and push all local changes to the remote.")]
    fn sync(&self) -> Result<CallToolResult, McpError> {
        let out = self.repo.sync().map_err(mcp_err)?;
        let msg = if out.is_empty() { "synced".to_string() } else { out };
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }
}

#[tool_handler]
impl ServerHandler for GitcakeMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("gitcake-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("gitcake MCP server — read and write slices in a gitcake repo")
    }
}

fn mcp_err(e: gitcake_core::error::AppError) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

fn mcp_err_str(s: String) -> McpError {
    McpError::internal_error(s, None)
}

// ── entry point ───────────────────────────────────────────────────────────────

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let repo = match load_repo() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gitcake-mcp: {e}");
            std::process::exit(1);
        }
    };

    let service = GitcakeMcp::new(repo)
        .serve(rmcp::transport::stdio())
        .await
        .expect("failed to start MCP server");

    service.waiting().await.expect("server error");
}
