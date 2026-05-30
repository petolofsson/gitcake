use clap::{Parser, Subcommand, ValueEnum};
use gitcake_core::{
    models::task::{Task, TaskStatus, TaskType},
    repo::TaskRepo,
};

use crate::config::Config;

#[derive(Parser)]
#[command(name = "gitcake", about = "Terminal slice tracker backed by git")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// List slices
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Filter by status
        #[arg(long)]
        status: Option<CliStatus>,
        /// Show backlog instead of personal slices
        #[arg(long)]
        backlog: bool,
    },
    /// Create a new slice
    Create {
        /// Slice title
        title: String,
        /// Slice type
        #[arg(long, default_value = "task")]
        r#type: CliType,
        /// Assign to this username
        #[arg(long)]
        assign: Option<String>,
    },
    /// Set a slice in-progress
    Start { id: String },
    /// Mark a slice done
    Done { id: String },
    /// Delete a slice
    Delete { id: String },
    /// Assign a slice to a user
    Assign {
        id: String,
        #[arg(long)]
        to: String,
    },
    /// Commit and push all changes
    Sync,
}

#[derive(ValueEnum, Clone)]
pub enum CliStatus {
    Open,
    #[value(name = "in-progress")]
    InProgress,
    Done,
}

#[derive(ValueEnum, Clone)]
pub enum CliType {
    Task,
    Bug,
    Incident,
}

impl From<CliType> for TaskType {
    fn from(t: CliType) -> Self {
        match t {
            CliType::Task => TaskType::Task,
            CliType::Bug => TaskType::Bug,
            CliType::Incident => TaskType::Incident,
        }
    }
}

pub fn run(cli: Cli) -> Result<(), String> {
    let config = Config::load();
    let repo_path = config
        .repo_path
        .ok_or("No repo configured. Run gitcake without arguments to set one up.")?;
    let repo = TaskRepo::open(&repo_path).map_err(|e| e.to_string())?;

    match cli.command {
        Command::List { json, status, backlog } => {
            let (tasks, _) = if backlog {
                repo.list_backlog_tasks().map_err(|e| e.to_string())?
            } else {
                repo.list_tasks().map_err(|e| e.to_string())?
            };
            let tasks = filter_by_status(tasks, status);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&tasks).map_err(|e| e.to_string())?
                );
            } else {
                print_task_table(&tasks);
            }
        }

        Command::Create { title, r#type, assign } => {
            let task = repo
                .create_task(title, r#type.into(), None)
                .map_err(|e| e.to_string())?;
            if let Some(username) = assign {
                repo.assign_task(&task.id, Some(username))
                    .map_err(|e| e.to_string())?;
            }
            println!("created {}", task.id);
        }

        Command::Start { id } => {
            repo.set_task_in_progress(&id).map_err(|e| e.to_string())?;
            println!("{id}: in-progress");
        }

        Command::Done { id } => {
            repo.mark_task_done(&id).map_err(|e| e.to_string())?;
            println!("{id}: done");
        }

        Command::Delete { id } => {
            repo.delete_task(&id).map_err(|e| e.to_string())?;
            println!("{id}: deleted");
        }

        Command::Assign { id, to } => {
            repo.assign_task(&id, Some(to.clone()))
                .map_err(|e| e.to_string())?;
            println!("{id}: assigned to {to}");
        }

        Command::Sync => {
            let out = repo.sync().map_err(|e| e.to_string())?;
            if !out.is_empty() {
                println!("{out}");
            }
        }
    }

    Ok(())
}

fn filter_by_status(tasks: Vec<Task>, status: Option<CliStatus>) -> Vec<Task> {
    let Some(s) = status else { return tasks };
    let target = match s {
        CliStatus::Open => TaskStatus::Open,
        CliStatus::InProgress => TaskStatus::InProgress,
        CliStatus::Done => TaskStatus::Done,
    };
    tasks.into_iter().filter(|t| t.status == target).collect()
}

fn print_task_table(tasks: &[Task]) {
    for task in tasks {
        let status = match task.status {
            TaskStatus::Open => "open       ",
            TaskStatus::InProgress => "in-progress",
            TaskStatus::Done => "done       ",
        };
        let type_str = match task.task_type {
            TaskType::Task => "task    ",
            TaskType::Bug => "bug     ",
            TaskType::Incident => "incident",
        };
        let assignee = task.assignee.as_deref().unwrap_or("");
        if assignee.is_empty() {
            println!("{}  {}  {}  {}", task.id, type_str, status, task.title);
        } else {
            println!("{}  {}  {}  {}  → {}", task.id, type_str, status, task.title, assignee);
        }
    }
}
