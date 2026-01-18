use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager, ManagerConfig},
    AsyncPgConnection, RunQueryDsl,
};
use uuid::Uuid;

use crate::schema::emails;

pub type DbPool = Pool<AsyncPgConnection>;

async fn establish_tls_connection(config: String) -> diesel::ConnectionResult<AsyncPgConnection> {
    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_config);

    let (client, connection) = tokio_postgres::connect(&config, tls)
        .await
        .map_err(|e| diesel::ConnectionError::BadConnection(e.to_string()))?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!("Connection error: {}", e);
        }
    });

    AsyncPgConnection::try_from(client).await
}

pub fn establish_connection_pool() -> anyhow::Result<DbPool> {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let mut manager_config = ManagerConfig::default();
    manager_config.custom_setup =
        Box::new(|url| Box::pin(establish_tls_connection(url.to_string())));

    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new_with_config(
        database_url,
        manager_config,
    );
    let pool = Pool::builder(config).build()?;

    Ok(pool)
}

/// Email model matching database schema
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = emails)]
pub struct Email {
    pub id: Uuid,
    pub account_id: Uuid,
    pub gmail_id: String,
    pub thread_id: String,
    pub history_id: Option<i64>,
    pub subject: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: Vec<Option<String>>,
    pub cc_addresses: Option<Vec<Option<String>>>,
    pub snippet: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub labels: Option<Vec<Option<String>>>,
    pub has_attachments: bool,
    pub received_at: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
    pub processed: bool,
    pub processed_at: Option<DateTime<Utc>>,
    pub archived_in_gmail: bool,
}

/// For inserting new emails
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = emails)]
pub struct NewEmail {
    pub account_id: Uuid,
    pub gmail_id: String,
    pub thread_id: String,
    pub history_id: Option<i64>,
    pub subject: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: Vec<Option<String>>,
    pub cc_addresses: Option<Vec<Option<String>>>,
    pub snippet: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub labels: Option<Vec<Option<String>>>,
    pub has_attachments: bool,
    pub received_at: DateTime<Utc>,
}

/// Insert a new email, returning it. Uses ON CONFLICT DO NOTHING to handle duplicates.
pub async fn insert_email(
    conn: &mut AsyncPgConnection,
    new_email: NewEmail,
) -> anyhow::Result<Option<Email>> {
    use crate::schema::emails::dsl::*;

    let result = diesel::insert_into(emails)
        .values(&new_email)
        .on_conflict((account_id, gmail_id))
        .do_nothing()
        .get_result::<Email>(conn)
        .await
        .optional()?;

    Ok(result)
}

/// Check if an email already exists
pub async fn email_exists(
    conn: &mut AsyncPgConnection,
    account_uuid: Uuid,
    gmail_message_id: &str,
) -> anyhow::Result<bool> {
    use crate::schema::emails::dsl::*;

    let count: i64 = emails
        .filter(account_id.eq(account_uuid))
        .filter(gmail_id.eq(gmail_message_id))
        .count()
        .get_result(conn)
        .await?;

    Ok(count > 0)
}

/// Get unprocessed emails for processing
pub async fn get_unprocessed_emails(
    conn: &mut AsyncPgConnection,
    limit: i64,
) -> anyhow::Result<Vec<Email>> {
    use crate::schema::emails::dsl::*;

    let result = emails
        .filter(processed.eq(false))
        .order_by(fetched_at.asc())
        .limit(limit)
        .load::<Email>(conn)
        .await?;

    Ok(result)
}

/// Mark an email as processed
pub async fn mark_email_processed(
    conn: &mut AsyncPgConnection,
    email_id: Uuid,
) -> anyhow::Result<()> {
    use crate::schema::emails::dsl::*;

    diesel::update(emails.filter(id.eq(email_id)))
        .set((processed.eq(true), processed_at.eq(Some(Utc::now()))))
        .execute(conn)
        .await?;

    Ok(())
}

