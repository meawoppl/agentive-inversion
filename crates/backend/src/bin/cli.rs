use anyhow::Context;
use clap::{Parser, Subcommand};
use reqwest::Client;
use serde::Deserialize;
use shared_types::{
    Category, CreateCategoryRequest, CreateTodoRequest, Todo, UpdateCategoryRequest,
    UpdateTodoRequest,
};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "todo-cli")]
#[command(about = "CLI for managing todos and categories via the backend API")]
#[command(
    long_about = "A command-line interface for interacting with the todo backend server.\n\n\
    Supports creating, listing, updating, and deleting todos and categories.\n\
    Can also import todos directly from email JSON files created by the email-poller."
)]
struct Cli {
    /// Backend server URL to connect to.
    ///
    /// The CLI will make HTTP requests to this server's API endpoints.
    /// Use this to connect to a remote server or a different port.
    #[arg(
        short,
        long,
        default_value = "http://localhost:3000",
        env = "TODO_API_URL"
    )]
    base_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage todos - create, list, update, delete, and mark as done
    Todos {
        #[command(subcommand)]
        action: TodoAction,
    },
    /// Manage categories - organize todos with colored labels
    Categories {
        #[command(subcommand)]
        action: CategoryAction,
    },
}

#[derive(Subcommand)]
enum TodoAction {
    /// List all todos with their current status
    ///
    /// Displays todos with a checkbox indicator (○ pending, ✓ completed),
    /// their short ID, title, description, and any attached links.
    List,

    /// Create a new todo item
    ///
    /// Creates a todo with the given title. Optionally add a description,
    /// link to external resources, or assign to a category.
    Create {
        /// The title/name of the todo item.
        /// This is the main text that will be displayed in the list.
        title: String,

        /// A longer description with additional details about the todo.
        /// Use this for notes, context, or step-by-step instructions.
        #[arg(short, long, value_name = "TEXT")]
        description: Option<String>,

        /// A URL link to attach to this todo.
        /// Useful for linking to relevant documents, issues, or resources.
        /// For emails, this will be auto-generated as a Gmail link.
        #[arg(short, long, value_name = "URL")]
        link: Option<String>,

        /// Category UUID to assign this todo to.
        /// Use 'categories list' to see available categories and their IDs.
        /// Only the first 8 characters of the UUID are needed.
        #[arg(short, long, value_name = "UUID")]
        category: Option<Uuid>,
    },

    /// Create a todo from an email JSON file
    ///
    /// Reads an email file (as created by the email-poller) and creates a todo
    /// with the email subject as the title, snippet as description, and a
    /// Gmail link pointing directly to the email in your inbox.
    ///
    /// The Gmail link format is:
    /// https://mail.google.com/mail/u/EMAIL/#all/EMAIL_UID
    FromEmail {
        /// Path to the email JSON file to import.
        /// These files are created by the email-poller in the inbox directory.
        /// Format: YYMMDD_HHMMSS-email-uid.json
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Category UUID to assign this todo to.
        /// Use 'categories list' to see available categories and their IDs.
        #[arg(short, long, value_name = "UUID")]
        category: Option<Uuid>,

        /// Override the default title (email subject) with custom text.
        #[arg(short, long, value_name = "TEXT")]
        title: Option<String>,
    },

    /// Update an existing todo's fields
    ///
    /// Modify any combination of title, description, completion status,
    /// link, or category. Only specified fields will be updated.
    Update {
        /// The UUID of the todo to update.
        /// Use 'todos list' to find the ID (shown in brackets).
        /// You can use the full UUID or just the first 8 characters.
        id: Uuid,

        /// New title to replace the existing one.
        #[arg(short, long, value_name = "TEXT")]
        title: Option<String>,

        /// New description to replace the existing one.
        /// Use empty string "" to clear the description.
        #[arg(short, long, value_name = "TEXT")]
        description: Option<String>,

        /// Set the completion status.
        /// Use --completed=true to mark as done, --completed=false to reopen.
        #[arg(short, long, value_name = "BOOL")]
        completed: Option<bool>,

        /// New link URL to attach.
        /// Use empty string "" to remove the link.
        #[arg(short, long, value_name = "URL")]
        link: Option<String>,

        /// New category UUID to assign.
        /// Use 'categories list' to see available categories.
        #[arg(long, value_name = "UUID")]
        category: Option<Uuid>,
    },

