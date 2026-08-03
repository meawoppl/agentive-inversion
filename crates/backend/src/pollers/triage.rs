//! Multi-model agentic triage pipeline.
//!
//! Runs as its own background task, decoupled from email ingestion — a triage
//! failure must never stop mail from landing in the database (emails simply
//! stay `pending` and the queue self-drains when triage recovers).
//!
//! Per cycle, three stages run as Claude Code sessions (via the claude-codes
//! crate), each restricted to the Read tool and the `agent-cli` binary, which
//! talks to our own REST API:
//!   1. screening (Haiku): pending → archive_candidate / event / action / keep
//!   2. archive determinations (Sonnet): archive_candidate → archived / kept
//!   3. action pass (Opus + about-me doc): action_queued → todo proposals

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use claude_codes::{AsyncClient, ClaudeCliBuilder, PermissionMode};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::auth::types::AuthConfig;
use crate::db::{self, DbPool};
use crate::services::triage::gmail_permalink;

/// Shared triage health state, exposed via /api/pipeline/stats
#[derive(Debug, Clone)]
pub struct TriageHealthState {
    pub mode: String,
    pub last_cycle_at: Option<DateTime<Utc>>,
    pub last_cycle_error: Option<String>,
    pub consecutive_failures: i32,
}

impl Default for TriageHealthState {
    fn default() -> Self {
        Self {
            mode: "starting".to_string(),
            last_cycle_at: None,
            last_cycle_error: None,
            consecutive_failures: 0,
        }
    }
}

pub type TriageHealth = Arc<RwLock<TriageHealthState>>;

#[derive(Debug, Clone)]
pub struct TriageConfig {
    pub poll_interval: Duration,
    pub batch_size: i64,
    pub screen_model: String,
    pub archive_model: String,
    pub action_model: String,
    pub session_timeout: Duration,
}

impl TriageConfig {
    pub fn from_env() -> Self {
        let env_or =
            |key: &str, default: &str| std::env::var(key).unwrap_or_else(|_| default.to_string());
        Self {
            poll_interval: Duration::from_secs(
                env_or("TRIAGE_POLL_INTERVAL_SECS", "300")
                    .parse()
                    .unwrap_or(300),
            ),
            batch_size: env_or("TRIAGE_BATCH_SIZE", "20").parse().unwrap_or(20),
            screen_model: env_or("TRIAGE_SCREEN_MODEL", "claude-haiku-4-5"),
            archive_model: env_or("TRIAGE_ARCHIVE_MODEL", "claude-sonnet-4-5"),
            action_model: env_or("TRIAGE_ACTION_MODEL", "claude-opus-4-8"),
            session_timeout: Duration::from_secs(
                env_or("TRIAGE_SESSION_TIMEOUT_SECS", "600")
                    .parse()
                    .unwrap_or(600),
            ),
        }
    }
}

/// Email fields handed to agent sessions in the datafile
#[derive(Debug, Serialize)]
struct EmailForAgent {
    id: uuid::Uuid,
    account_email: String,
    from_address: String,
    from_name: Option<String>,
    subject: String,
    received_at: DateTime<Utc>,
    snippet: Option<String>,
    body_text: Option<String>,
    gmail_link: String,
}

const BODY_TRUNCATE_CHARS: usize = 4000;

const SCREEN_INSTRUCTIONS: &str = r#"You are the screening stage of an email triage pipeline.

Read ./emails.json (use the Read tool). It contains a JSON array of emails.
For EVERY email in the file, make exactly one disposition by running the
agent-cli command via Bash. Do not skip any email. Do not take any other action.

Dispositions:
- Newsletter, promotion, notification, or automated mail with no action needed:
  agent-cli decide archive-candidate --email-id <id> --reasoning "<why>"
- Contains a real-world event with a concrete date/time the user could attend
  (invitation, appointment, meetup, wedding, show, deadline-as-event):
  agent-cli decide event --email-id <id> --summary "<title>" --start <rfc3339> --end <rfc3339> [--location "<loc>"] [--description "<details>"] --reasoning "<why>"
  If the timezone is unclear, assume America/Los_Angeles. If no end time is
  given, assume one hour after the start.
