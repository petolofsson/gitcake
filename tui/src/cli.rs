use clap::{Parser, Subcommand, ValueEnum};
use gitcake_core::{
    models::task::{NewTask, Priority, Task, TaskPatch, TaskStatus, TaskType},
    repo::TaskRepo,
};

use crate::config::Config;

#[derive(Parser)]
#[command(name = "gitcake", about = "Terminal slice tracker backed by git")]
pub struct Cli {
    /// Open a specific repo by path (session only — does not change saved config)
    #[arg(long, value_name = "PATH")]
    pub repo: Option<String>,

    /// Start fresh — show repo setup screen regardless of saved config
    #[arg(long)]
    pub new: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
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
        /// Priority: high, normal, or low
        #[arg(long, default_value = "normal")]
        priority: CliPriority,
        /// Sequence number for ordering within a plan
        #[arg(long)]
        order: Option<u32>,
        /// ID of a parent slice
        #[arg(long)]
        parent: Option<String>,
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
    /// Show a single slice (full detail including description)
    Show {
        id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Edit a slice title and/or description
    Set {
        id: String,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New description body
        #[arg(long)]
        description: Option<String>,
        /// New priority: high, normal, or low
        #[arg(long)]
        priority: Option<CliPriority>,
        /// Mark the slice as blocked (needs human input)
        #[arg(long)]
        block: bool,
        /// Clear the blocked flag
        #[arg(long)]
        unblock: bool,
        /// Sequence number for ordering within a plan
        #[arg(long)]
        order: Option<u32>,
        /// ID of a parent slice (use empty string to clear)
        #[arg(long)]
        parent: Option<String>,
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

#[derive(ValueEnum, Clone)]
pub enum CliPriority {
    High,
    Normal,
    Low,
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

impl From<CliPriority> for Priority {
    fn from(p: CliPriority) -> Self {
        match p {
            CliPriority::High => Priority::High,
            CliPriority::Normal => Priority::Normal,
            CliPriority::Low => Priority::Low,
        }
    }
}

pub fn run(command: Command, repo_flag: Option<String>) -> Result<(), String> {
    let config = Config::load();
    let repo_path = repo_flag
        .or(config.repo_path)
        .ok_or("No repo configured. Run gitcake without arguments to set one up.")?;
    let repo = TaskRepo::open(&repo_path).map_err(|e| e.to_string())?;

    match command {
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

        Command::Create { title, r#type, assign, priority, order, parent } => {
            let task = repo
                .create_task(NewTask {
                    title,
                    task_type: r#type.into(),
                    priority: priority.into(),
                    order,
                    parent_id: parent,
                    ..Default::default()
                })
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

        Command::Show { id, json } => {
            let task = repo.get_task(&id).map_err(|e| e.to_string())?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&task).map_err(|e| e.to_string())?
                );
            } else {
                print_task_detail(&task);
            }
        }

        Command::Set { id, title, description, priority, block, unblock, order, parent } => {
            if title.is_none() && description.is_none() && priority.is_none()
                && !block && !unblock && order.is_none() && parent.is_none()
            {
                return Err("provide at least one field to update".into());
            }
            if block && unblock {
                return Err("--block and --unblock are mutually exclusive".into());
            }
            let blocked = if block { Some(true) } else if unblock { Some(false) } else { None };
            let description = description.map(|d| if d.is_empty() { None } else { Some(d) });
            let order = order.map(Some);
            let parent_id = parent.map(|p| if p.is_empty() { None } else { Some(p) });
            let task = repo
                .update_task(&id, TaskPatch {
                    title,
                    description,
                    priority: priority.map(Into::into),
                    blocked,
                    order,
                    parent_id,
                    cake_id: None,
                })
                .map_err(|e| e.to_string())?;
            println!("{}: updated", task.id);
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

fn print_task_detail(task: &Task) {
    let status = match task.status {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in-progress",
        TaskStatus::Done => "done",
    };
    let type_str = match task.task_type {
        TaskType::Task => "task",
        TaskType::Bug => "bug",
        TaskType::Incident => "incident",
    };
    let priority_str = match task.priority {
        Priority::High => "high",
        Priority::Normal => "normal",
        Priority::Low => "low",
    };
    println!("id:       {}", task.id);
    println!("type:     {type_str}");
    println!("status:   {status}");
    println!("priority: {priority_str}");
    if task.blocked { println!("blocked:  yes"); }
    println!("title:    {}", task.title);
    println!("created:  {}", task.created.format("%Y-%m-%dT%H:%M:%S"));
    if let Some(d) = task.done {
        println!("done:     {}", d.format("%Y-%m-%dT%H:%M:%S"));
    }
    if let Some(owner) = &task.owner {
        println!("owner:    {owner}");
    }
    if let Some(o) = task.order {
        println!("order:    {o}");
    }
    if let Some(p) = &task.parent_id {
        println!("parent:   {p}");
    }
    if let Some(desc) = &task.description {
        println!("\n{desc}");
    }
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
        let owner = task.owner.as_deref().unwrap_or("");
        if owner.is_empty() {
            println!("{}  {}  {}  {}", task.id, type_str, status, task.title);
        } else {
            println!("{}  {}  {}  {}  → {}", task.id, type_str, status, task.title, owner);
        }
    }
}