    /// Permanently delete a todo
    ///
    /// This action cannot be undone. The todo will be completely removed.
    Delete {
        /// The UUID of the todo to delete.
        /// Use 'todos list' to find the ID (shown in brackets).
        id: Uuid,
    },

    /// Mark a todo as completed
    ///
    /// Shorthand for 'update <id> --completed=true'.
    /// The todo will show a ✓ checkmark in the list.
    Done {
        /// The UUID of the todo to mark as completed.
        /// Use 'todos list' to find the ID (shown in brackets).
        id: Uuid,
    },

    /// Mark a todo as not completed (reopen)
    ///
    /// Shorthand for 'update <id> --completed=false'.
    /// The todo will show a ○ circle in the list.
    Undo {
        /// The UUID of the todo to mark as not completed.
        id: Uuid,
    },
}

#[derive(Subcommand)]
enum CategoryAction {
    /// List all categories with their colors
    ///
    /// Displays categories with their short ID, name, and color code.
    /// Use these IDs when assigning todos to categories.
    List,

    /// Create a new category for organizing todos
    ///
    /// Categories help group related todos together.
    /// Each category can have a color for visual distinction.
    Create {
        /// The name of the category (e.g., "Work", "Personal", "Urgent").
        /// Should be short and descriptive.
        name: String,

        /// Color for the category in hex format (e.g., "#ff0000" for red).
        /// Used for visual styling in the frontend.
        /// Common colors: #3b82f6 (blue), #10b981 (green), #ef4444 (red),
        /// #f59e0b (amber), #8b5cf6 (purple), #ec4899 (pink).
        #[arg(short, long, value_name = "HEX")]
        color: Option<String>,
    },

    /// Update an existing category's name or color
    Update {
        /// The UUID of the category to update.
        /// Use 'categories list' to find the ID (shown in brackets).
        id: Uuid,

        /// New name for the category.
        #[arg(short, long, value_name = "TEXT")]
        name: Option<String>,

        /// New color in hex format (e.g., "#ff0000").
        #[arg(short, long, value_name = "HEX")]
        color: Option<String>,
    },

    /// Permanently delete a category
    ///
    /// Todos assigned to this category will have their category cleared.
    /// This action cannot be undone.
    Delete {
        /// The UUID of the category to delete.
        /// Use 'categories list' to find the ID (shown in brackets).
        id: Uuid,
    },
}

/// Email metadata from the email-poller JSON files
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct EmailFile {
    uid: String,
    mailbox: String,
    subject: Option<String>,
    from: Option<String>,
    snippet: Option<String>,
    body: Option<String>,
}