- Requires the user to act, reply, decide, or follow up:
  agent-cli decide queue-action --email-id <id> --reasoning "<why>"
- Personal or potentially important, but nothing to do right now:
  agent-cli decide keep --email-id <id> --reasoning "<why>"

Be decisive and fast. One agent-cli call per email. When finished, reply with
a one-line summary of counts per category."#;

const ARCHIVE_INSTRUCTIONS: &str = r#"You are the archive-determination stage of an email triage pipeline.

Read ./emails.json (use the Read tool). Each email was flagged by a screening
pass as probably archivable. Make the FINAL call for every email by running
agent-cli via Bash:

- Confirm archive (removes it from the inbox, tagged 'agent-archived',
  recoverable): agent-cli decide archive --email-id <id> --reasoning "<why>"
- Not sure, or it might matter to the user: agent-cli decide keep --email-id <id> --reasoning "<why>"

Be conservative: personal correspondence, money, legal, medical, childcare,
travel, or anything that reads like a human wrote it to the user specifically
should be kept. Bulk mail, marketing, and automated notifications with no
action should be archived. One call per email; do not skip any. Finish with a
one-line summary."#;

const ACTION_INSTRUCTIONS: &str = r#"You are the action stage of an email triage pipeline: you decide what belongs
on the user's todo list.

First run: agent-cli about-me show
That document describes who the user is and what matters to them. Use it to
judge importance, urgency, and how to phrase todos.

Then read ./emails.json (use the Read tool). Every email was flagged as
needing action. For each one, run exactly one agent-cli command via Bash:

