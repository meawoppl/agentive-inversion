use anyhow::{Context, Result};
use deadpool_diesel::postgres::{Manager, Pool};
use diesel::PgConnection;

pub type DbPool = Pool<Manager<PgConnection>>;
pub type DbConnection = deadpool_diesel::postgres::Object<Manager<PgConnection>>;

impl DbPool {
    pub async fn new(database_url: &str) -> Result<Self> {
        let manager = Manager::new(database_url, deadpool_diesel::Runtime::Tokio1);
        let pool = Pool::builder(manager)
            .max_size(10)
            .build()
            .context("Failed to create database pool")?;

        Ok(pool)
    }
}
