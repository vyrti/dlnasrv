mod config;
mod error;
mod media;
mod ssdp;
mod web;

use anyhow::Context;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tracing::info;

use config::Config;
use state::AppState;

// Publicly export the AppState for use in other modules
pub mod state {
    use crate::{config::Config, media::MediaFile};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[derive(Clone)]
    pub struct AppState {
        pub config: Arc<Config>,
        pub media_files: Arc<RwLock<Vec<MediaFile>>>,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Parse configuration from command line arguments
    let config = Arc::new(Config::from_args().await?);

    info!("Starting DLNA server...");
    info!("Media directory: {}", config.media_dir.display());

    // Scan for media files
    let media_files = media::scan_media_files(&config.media_dir)
        .await
        .context("Failed to scan media library")?;
    info!("Found {} media files", media_files.len());

    // Create shared application state
    let app_state = AppState {
        config: config.clone(),
        media_files: Arc::new(RwLock::new(media_files)),
    };

    // Start SSDP discovery service in the background
    ssdp::run_ssdp_service(app_state.clone())
        .context("Failed to start SSDP service")?;

    // Create the Axum web server
    let app = web::create_router(app_state);

    // Start the server
    let addr = SocketAddr::new(config.host, config.port);
    info!("Server UUID: {}", config.uuid);
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service())
        .await
        .context("HTTP server failed")?;

    Ok(())
}