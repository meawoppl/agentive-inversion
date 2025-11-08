use diesel_async::{pooled_connection::deadpool::Pool, AsyncPgConnection};

#[allow(dead_code)]
pub type DbPool = Pool<AsyncPgConnection>;

#[allow(dead_code)]
pub fn establish_connection_pool() -> anyhow::Result<DbPool> {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let config =
        diesel_async::pooled_connection::AsyncDieselConnectionManager::<AsyncPgConnection>::new(
            database_url,
        );
    let pool = Pool::builder(config).build()?;

    Ok(pool)
}
