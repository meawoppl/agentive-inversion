//! CLI for triage agent sessions: the only way agents touch the system.
//!
//! Every call goes through the backend REST API so all agent activity is
//! authenticated, validated, and audit-trailed as decisions. The session
//! orchestrator drops `.agent-token` (a short-lived JWT) and `.agent-model`
//! files in the session working directory; this binary picks them up.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use reqwest::Client;
use shared_types::{TriageDecideAction, TriageDecideRequest};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "agent-cli")]
#[command(about = "Triage agent interface to the agentive-inversion backend API")]
struct Cli {
    /// Backend API base URL
    #[arg(long, default_value = "http://localhost:3000", env = "AGENT_API_URL")]
    api_url: String,

    /// Bearer token; falls back to ./.agent-token
    #[arg(long, env = "AGENT_API_TOKEN")]
    token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show the user's about-me document
    AboutMe {
        #[command(subcommand)]
        command: AboutMeCommands,
    },
    /// Submit a triage disposition for an email
    Decide {
        #[command(subcommand)]
        command: DecideCommands,
    },
    /// Show pipeline stage counts and health
    Pipeline,
}

#[derive(Subcommand)]
enum AboutMeCommands {
    /// Print the about-me document content
    Show,
}

#[derive(Subcommand)]
enum DecideCommands {
    /// Flag as probably-archivable (final call happens in a later pass)
    ArchiveCandidate {
        #[arg(long)]
        email_id: Uuid,
        #[arg(long)]
        reasoning: String,
    },
    /// Archive now: label agent-archived and remove from inbox
    Archive {
        #[arg(long)]
        email_id: Uuid,
        #[arg(long)]
        reasoning: String,
    },
    /// Deliberately leave in the inbox
    Keep {
        #[arg(long)]
        email_id: Uuid,
        #[arg(long)]
        reasoning: String,
    },
    /// Propose a calendar event (goes to the user's review inbox)
    Event {
        #[arg(long)]
        email_id: Uuid,
        #[arg(long)]
        summary: String,
        /// RFC3339, e.g. 2026-08-09T18:00:00-07:00
        #[arg(long)]
        start: DateTime<Utc>,
        /// RFC3339; defaults to one hour after start if omitted
        #[arg(long)]
        end: Option<DateTime<Utc>>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        location: Option<String>,
        #[arg(long)]
        reasoning: String,
    },
    /// Propose a todo (goes to the user's review inbox)
    Todo {
        #[arg(long)]
        email_id: Uuid,
        #[arg(long)]
        title: String,
        #[arg(long)]
        description: Option<String>,
        /// RFC3339 due date
        #[arg(long)]
        due: Option<DateTime<Utc>>,
        #[arg(long)]
        reasoning: String,
    },
    /// Nothing actionable here
    Ignore {
        #[arg(long)]
        email_id: Uuid,
        #[arg(long)]
        reasoning: String,
    },
    /// Send to the action queue for the deeper pass
    QueueAction {
        #[arg(long)]
        email_id: Uuid,
        #[arg(long)]
        reasoning: String,
    },
    /// Propose forwarding (e.g. a receipt to the expense system); the
    /// destination is server policy and the user approves before it sends
    Forward {
        #[arg(long)]
        email_id: Uuid,
        #[arg(long)]
        reasoning: String,
    },
}

fn resolve_token(cli_token: Option<String>) -> Result<String> {
    if let Some(t) = cli_token {
        return Ok(t);
    }
    std::fs::read_to_string(".agent-token")
        .map(|s| s.trim().to_string())
        .context("No token: pass --token, set AGENT_API_TOKEN, or provide ./.agent-token")
}

fn session_model() -> Option<String> {
    std::fs::read_to_string(".agent-model")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let token = resolve_token(cli.token)?;
    let client = Client::new();
    let base = cli.api_url.trim_end_matches('/');

    match cli.command {
        Commands::AboutMe {
            command: AboutMeCommands::Show,
        } => {
            let resp = client
                .get(format!("{base}/api/about-me"))
                .bearer_auth(&token)
                .send()
                .await
                .context("Request failed")?;
            let status = resp.status();
            let body: serde_json::Value = resp.json().await.context("Invalid response")?;
            if !status.is_success() {
                anyhow::bail!("API error {status}: {body}");
            }
            println!(
                "{}",
                body.get("content").and_then(|c| c.as_str()).unwrap_or("")
            );
        }

        Commands::Pipeline => {
            let resp = client
                .get(format!("{base}/api/pipeline/stats"))
                .bearer_auth(&token)
                .send()
                .await
                .context("Request failed")?;
            let status = resp.status();
            let body: serde_json::Value = resp.json().await.context("Invalid response")?;
            if !status.is_success() {
                anyhow::bail!("API error {status}: {body}");
            }
            println!("{}", serde_json::to_string_pretty(&body)?);
        }

        Commands::Decide { command } => {
            let (email_id, action, reasoning) = match command {
                DecideCommands::ArchiveCandidate {
                    email_id,
                    reasoning,
                } => (email_id, TriageDecideAction::ArchiveCandidate, reasoning),
                DecideCommands::Archive {
                    email_id,
                    reasoning,
                } => (email_id, TriageDecideAction::Archive, reasoning),
                DecideCommands::Keep {
                    email_id,
                    reasoning,
                } => (email_id, TriageDecideAction::Keep, reasoning),
                DecideCommands::Ignore {
                    email_id,
                    reasoning,
                } => (email_id, TriageDecideAction::Ignore, reasoning),
                DecideCommands::QueueAction {
                    email_id,
                    reasoning,
                } => (email_id, TriageDecideAction::QueueAction, reasoning),
                DecideCommands::Forward {
                    email_id,
                    reasoning,
                } => (email_id, TriageDecideAction::Forward, reasoning),
                DecideCommands::Event {
                    email_id,
                    summary,
                    start,
                    end,
                    description,
                    location,
                    reasoning,
                } => (
                    email_id,
                    TriageDecideAction::Event {
                        summary,
                        start,
                        end: end.unwrap_or(start + chrono::Duration::hours(1)),
                        description,
                        location,
                    },
                    reasoning,
                ),
                DecideCommands::Todo {
                    email_id,
                    title,
                    description,
                    due,
                    reasoning,
                } => (
                    email_id,
                    TriageDecideAction::Todo {
                        title,
                        description,
                        due_date: due,
                    },
                    reasoning,
                ),
            };

            let req = TriageDecideRequest {
                email_id,
                action,
                reasoning,
                model: session_model(),
            };

            let resp = client
                .post(format!("{base}/api/triage/decisions"))
                .bearer_auth(&token)
                .json(&req)
                .send()
                .await
                .context("Request failed")?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("API error {status}: {body}");
            }
            println!("{body}");
        }
    }

    Ok(())
}
