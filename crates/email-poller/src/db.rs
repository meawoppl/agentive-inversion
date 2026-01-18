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