// ============================================================================
// Agent Rules
// ============================================================================

/// Get all active rules for email source type
pub async fn get_active_email_rules(
    conn: &mut AsyncPgConnection,
) -> anyhow::Result<Vec<shared_types::AgentRule>> {
    use crate::schema::agent_rules::dsl::*;

    let rules = agent_rules
        .filter(is_active.eq(true))
        .filter(source_type.eq("email").or(source_type.eq("any")))
        .order_by((priority.desc(), created_at.desc()))
        .load::<shared_types::AgentRule>(conn)
        .await?;

    Ok(rules)
}

/// Increment the match count for a rule
pub async fn increment_rule_match_count(
    conn: &mut AsyncPgConnection,
    rule_id: Uuid,
) -> anyhow::Result<()> {
    use crate::schema::agent_rules::dsl::*;

    diesel::update(agent_rules.filter(id.eq(rule_id)))
        .set((
            match_count.eq(match_count + 1),
            last_matched_at.eq(Some(Utc::now())),
        ))
        .execute(conn)
        .await?;

    Ok(())
}

// ============================================================================
// Agent Decisions
// ============================================================================

/// Create a new agent decision
#[allow(clippy::too_many_arguments)]
pub async fn create_decision(
    conn: &mut AsyncPgConnection,
    source_type_val: &str,
    source_id_val: Option<Uuid>,
    source_external_id_val: Option<&str>,
    decision_type_val: &str,
    proposed_action_val: &str,
    reasoning_val: &str,
    reasoning_details_val: Option<&str>,
    confidence_val: f32,
    status_val: &str,
    applied_rule_id_val: Option<Uuid>,
) -> anyhow::Result<Uuid> {
    use crate::schema::agent_decisions::dsl::*;

    let decision_id = diesel::insert_into(agent_decisions)
        .values((
            source_type.eq(source_type_val),
            source_id.eq(source_id_val),
            source_external_id.eq(source_external_id_val),
            decision_type.eq(decision_type_val),
            proposed_action.eq(proposed_action_val),
            reasoning.eq(reasoning_val),
            reasoning_details.eq(reasoning_details_val),
            confidence.eq(confidence_val),
            status.eq(status_val),
            applied_rule_id.eq(applied_rule_id_val),
        ))
        .returning(id)
        .get_result::<Uuid>(conn)
        .await?;

    Ok(decision_id)
}

/// Create a todo from an approved decision
#[allow(clippy::too_many_arguments)]
pub async fn create_todo_from_decision(
    conn: &mut AsyncPgConnection,
    decision_id_val: Uuid,
    title_val: &str,
    description_val: Option<&str>,
    source_val: &str,
    source_id_val: Option<&str>,
    due_date_val: Option<DateTime<Utc>>,
    category_id_val: Option<Uuid>,
) -> anyhow::Result<Uuid> {
    use crate::schema::todos::dsl::*;

    let todo_id = diesel::insert_into(todos)
        .values((
            title.eq(title_val),
            description.eq(description_val),
            completed.eq(false),
            source.eq(source_val),
            source_id.eq(source_id_val),
            due_date.eq(due_date_val),
            category_id.eq(category_id_val),
            decision_id.eq(Some(decision_id_val)),
        ))
        .returning(id)
        .get_result::<Uuid>(conn)
        .await?;

    Ok(todo_id)
}

/// Update a decision with the result todo ID
pub async fn update_decision_result_todo(
    conn: &mut AsyncPgConnection,
    decision_id_val: Uuid,
    todo_id_val: Uuid,
) -> anyhow::Result<()> {
    use crate::schema::agent_decisions::dsl::*;

    diesel::update(agent_decisions.filter(id.eq(decision_id_val)))
        .set((
            result_todo_id.eq(Some(todo_id_val)),
            status.eq("executed"),
            executed_at.eq(Some(Utc::now())),
        ))
        .execute(conn)
        .await?;

    Ok(())
}
