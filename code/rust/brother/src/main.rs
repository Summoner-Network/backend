mod service;
mod db;

use service::BrotherService;
use brother::pb::brother_server::BrotherServer;
use tonic::transport::Server;
use tracing_subscriber::{EnvFilter};
use std::net::SocketAddr;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    
    // ---------- logging ----------
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx::query=warn".into()),
        )
        .init();

    // ---------- DB connection ----------
    let pool = db::init_pool().await?;        // implement this in db.rs

    // ---------- gRPC server ----------
    let addr: SocketAddr = "[::1]:42069".parse()?;
    let svc  = BrotherService::new(pool);
    tracing::info!("Brother gRPC server listening on {}", addr);

    Server::builder()
        .add_service(BrotherServer::new(svc))
        .serve(addr)
        .await?;

    Ok(())
}