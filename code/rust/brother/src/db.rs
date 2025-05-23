//! src/db.rs
//! Simple Postgres helper for the Brother service.

use std::time::Duration;

use sqlx::{
    migrate::Migrator,
    postgres::PgPoolOptions,
    Pool, Postgres
};
use tonic::Status;
use tracing::info;

/// Alias that the rest of the code uses.
pub type PgPool = Pool<Postgres>;

/// Compile-time embedded migration directory.
///
/// `sqlx::migrate!` expands to a `static MIGRATOR` that embeds every
/// `*.sql` file under the given path and applies them in lexicographic
/// order.
///
/// The path is **relative to the crate root** *at compile time* – adjust
/// if you move the `database/` folder elsewhere.
static MIGRATOR: Migrator = sqlx::migrate!("./src/database");

/// Build a pool, apply migrations, and hand it back.
///
/// * Reads `DATABASE_URL` from the environment.  
/// * Uses a small, sensible pool size for dev – tune in prod.  
/// * Runs the embedded migrations **exactly once**, even in concurrent
///   start-ups.
pub async fn init_pool() -> Result<PgPool, sqlx::Error> {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL env-var must be set");

    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await?;

    // Apply migrations (idempotent).
    MIGRATOR.run(&pool).await?;
    info!("✅ database ready – migrations are up-to-date");

    Ok(pool)
}

/// Map a database error to a tonic `Status`, so handlers can simply
/// `...? .await .map_err(db_err)?`.
pub fn db_err(e: sqlx::Error) -> Status {
    match e {
        sqlx::Error::RowNotFound => Status::not_found("record not found"),
        other => {
            tracing::error!("database error: {:?}", other);
            Status::internal("database error")
        }
    }
}