/// Generate a Gmail web link for an email
fn gmail_link(mailbox: &str, uid: &str) -> String {
    // Gmail URL format: https://mail.google.com/mail/u/EMAIL/#all/EMAIL_UID
    let encoded_email = urlencoding::encode(mailbox);
    format!(
        "https://mail.google.com/mail/u/{}/#all/{}",
        encoded_email, uid
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = Client::new();

    match cli.command {
        Commands::Todos { action } => handle_todos(&client, &cli.base_url, action).await?,
        Commands::Categories { action } => {
            handle_categories(&client, &cli.base_url, action).await?
        }
    }

    Ok(())
}

async fn handle_todos(client: &Client, base_url: &str, action: TodoAction) -> anyhow::Result<()> {
    let url = format!("{}/api/todos", base_url);

    match action {
        TodoAction::List => {
            let todos: Vec<Todo> = client.get(&url).send().await?.json().await?;
            if todos.is_empty() {
                println!("No todos found.");
            } else {
                for todo in todos {
                    let status = if todo.completed { "✓" } else { "○" };
                    println!("{} [{}] {}", status, &todo.id.to_string()[..8], todo.title);
                    if let Some(desc) = &todo.description {
                        println!("    {}", desc);
                    }
                    if let Some(link) = &todo.link {
                        println!("    Link: {}", link);
                    }
                }
            }
        }
        TodoAction::Create {
            title,
            description,
            link,
            category,
        } => {
            let req = CreateTodoRequest {
                title: title.clone(),
                description,
                due_date: None,
                link,
                category_id: category,
            };
            let todo: Todo = client.post(&url).json(&req).send().await?.json().await?;
            println!(
                "Created todo: [{}] {}",
                &todo.id.to_string()[..8],
                todo.title
            );
        }
        TodoAction::FromEmail {
            file,
            category,
            title,
        } => {
            let content = std::fs::read_to_string(&file).context("Failed to read email file")?;
            let email: EmailFile =
                serde_json::from_str(&content).context("Failed to parse email JSON")?;

            let todo_title = title.unwrap_or_else(|| {
                email
                    .subject
                    .clone()
                    .unwrap_or_else(|| "(no subject)".to_string())
            });

            // Build description from sender and snippet
            let description = {
                let mut parts = Vec::new();
                if let Some(from) = &email.from {
                    parts.push(format!("From: {}", from));
                }
                if let Some(snippet) = &email.snippet {
                    parts.push(snippet.clone());
                }
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join("\n"))
                }
            };

            let link = gmail_link(&email.mailbox, &email.uid);

            let req = CreateTodoRequest {
                title: todo_title.clone(),
                description,
                due_date: None,
                link: Some(link.clone()),
                category_id: category,
            };
            let todo: Todo = client.post(&url).json(&req).send().await?.json().await?;
            println!(
                "Created todo from email: [{}] {}",
                &todo.id.to_string()[..8],
                todo.title
            );
            println!("    Link: {}", link);
        }
        TodoAction::Update {
            id,
            title,
            description,
            completed,
            link,
            category,
        } => {
            let req = UpdateTodoRequest {
                title,
                description,
                completed,
                due_date: None,
                link,
                category_id: category,
            };
            let todo: Todo = client
                .put(format!("{}/{}", url, id))
                .json(&req)
                .send()
                .await?
                .json()
                .await?;
            println!(
                "Updated todo: [{}] {}",
                &todo.id.to_string()[..8],
                todo.title
            );
        }
        TodoAction::Delete { id } => {
            client.delete(format!("{}/{}", url, id)).send().await?;
            println!("Deleted todo: {}", id);
        }
        TodoAction::Done { id } => {
            let req = UpdateTodoRequest {
                title: None,
                description: None,
                completed: Some(true),
                due_date: None,
                link: None,
                category_id: None,
            };
            let todo: Todo = client
                .put(format!("{}/{}", url, id))
                .json(&req)
                .send()
                .await?
                .json()
                .await?;
            println!(
                "Marked as done: [{}] {}",
                &todo.id.to_string()[..8],
                todo.title
            );
        }
        TodoAction::Undo { id } => {
            let req = UpdateTodoRequest {
                title: None,
                description: None,
                completed: Some(false),
                due_date: None,
                link: None,
                category_id: None,
            };
            let todo: Todo = client
                .put(format!("{}/{}", url, id))
                .json(&req)
                .send()
                .await?
                .json()
                .await?;
            println!(
                "Marked as not done: [{}] {}",
                &todo.id.to_string()[..8],
                todo.title
            );
        }
    }

    Ok(())
}

async fn handle_categories(
    client: &Client,
    base_url: &str,
    action: CategoryAction,
) -> anyhow::Result<()> {
    let url = format!("{}/api/categories", base_url);

    match action {
        CategoryAction::List => {
            let categories: Vec<Category> = client.get(&url).send().await?.json().await?;
            if categories.is_empty() {
                println!("No categories found.");
            } else {
                for cat in categories {
                    let color = cat.color.as_deref().unwrap_or("none");
                    println!(
                        "[{}] {} (color: {})",
                        &cat.id.to_string()[..8],
                        cat.name,
                        color
                    );
                }
            }
        }
        CategoryAction::Create { name, color } => {
            let req = CreateCategoryRequest {
                name: name.clone(),
                color,
            };
            let cat: Category = client.post(&url).json(&req).send().await?.json().await?;
            println!(
                "Created category: [{}] {}",
                &cat.id.to_string()[..8],
                cat.name
            );
        }
        CategoryAction::Update { id, name, color } => {
            let req = UpdateCategoryRequest { name, color };
            let cat: Category = client
                .put(format!("{}/{}", url, id))
                .json(&req)
                .send()
                .await?
                .json()
                .await?;
            println!(
                "Updated category: [{}] {}",
                &cat.id.to_string()[..8],
                cat.name
            );
        }
        CategoryAction::Delete { id } => {
            client.delete(format!("{}/{}", url, id)).send().await?;
            println!("Deleted category: {}", id);
        }
    }

    Ok(())
}
