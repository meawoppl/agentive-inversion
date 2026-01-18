use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager, ManagerConfig},
    AsyncPgConnection, RunQueryDsl,
};
use shared_types::{AgentDecision, AgentRule, Category, EmailAccount, Todo};
use uuid::Uuid;

use crate::models::{AgentDecisionRow, NewEmail};

pub type DbPool = Pool<AsyncPgConnection>;

async fn establish_tls_connection(config: String) -> diesel::ConnectionResult<AsyncPgConnection> {
    // Set up rustls TLS configuration
    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_config);

    // Parse the connection string and connect with TLS
    let (client, connection) = tokio_postgres::connect(&config, tls)
        .await
        .map_err(|e| diesel::ConnectionError::BadConnection(e.to_string()))?;

    // Spawn the connection task
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!("Connection error: {}", e);
        }
    });

    // Build the async connection from the tokio-postgres client
    AsyncPgConnection::try_from(client).await
}

pub fn establish_connection_pool() -> anyhow::Result<DbPool> {
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable must be set"))?;

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

// Email account database operations
#[allow(dead_code)]
pub mod email_accounts {
    use super::*;

    pub async fn list_all(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<EmailAccount>> {
        use crate::schema::email_accounts::dsl::*;

        let accounts = email_accounts
            .order_by(created_at.desc())
            .load::<EmailAccount>(conn)
            .await?;

        Ok(accounts)
    }

    pub async fn list_active(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<EmailAccount>> {
        use crate::schema::email_accounts::dsl::*;

        let accounts = email_accounts
            .filter(is_active.eq(true))
            .order_by(created_at.desc())
            .load::<EmailAccount>(conn)
            .await?;

        Ok(accounts)
    }

    pub async fn get_by_id(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> anyhow::Result<EmailAccount> {
        use crate::schema::email_accounts::dsl::*;

        let account = email_accounts
            .filter(id.eq(account_id))
            .first::<EmailAccount>(conn)
            .await?;

        Ok(account)
    }

    pub async fn get_by_email(
        conn: &mut AsyncPgConnection,
        email: &str,
    ) -> anyhow::Result<Option<EmailAccount>> {
        use crate::schema::email_accounts::dsl::*;

        let account = email_accounts
            .filter(email_address.eq(email))
            .first::<EmailAccount>(conn)
            .await
            .optional()?;

        Ok(account)
    }

    pub async fn create(
        conn: &mut AsyncPgConnection,
        account_name_val: &str,
        email_addr: &str,
        provider_val: &str,
    ) -> anyhow::Result<EmailAccount> {
        use crate::schema::email_accounts::dsl::*;

        let new_account = diesel::insert_into(email_accounts)
            .values((
                account_name.eq(account_name_val),
                email_address.eq(email_addr),
                provider.eq(provider_val),
                sync_status.eq("pending"),
                is_active.eq(true),
            ))
            .get_result::<EmailAccount>(conn)
            .await?;

        Ok(new_account)
    }

    pub async fn update_oauth_tokens(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        refresh_token: &str,
        access_token: &str,
        expires_at: DateTime<Utc>,
    ) -> anyhow::Result<EmailAccount> {
        use crate::schema::email_accounts::dsl::*;

        let updated = diesel::update(email_accounts.filter(id.eq(account_id)))
            .set((
                oauth_refresh_token.eq(Some(refresh_token)),
                oauth_access_token.eq(Some(access_token)),
                oauth_token_expires_at.eq(Some(expires_at)),
                sync_status.eq("pending"),
            ))
            .get_result::<EmailAccount>(conn)
            .await?;

        Ok(updated)
    }

    pub async fn update_sync_status(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        status: &str,
        error: Option<&str>,
        last_msg_id: Option<&str>,
    ) -> anyhow::Result<EmailAccount> {
        use crate::schema::email_accounts::dsl::*;

        let updated = diesel::update(email_accounts.filter(id.eq(account_id)))
            .set((
                sync_status.eq(status),
                last_sync_error.eq(error),
                last_message_id.eq(last_msg_id),
                last_synced.eq(Some(Utc::now())),
            ))
            .get_result::<EmailAccount>(conn)
            .await?;

        Ok(updated)
    }

    pub async fn delete(conn: &mut AsyncPgConnection, account_id: Uuid) -> anyhow::Result<()> {
        use crate::schema::email_accounts::dsl::*;

        diesel::delete(email_accounts.filter(id.eq(account_id)))
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn deactivate(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> anyhow::Result<EmailAccount> {
        use crate::schema::email_accounts::dsl::*;

        let updated = diesel::update(email_accounts.filter(id.eq(account_id)))
            .set(is_active.eq(false))
            .get_result::<EmailAccount>(conn)
            .await?;

        Ok(updated)
    }

    /// Update sync error status for an account
    pub async fn update_sync_error(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        error: &str,
    ) -> anyhow::Result<()> {
        use crate::schema::email_accounts::dsl::*;

        diesel::update(email_accounts.filter(id.eq(account_id)))
            .set((
                sync_status.eq("error"),
                last_sync_error.eq(Some(error)),
                last_synced.eq(Some(Utc::now())),
            ))
            .execute(conn)
            .await?;

        Ok(())
    }
}

// Todo database operations
#[allow(dead_code)]
pub mod todos {
    use super::*;

    pub async fn list_all(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<Todo>> {
        use crate::schema::todos::dsl::*;

        let items = todos.order_by(created_at.desc()).load::<Todo>(conn).await?;

        Ok(items)
    }

    pub async fn get_by_id(conn: &mut AsyncPgConnection, todo_id: Uuid) -> anyhow::Result<Todo> {
        use crate::schema::todos::dsl::*;

        let todo = todos.filter(id.eq(todo_id)).first::<Todo>(conn).await?;

        Ok(todo)
    }

    pub async fn create(
        conn: &mut AsyncPgConnection,
        title_val: &str,
        description_val: Option<&str>,
        due_date_val: Option<DateTime<Utc>>,
        link_val: Option<&str>,
        category_id_val: Option<Uuid>,
    ) -> anyhow::Result<Todo> {
        use crate::schema::todos::dsl::*;

        let new_todo = diesel::insert_into(todos)
            .values((
                title.eq(title_val),
                description.eq(description_val),
                completed.eq(false),
                source.eq("manual"),
                due_date.eq(due_date_val),
                link.eq(link_val),
                category_id.eq(category_id_val),
            ))
            .get_result::<Todo>(conn)
            .await?;

        Ok(new_todo)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        conn: &mut AsyncPgConnection,
        todo_id: Uuid,
        title_val: Option<&str>,
        description_val: Option<&str>,
        completed_val: Option<bool>,
        due_date_val: Option<DateTime<Utc>>,
        link_val: Option<&str>,
        category_id_val: Option<Uuid>,
    ) -> anyhow::Result<Todo> {
        use crate::schema::todos::dsl::*;

        // Update each field individually if provided
        if let Some(t) = title_val {
            diesel::update(todos.filter(id.eq(todo_id)))
                .set(title.eq(t))
                .execute(conn)
                .await?;
        }
        if let Some(d) = description_val {
            diesel::update(todos.filter(id.eq(todo_id)))
                .set(description.eq(Some(d)))
                .execute(conn)
                .await?;
        }
        if let Some(c) = completed_val {
            diesel::update(todos.filter(id.eq(todo_id)))
                .set(completed.eq(c))
                .execute(conn)
                .await?;
        }
        if let Some(dd) = due_date_val {
            diesel::update(todos.filter(id.eq(todo_id)))
                .set(due_date.eq(Some(dd)))
                .execute(conn)
                .await?;
        }
        if let Some(l) = link_val {
            diesel::update(todos.filter(id.eq(todo_id)))
                .set(link.eq(Some(l)))
                .execute(conn)
                .await?;
        }
        if let Some(cat) = category_id_val {
            diesel::update(todos.filter(id.eq(todo_id)))
                .set(category_id.eq(Some(cat)))
                .execute(conn)
                .await?;
        }

        // Always update updated_at and return the result
        let updated = diesel::update(todos.filter(id.eq(todo_id)))
            .set(updated_at.eq(Utc::now()))
            .get_result::<Todo>(conn)
            .await?;

        Ok(updated)
    }

    pub async fn delete(conn: &mut AsyncPgConnection, todo_id: Uuid) -> anyhow::Result<()> {
        use crate::schema::todos::dsl::*;

        diesel::delete(todos.filter(id.eq(todo_id)))
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn set_completed(
        conn: &mut AsyncPgConnection,
        todo_id: Uuid,
        is_completed: bool,
    ) -> anyhow::Result<Todo> {
        use crate::schema::todos::dsl::*;

        let updated = diesel::update(todos.filter(id.eq(todo_id)))
            .set((completed.eq(is_completed), updated_at.eq(Utc::now())))
            .get_result::<Todo>(conn)
            .await?;

        Ok(updated)
    }
}

// Email database operations
#[allow(dead_code)]
pub mod emails {
    use super::*;
    use chrono::DateTime;

    /// Email model matching database schema
    #[derive(Debug, Clone, Queryable, diesel::Selectable)]
    #[diesel(table_name = crate::schema::emails)]
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

    pub async fn list_all(
        conn: &mut AsyncPgConnection,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> anyhow::Result<Vec<Email>> {
        use crate::schema::emails::dsl::*;

        let mut query = emails.order_by(received_at.desc()).into_boxed();

        if let Some(l) = limit {
            query = query.limit(l);
        }
        if let Some(o) = offset {
            query = query.offset(o);
        }

        let items = query.load::<Email>(conn).await?;
        Ok(items)
    }

    pub async fn get_by_id(conn: &mut AsyncPgConnection, email_id: Uuid) -> anyhow::Result<Email> {
        use crate::schema::emails::dsl::*;

        let email = emails.filter(id.eq(email_id)).first::<Email>(conn).await?;
        Ok(email)
    }

    pub async fn list_by_account(
        conn: &mut AsyncPgConnection,
        acc_id: Uuid,
        limit: Option<i64>,
    ) -> anyhow::Result<Vec<Email>> {
        use crate::schema::emails::dsl::*;

        let mut query = emails
            .filter(account_id.eq(acc_id))
            .order_by(received_at.desc())
            .into_boxed();

        if let Some(l) = limit {
            query = query.limit(l);
        }

        let items = query.load::<Email>(conn).await?;
        Ok(items)
    }

    pub async fn list_unprocessed(
        conn: &mut AsyncPgConnection,
        limit: i64,
    ) -> anyhow::Result<Vec<Email>> {
        use crate::schema::emails::dsl::*;

        let items = emails
            .filter(processed.eq(false))
            .order_by(fetched_at.asc())
            .limit(limit)
            .load::<Email>(conn)
            .await?;

        Ok(items)
    }

    pub async fn count_all(conn: &mut AsyncPgConnection) -> anyhow::Result<i64> {
        use crate::schema::emails::dsl::*;

        let count: i64 = emails.count().get_result(conn).await?;
        Ok(count)
    }

    pub async fn count_unprocessed(conn: &mut AsyncPgConnection) -> anyhow::Result<i64> {
        use crate::schema::emails::dsl::*;

        let count: i64 = emails
            .filter(processed.eq(false))
            .count()
            .get_result(conn)
            .await?;
        Ok(count)
    }

    /// Insert a new email, returning None if it already exists (by gmail_id)
    pub async fn insert(
        conn: &mut AsyncPgConnection,
        new_email: NewEmail,
    ) -> anyhow::Result<Option<Email>> {
        use crate::schema::emails::dsl::*;

        let result = diesel::insert_into(emails)
            .values(&new_email)
            .on_conflict(gmail_id)
            .do_nothing()
            .get_result::<Email>(conn)
            .await
            .optional()?;

        Ok(result)
    }

    /// Mark an email as processed
    pub async fn mark_processed(
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
}

// Category database operations
pub mod categories {
    use super::*;

    pub async fn list_all(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<Category>> {
        use crate::schema::categories::dsl::*;

        let items = categories
            .order_by(name.asc())
            .load::<Category>(conn)
            .await?;

        Ok(items)
    }

    pub async fn get_by_id(
        conn: &mut AsyncPgConnection,
        category_id: Uuid,
    ) -> anyhow::Result<Category> {
        use crate::schema::categories::dsl::*;

        let category = categories
            .filter(id.eq(category_id))
            .first::<Category>(conn)
            .await?;

        Ok(category)
    }

    pub async fn create(
        conn: &mut AsyncPgConnection,
        name_val: &str,
        color_val: Option<&str>,
    ) -> anyhow::Result<Category> {
        use crate::schema::categories::dsl::*;

        let new_category = diesel::insert_into(categories)
            .values((name.eq(name_val), color.eq(color_val)))
            .get_result::<Category>(conn)
            .await?;

        Ok(new_category)
    }

    pub async fn update(
        conn: &mut AsyncPgConnection,
        category_id: Uuid,
        name_val: Option<&str>,
        color_val: Option<&str>,
    ) -> anyhow::Result<Category> {
        use crate::schema::categories::dsl::*;

        // Update fields
        if let Some(n) = name_val {
            diesel::update(categories.filter(id.eq(category_id)))
                .set(name.eq(n))
                .execute(conn)
                .await?;
        }
        if let Some(c) = color_val {
            diesel::update(categories.filter(id.eq(category_id)))
                .set(color.eq(Some(c)))
                .execute(conn)
                .await?;
        }

        // Always update updated_at
        diesel::update(categories.filter(id.eq(category_id)))
            .set(updated_at.eq(Utc::now()))
            .execute(conn)
            .await?;

        get_by_id(conn, category_id).await
    }

    pub async fn delete(conn: &mut AsyncPgConnection, category_id: Uuid) -> anyhow::Result<()> {
        use crate::schema::categories::dsl::*;

        diesel::delete(categories.filter(id.eq(category_id)))
            .execute(conn)
            .await?;

        Ok(())
    }
}

// Agent decision database operations
#[allow(dead_code)]
pub mod decisions {
    use super::*;

    pub async fn list_all(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<AgentDecision>> {
        use crate::schema::agent_decisions::dsl::*;

        let rows = agent_decisions
            .order_by(created_at.desc())
            .load::<AgentDecisionRow>(conn)
            .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_pending(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<AgentDecision>> {
        use crate::schema::agent_decisions::dsl::*;

        let rows = agent_decisions
            .filter(status.eq("proposed"))
            .order_by(created_at.desc())
            .load::<AgentDecisionRow>(conn)
            .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_by_status(
        conn: &mut AsyncPgConnection,
        status_filter: &str,
    ) -> anyhow::Result<Vec<AgentDecision>> {
        use crate::schema::agent_decisions::dsl::*;

        let rows = agent_decisions
            .filter(status.eq(status_filter))
            .order_by(created_at.desc())
            .load::<AgentDecisionRow>(conn)
            .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_by_source(
        conn: &mut AsyncPgConnection,
        source_type_filter: &str,
    ) -> anyhow::Result<Vec<AgentDecision>> {
        use crate::schema::agent_decisions::dsl::*;

        let rows = agent_decisions
            .filter(source_type.eq(source_type_filter))
            .order_by(created_at.desc())
            .load::<AgentDecisionRow>(conn)
            .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_by_id(
        conn: &mut AsyncPgConnection,
        decision_id: Uuid,
    ) -> anyhow::Result<AgentDecision> {
        use crate::schema::agent_decisions::dsl::*;

        let row = agent_decisions
            .filter(id.eq(decision_id))
            .first::<AgentDecisionRow>(conn)
            .await?;

        Ok(row.into())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        conn: &mut AsyncPgConnection,
        source_type_val: &str,
        source_id_val: Option<Uuid>,
        source_external_id_val: Option<&str>,
        decision_type_val: &str,
        proposed_action_val: serde_json::Value,
        reasoning_val: &str,
        reasoning_details_val: Option<serde_json::Value>,
        confidence_val: f32,
    ) -> anyhow::Result<AgentDecision> {
        use crate::schema::agent_decisions::dsl::*;

        // Serialize JSON values to strings for TEXT storage
        let proposed_action_str = serde_json::to_string(&proposed_action_val)?;
        let reasoning_details_str = reasoning_details_val
            .map(|v| serde_json::to_string(&v))
            .transpose()?;

        let row = diesel::insert_into(agent_decisions)
            .values((
                source_type.eq(source_type_val),
                source_id.eq(source_id_val),
                source_external_id.eq(source_external_id_val),
                decision_type.eq(decision_type_val),
                proposed_action.eq(proposed_action_str),
                reasoning.eq(reasoning_val),
                reasoning_details.eq(reasoning_details_str),
                confidence.eq(confidence_val),
                status.eq("proposed"),
            ))
            .get_result::<AgentDecisionRow>(conn)
            .await?;

        Ok(row.into())
    }

    pub async fn approve(
        conn: &mut AsyncPgConnection,
        decision_id: Uuid,
        todo_id: Option<Uuid>,
    ) -> anyhow::Result<AgentDecision> {
        use crate::schema::agent_decisions::dsl::*;

        let row = diesel::update(agent_decisions.filter(id.eq(decision_id)))
            .set((
                status.eq("approved"),
                result_todo_id.eq(todo_id),
                reviewed_at.eq(Some(Utc::now())),
            ))
            .get_result::<AgentDecisionRow>(conn)
            .await?;

        Ok(row.into())
    }

    pub async fn reject(
        conn: &mut AsyncPgConnection,
        decision_id: Uuid,
        feedback: Option<&str>,
    ) -> anyhow::Result<AgentDecision> {
        use crate::schema::agent_decisions::dsl::*;

        let row = diesel::update(agent_decisions.filter(id.eq(decision_id)))
            .set((
                status.eq("rejected"),
                user_feedback.eq(feedback),
                reviewed_at.eq(Some(Utc::now())),
            ))
            .get_result::<AgentDecisionRow>(conn)
            .await?;

        Ok(row.into())
    }

    pub async fn mark_executed(
        conn: &mut AsyncPgConnection,
        decision_id: Uuid,
    ) -> anyhow::Result<AgentDecision> {
        use crate::schema::agent_decisions::dsl::*;

        let row = diesel::update(agent_decisions.filter(id.eq(decision_id)))
            .set((status.eq("executed"), executed_at.eq(Some(Utc::now()))))
            .get_result::<AgentDecisionRow>(conn)
            .await?;

        Ok(row.into())
    }

    pub async fn mark_failed(
        conn: &mut AsyncPgConnection,
        decision_id: Uuid,
        error_msg: &str,
    ) -> anyhow::Result<AgentDecision> {
        use crate::schema::agent_decisions::dsl::*;

        let row = diesel::update(agent_decisions.filter(id.eq(decision_id)))
            .set((
                status.eq("failed"),
                user_feedback.eq(Some(error_msg)),
                executed_at.eq(Some(Utc::now())),
            ))
            .get_result::<AgentDecisionRow>(conn)
            .await?;

        Ok(row.into())
    }

    pub async fn auto_approve(
        conn: &mut AsyncPgConnection,
        decision_id: Uuid,
        rule_id: Uuid,
    ) -> anyhow::Result<AgentDecision> {
        use crate::schema::agent_decisions::dsl::*;

        let row = diesel::update(agent_decisions.filter(id.eq(decision_id)))
            .set((
                status.eq("auto_approved"),
                applied_rule_id.eq(Some(rule_id)),
                reviewed_at.eq(Some(Utc::now())),
            ))
            .get_result::<AgentDecisionRow>(conn)
            .await?;

        Ok(row.into())
    }

    pub async fn get_stats(
        conn: &mut AsyncPgConnection,
    ) -> anyhow::Result<shared_types::DecisionStats> {
        use crate::schema::agent_decisions::dsl::*;
        use diesel::dsl::count_star;

        let total: i64 = agent_decisions.select(count_star()).first(conn).await?;

        let pending_count: i64 = agent_decisions
            .filter(status.eq("proposed"))
            .select(count_star())
            .first(conn)
            .await?;

        let approved_count: i64 = agent_decisions
            .filter(status.eq("approved").or(status.eq("executed")))
            .select(count_star())
            .first(conn)
            .await?;

        let rejected_count: i64 = agent_decisions
            .filter(status.eq("rejected"))
            .select(count_star())
            .first(conn)
            .await?;

        let auto_approved_count: i64 = agent_decisions
            .filter(status.eq("auto_approved"))
            .select(count_star())
            .first(conn)
            .await?;

        // Calculate average confidence
        let avg_confidence: Option<f32> = agent_decisions
            .select(diesel::dsl::avg(confidence))
            .first::<Option<f64>>(conn)
            .await?
            .map(|v| v as f32);

        Ok(shared_types::DecisionStats {
            total,
            pending: pending_count,
            approved: approved_count,
            rejected: rejected_count,
            auto_approved: auto_approved_count,
            average_confidence: avg_confidence.unwrap_or(0.0),
        })
    }

    pub async fn delete(conn: &mut AsyncPgConnection, decision_id: Uuid) -> anyhow::Result<()> {
        use crate::schema::agent_decisions::dsl::*;

        diesel::delete(agent_decisions.filter(id.eq(decision_id)))
            .execute(conn)
            .await?;

        Ok(())
    }

    /// Create a decision with status and optional applied rule (used by processor)
    #[allow(clippy::too_many_arguments)]
    pub async fn create_with_status(
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

        let row = diesel::insert_into(agent_decisions)
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
            .get_result::<AgentDecisionRow>(conn)
            .await?;

        Ok(row.id)
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

        let row = diesel::insert_into(todos)
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
            .get_result::<Todo>(conn)
            .await?;

        Ok(row.id)
    }

    /// Update a decision with the resulting todo ID
    pub async fn update_result_todo(
        conn: &mut AsyncPgConnection,
        decision_id_val: Uuid,
        todo_id_val: Uuid,
    ) -> anyhow::Result<()> {
        use crate::schema::agent_decisions::dsl::*;

        diesel::update(agent_decisions.filter(id.eq(decision_id_val)))
            .set(result_todo_id.eq(Some(todo_id_val)))
            .execute(conn)
            .await?;

        Ok(())
    }
}

// Agent rules database operations
#[allow(dead_code)]
pub mod agent_rules {
    use super::*;
    use shared_types::{CreateAgentRuleRequest, RuleActionParams, RuleConditions};

    pub async fn list_all(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<AgentRule>> {
        use crate::schema::agent_rules::dsl::*;

        let items = agent_rules
            .order_by((priority.desc(), created_at.desc()))
            .load::<AgentRule>(conn)
            .await?;

        Ok(items)
    }

    pub async fn list_active(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<AgentRule>> {
        use crate::schema::agent_rules::dsl::*;

        let items = agent_rules
            .filter(is_active.eq(true))
            .order_by((priority.desc(), created_at.desc()))
            .load::<AgentRule>(conn)
            .await?;

        Ok(items)
    }

    pub async fn list_by_source_type(
        conn: &mut AsyncPgConnection,
        source: &str,
    ) -> anyhow::Result<Vec<AgentRule>> {
        use crate::schema::agent_rules::dsl::*;

        let items = agent_rules
            .filter(source_type.eq(source).or(source_type.eq("any")))
            .filter(is_active.eq(true))
            .order_by((priority.desc(), created_at.desc()))
            .load::<AgentRule>(conn)
            .await?;

        Ok(items)
    }

    /// Alias for list_by_source_type - used by processor module
    pub async fn list_active_for_source(
        conn: &mut AsyncPgConnection,
        source: &str,
    ) -> anyhow::Result<Vec<AgentRule>> {
        list_by_source_type(conn, source).await
    }

    pub async fn get_by_id(
        conn: &mut AsyncPgConnection,
        rule_id: Uuid,
    ) -> anyhow::Result<AgentRule> {
        use crate::schema::agent_rules::dsl::*;

        let rule = agent_rules
            .filter(id.eq(rule_id))
            .first::<AgentRule>(conn)
            .await?;

        Ok(rule)
    }

    pub async fn create(
        conn: &mut AsyncPgConnection,
        request: &CreateAgentRuleRequest,
    ) -> anyhow::Result<AgentRule> {
        use crate::schema::agent_rules::dsl::*;

        let conditions_json = serde_json::to_string(&request.conditions)?;
        let action_params_json = request
            .action_params
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        let new_rule = diesel::insert_into(agent_rules)
            .values((
                name.eq(&request.name),
                description.eq(&request.description),
                source_type.eq(&request.source_type),
                rule_type.eq(&request.rule_type),
                conditions.eq(&conditions_json),
                action.eq(&request.action),
                action_params.eq(&action_params_json),
                priority.eq(request.priority.unwrap_or(0)),
                is_active.eq(request.is_active.unwrap_or(true)),
                created_from_decision_id.eq(&request.created_from_decision_id),
            ))
            .get_result::<AgentRule>(conn)
            .await?;

        Ok(new_rule)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        conn: &mut AsyncPgConnection,
        rule_id: Uuid,
        name_val: Option<&str>,
        description_val: Option<&str>,
        source_type_val: Option<&str>,
        rule_type_val: Option<&str>,
        conditions_val: Option<&RuleConditions>,
        action_val: Option<&str>,
        action_params_val: Option<&RuleActionParams>,
        priority_val: Option<i32>,
        is_active_val: Option<bool>,
    ) -> anyhow::Result<AgentRule> {
        use crate::schema::agent_rules::dsl::*;

        if let Some(n) = name_val {
            diesel::update(agent_rules.filter(id.eq(rule_id)))
                .set(name.eq(n))
                .execute(conn)
                .await?;
        }
        if let Some(d) = description_val {
            diesel::update(agent_rules.filter(id.eq(rule_id)))
                .set(description.eq(Some(d)))
                .execute(conn)
                .await?;
        }
        if let Some(st) = source_type_val {
            diesel::update(agent_rules.filter(id.eq(rule_id)))
                .set(source_type.eq(st))
                .execute(conn)
                .await?;
        }
        if let Some(rt) = rule_type_val {
            diesel::update(agent_rules.filter(id.eq(rule_id)))
                .set(rule_type.eq(rt))
                .execute(conn)
                .await?;
        }
        if let Some(c) = conditions_val {
            let conditions_json = serde_json::to_string(c)?;
            diesel::update(agent_rules.filter(id.eq(rule_id)))
                .set(conditions.eq(conditions_json))
                .execute(conn)
                .await?;
        }
        if let Some(a) = action_val {
            diesel::update(agent_rules.filter(id.eq(rule_id)))
                .set(action.eq(a))
                .execute(conn)
                .await?;
        }
        if let Some(ap) = action_params_val {
            let action_params_json = serde_json::to_string(ap)?;
            diesel::update(agent_rules.filter(id.eq(rule_id)))
                .set(action_params.eq(Some(action_params_json)))
                .execute(conn)
                .await?;
        }
        if let Some(p) = priority_val {
            diesel::update(agent_rules.filter(id.eq(rule_id)))
                .set(priority.eq(p))
                .execute(conn)
                .await?;
        }
        if let Some(active) = is_active_val {
            diesel::update(agent_rules.filter(id.eq(rule_id)))
                .set(is_active.eq(active))
                .execute(conn)
                .await?;
        }

        // Always update updated_at
        diesel::update(agent_rules.filter(id.eq(rule_id)))
            .set(updated_at.eq(Utc::now()))
            .execute(conn)
            .await?;

        get_by_id(conn, rule_id).await
    }

    pub async fn delete(conn: &mut AsyncPgConnection, rule_id: Uuid) -> anyhow::Result<()> {
        use crate::schema::agent_rules::dsl::*;

        diesel::delete(agent_rules.filter(id.eq(rule_id)))
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn increment_match_count(
        conn: &mut AsyncPgConnection,
        rule_id: Uuid,
    ) -> anyhow::Result<AgentRule> {
        use crate::schema::agent_rules::dsl::*;

        let updated = diesel::update(agent_rules.filter(id.eq(rule_id)))
            .set((
                match_count.eq(match_count + 1),
                last_matched_at.eq(Some(Utc::now())),
            ))
            .get_result::<AgentRule>(conn)
            .await?;

        Ok(updated)
    }

    pub async fn set_active(
        conn: &mut AsyncPgConnection,
        rule_id: Uuid,
        active: bool,
    ) -> anyhow::Result<AgentRule> {
        use crate::schema::agent_rules::dsl::*;

        let updated = diesel::update(agent_rules.filter(id.eq(rule_id)))
            .set((is_active.eq(active), updated_at.eq(Utc::now())))
            .get_result::<AgentRule>(conn)
            .await?;

        Ok(updated)
    }
}
