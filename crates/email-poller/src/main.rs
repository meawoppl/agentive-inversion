use anyhow::{Context, Result};
use diesel::prelude::*;
use diesel_async::{pooled_connection::deadpool::Pool, AsyncPgConnection, RunQueryDsl};
use tokio::time::{interval, Duration};

mod gmail_client;
mod processor;
mod schema;

use gmail_client::{EmailMessage, GmailClient};
use processor::process_email_to_todo;
use shared_types::EmailAccount;

type DbPool = Pool<AsyncPgConnection>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    tracing::info!("Starting email poller service");

    // Establish database connection
    let pool = establish_connection_pool()?;

    let mut interval = interval(Duration::from_secs(300));

    loop {
        interval.tick().await;

        if let Err(e) = poll_emails(&pool).await {
            tracing::error!("Error polling emails: {}", e);
        }
    }
}

fn establish_connection_pool() -> Result<DbPool> {
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

    let config =
        diesel_async::pooled_connection::AsyncDieselConnectionManager::<AsyncPgConnection>::new(
            database_url,
        );
    let pool = Pool::builder(config).build()?;

    Ok(pool)
}

async fn poll_emails(pool: &DbPool) -> Result<()> {
    tracing::info!("Polling emails from all accounts...");

    let mut conn = pool.get().await.context("Failed to get DB connection")?;

    // Get all active email accounts
    let accounts = get_active_accounts(&mut conn).await?;

    tracing::info!("Found {} active email accounts", accounts.len());

    for account in accounts {
        if let Err(e) = poll_account(&mut conn, &account).await {
            tracing::error!(
                "Failed to poll account {} ({}): {}",
                account.account_name,
                account.email_address,
                e
            );

            // Update sync status to failed
            update_account_sync_status(&mut conn, account.id, "failed", Some(&e.to_string()), None)
                .await
                .ok();
        }
    }

    Ok(())
}

async fn get_active_accounts(conn: &mut AsyncPgConnection) -> Result<Vec<EmailAccount>> {
    use schema::email_accounts::dsl::*;

    let accounts = email_accounts
        .filter(is_active.eq(true))
        .filter(oauth_refresh_token.is_not_null())
        .order_by(last_synced.asc().nulls_first())
        .load::<EmailAccount>(conn)
        .await?;

    Ok(accounts)
}

async fn poll_account(conn: &mut AsyncPgConnection, account: &EmailAccount) -> Result<()> {
    tracing::info!(
        "Polling account: {} ({})",
        account.account_name,
        account.email_address
    );

    // Update status to syncing
    update_account_sync_status(conn, account.id, "syncing", None, None).await?;

    // Create Gmail client
    let client = GmailClient::new(account)
        .await
        .context("Failed to create Gmail client")?;

    // Fetch emails
    let emails = if let Some(last_msg_id) = &account.last_message_id {
        client.fetch_emails_since(last_msg_id, 50).await?
    } else {
        client.fetch_recent_emails(10).await?
    };

    tracing::info!(
        "Found {} new emails for {}",
        emails.len(),
        account.email_address
    );

    // Process emails into todos
    let mut last_message_id = account.last_message_id.clone();
    for email in &emails {
        if let Some(todo) = process_email_to_todo(email, account) {
            // Insert todo into database
            if let Err(e) = insert_todo(conn, &todo, account.id).await {
                tracing::error!("Failed to insert todo from email {}: {}", email.id, e);
            } else {
                tracing::info!("Created todo from email: {}", email.subject);
            }
        }

        // Track the latest message ID
        last_message_id = Some(email.id.clone());
    }

    // Update sync status to success
    update_account_sync_status(
        conn,
        account.id,
        "success",
        None,
        last_message_id.as_deref(),
    )
    .await?;

    Ok(())
}

async fn update_account_sync_status(
    conn: &mut AsyncPgConnection,
    account_id: uuid::Uuid,
    status: &str,
    error: Option<&str>,
    last_msg_id: Option<&str>,
) -> Result<()> {
    use schema::email_accounts::dsl::*;

    diesel::update(email_accounts.filter(id.eq(account_id)))
        .set((
            sync_status.eq(status),
            last_sync_error.eq(error),
            last_message_id.eq(last_msg_id),
            last_synced.eq(Some(chrono::Utc::now())),
        ))
        .execute(conn)
        .await?;

    Ok(())
}

async fn insert_todo(
    conn: &mut AsyncPgConnection,
    todo: &shared_types::Todo,
    account_id: uuid::Uuid,
) -> Result<()> {
    use schema::todos::dsl::*;

    diesel::insert_into(todos)
        .values((
            title.eq(&todo.title),
            description.eq(&todo.description),
            completed.eq(false),
            source.eq("email"),
            source_id.eq(Some(account_id.to_string())),
            due_date.eq(todo.due_date),
            created_at.eq(chrono::Utc::now()),
            updated_at.eq(chrono::Utc::now()),
        ))
        .execute(conn)
        .await?;

    Ok(())
}
