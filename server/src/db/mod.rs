pub mod models;

use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn create_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    // Migrations are embedded at compile time from src/db/migrations/
    sqlx::migrate!("src/db/migrations").run(&pool).await?;

    Ok(pool)
}