- Propose a todo (goes to the user's review inbox):
  agent-cli decide todo --email-id <id> --title "<concrete next action>" [--description "<context>"] [--due <rfc3339>] --reasoning "<why this matters, referencing the about-me context where relevant>"
- On reflection nothing is actually needed:
  agent-cli decide ignore --email-id <id> --reasoning "<why>"

Todo titles should be concrete next actions ("Reply to adjuster with repair
estimate"), not vague ("Handle insurance email"). Extract real deadlines into
--due. One call per email; do not skip any. Finish with a one-line summary."#;

/// Start the triage background task. Never blocks ingestion; failures leave
/// emails pending for the next cycle.
pub async fn start_triage_task(pool: DbPool, auth_config: Arc<AuthConfig>, health: TriageHealth) {
    let config = TriageConfig::from_env();

    // Initial mode detection with explicit, greppable logging
    match detect_mode(&pool).await {
        Ok(cred) => {
            tracing::info!(
                "Email triage mode: agentic (credential={}, screen={}, archive={}, action={})",
                cred.source_name(),
                config.screen_model,
                config.archive_model,
                config.action_model
            );
            tracing::info!(
                "Triage archive mode: {} ({})",
                crate::services::triage::archive_mode(),
                if crate::services::triage::archive_mode() == "execute" {
                    "auto-archives on determination"
                } else {
                    "dry run - determinations recorded as proposals only"
                }
            );
            health.write().await.mode = "agentic".to_string();
        }
        Err(reason) => {
            tracing::warn!("Email triage mode: disabled ({reason})");
            health.write().await.mode = "disabled".to_string();
        }
    }

    loop {
        // Re-detect each cycle so adding a credential recovers without restart
        match detect_mode(&pool).await {
            Ok(cred) => {
                {
                    let mut h = health.write().await;
                    if h.mode != "agentic" {
                        tracing::info!("Email triage mode: agentic (recovered)");
                    }
                    h.mode = "agentic".to_string();
                }

                match run_cycle(&pool, &auth_config, &config, &cred).await {
                    Ok(worked) => {
                        let mut h = health.write().await;
                        h.last_cycle_at = Some(Utc::now());
                        h.last_cycle_error = None;
                        h.consecutive_failures = 0;
                        drop(h);
                        if worked > 0 {
                            tracing::info!("Triage cycle processed {} emails", worked);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Triage cycle failed: {:#}", e);
                        let mut h = health.write().await;
                        h.last_cycle_at = Some(Utc::now());
                        h.last_cycle_error = Some(format!("{e:#}"));
                        h.consecutive_failures += 1;
                    }
                }
            }
            Err(reason) => {
                let mut h = health.write().await;
                if h.mode != "disabled" {
                    tracing::warn!("Email triage mode: disabled ({reason})");
                }
                h.mode = "disabled".to_string();
            }
        }

        tokio::time::sleep(config.poll_interval).await;
    }
}

/// Credential the pipeline authenticates sessions with
#[derive(Clone)]
pub enum TriageCredential {
    /// Claude Code OAuth token from the in-app login (subscription auth)
    OAuthToken(String),
    /// In-app login succeeded but the token wasn't capturable; the CLI
    /// persisted its own credentials inside the container, so sessions
    /// authenticate from disk with no env override
    CliPersisted,
    /// ANTHROPIC_API_KEY inherited from the environment
    ApiKeyEnv,
}

impl TriageCredential {
    fn source_name(&self) -> &'static str {
        match self {
            TriageCredential::OAuthToken(_) => "claude-code-login",
            TriageCredential::CliPersisted => "claude-cli-persisted",
            TriageCredential::ApiKeyEnv => "api-key-env",
        }
    }
}

/// Check the runtime prerequisites for agentic triage. The in-app Claude
/// Code credential takes precedence over an environment API key.
async fn detect_mode(pool: &DbPool) -> Result<TriageCredential, String> {
    match tokio::process::Command::new("claude")
        .arg("--version")
        .output()
        .await
    {
        Ok(out) if out.status.success() => {}
        Ok(_) => return Err("claude binary exited non-zero".to_string()),
        Err(e) => return Err(format!("claude binary not found: {e}")),
    }

    if let Ok(mut conn) = pool.get().await {
        if let Ok(Some(cred)) = db::claude_credentials::get(&mut conn).await {
            // Empty token = cli-persisted sentinel: the login succeeded but
            // the token wasn't capturable; sessions use the CLI's own disk
            // credentials (container-lifetime only)
            if cred.oauth_token.is_empty() {
                return Ok(TriageCredential::CliPersisted);
            }
            return Ok(TriageCredential::OAuthToken(cred.oauth_token));
        }
    }

    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        return Ok(TriageCredential::ApiKeyEnv);
    }

    Err("no credential: connect Claude Code in the UI or set ANTHROPIC_API_KEY".to_string())
}

/// Run one full triage cycle (all three stages). Returns emails touched.
async fn run_cycle(
    pool: &DbPool,
    auth_config: &AuthConfig,
    config: &TriageConfig,
    credential: &TriageCredential,
) -> Result<usize> {
    let service_email = auth_config
        .allowed_emails
        .first()
        .context("ALLOWED_EMAILS is empty; cannot mint agent token")?;
    let token = crate::auth::jwt::create_token(
        auth_config,
        service_email,
        Some("triage-agent".to_string()),
    )
    .context("Failed to mint agent session token")?;

    let mut total = 0;

    // Stage 1: screening (Haiku)
    let pending = load_stage_emails(pool, "pending", config.batch_size).await?;
    if !pending.is_empty() {
        total += pending.len();
        run_stage(
            &config.screen_model,
            SCREEN_INSTRUCTIONS,
            &pending,
            &token,
            config.session_timeout,
            credential,
        )
        .await
        .context("Screening stage failed")?;
    }

    // Stage 2: archive determinations (Sonnet)
    let candidates = load_stage_emails(pool, "archive_candidate", config.batch_size).await?;
    if !candidates.is_empty() {
        run_stage(
            &config.archive_model,
            ARCHIVE_INSTRUCTIONS,
            &candidates,
            &token,
            config.session_timeout,
            credential,
        )
        .await
        .context("Archive determination stage failed")?;
    }

    // Stage 3: action pass (Opus + about-me)
    let actions = load_stage_emails(pool, "action_queued", config.batch_size).await?;
    if !actions.is_empty() {
        run_stage(
            &config.action_model,
            ACTION_INSTRUCTIONS,
            &actions,
            &token,
            config.session_timeout,
            credential,
        )
        .await
        .context("Action stage failed")?;
    }

    Ok(total)
}

async fn load_stage_emails(pool: &DbPool, status: &str, limit: i64) -> Result<Vec<EmailForAgent>> {
    let mut conn = pool.get().await.context("Failed to get DB connection")?;
    let emails = db::emails::list_by_triage_status(&mut conn, status, limit).await?;
    let accounts = db::google_accounts::list_all(&mut conn).await?;

    let account_email = |id: uuid::Uuid| {
        accounts
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.email.clone())
            .unwrap_or_default()
    };

    Ok(emails
        .into_iter()
        .map(|e| {
            let acct = account_email(e.account_id);
            let gmail_link = gmail_permalink(&acct, &e.gmail_id);
            EmailForAgent {
                id: e.id,
                account_email: acct,
                from_address: e.from_address,
                from_name: e.from_name,
                subject: e.subject,
                received_at: e.received_at,
                snippet: e.snippet,
                body_text: e
                    .body_text
                    .map(|b| b.chars().take(BODY_TRUNCATE_CHARS).collect()),
                gmail_link,
            }
        })
        .collect())
}

/// Run one agent session over a batch of emails
async fn run_stage(
    model: &str,
    instructions: &str,
    emails: &[EmailForAgent],
    token: &str,
    timeout: Duration,
    credential: &TriageCredential,
) -> Result<()> {
    // Explicit stage-start marker so a session's run window is observed, not
    // inferred from the previous stage's completion — makes memory/profile
    // attribution unambiguous even for the first stage of a cycle.
    tracing::info!(
        "Triage session starting: model={model} emails={}",
        emails.len()
    );

    let workdir = tempfile::tempdir().context("Failed to create session workdir")?;

    std::fs::write(
        workdir.path().join("emails.json"),
        serde_json::to_vec_pretty(emails)?,
    )
    .context("Failed to write datafile")?;
    std::fs::write(workdir.path().join(".agent-token"), token)
        .context("Failed to write agent token")?;
    std::fs::write(workdir.path().join(".agent-model"), model)
        .context("Failed to write agent model tag")?;

    let mut builder = ClaudeCliBuilder::new()
        .model(model)
        .working_directory(workdir.path())
        .allowed_tools(["Read", "Bash(agent-cli:*)"])
        .permission_mode(PermissionMode::DontAsk);

    // Subscription auth from the in-app login takes precedence; the builder
    // sets CLAUDE_CODE_OAUTH_TOKEN on the child process
    if let TriageCredential::OAuthToken(oauth) = credential {
        builder = builder.oauth_token(oauth.clone());
    }

    let mut client = AsyncClient::from_builder(builder)
        .await
        .context("Failed to start claude session")?;

    let prompt = format!(
        "{instructions}\n\nThere are {} emails in ./emails.json.",
        emails.len()
    );

    let result = tokio::time::timeout(timeout, client.query(&prompt)).await;

    // Always reap the claude subprocess — the crate does not kill on drop, so
    // skipping shutdown on the error paths would orphan a session that keeps
    // burning memory and tokens
    let outputs = match result {
        Ok(Ok(outputs)) => {
            client.shutdown().await.ok();
            outputs
        }
        Ok(Err(e)) => {
            client.shutdown().await.ok();
            return Err(e).context("Agent session failed");
        }
        Err(_) => {
            client.shutdown().await.ok();
            anyhow::bail!("Agent session timed out after {timeout:?}; claude process killed");
        }
    };

    for output in &outputs {
        if let claude_codes::ClaudeOutput::Result(r) = output {
            if r.is_error {
                anyhow::bail!("Agent session ended in error state");
            }
            tracing::info!(
                "Triage session complete: model={model} turns={} emails={}",
                r.num_turns,
                emails.len()
            );
        }
    }

    Ok(())
}
