mod config;
mod protocol;
mod server;
mod zone;

use anyhow::{Context, Result};
use config::Config;
use protocol::QueryProcessor;
use server::DnsServer;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use zone::ZoneStore;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let config_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("lrmdns.yaml")
    };

    // Load configuration
    let config = Config::from_file(&config_path)
        .context(format!("Failed to load config from {}", config_path.display()))?;

    // Initialize logging
    let log_level = config.server.log_level.clone();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("lrmdns={}", log_level).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting lrmdns authoritative DNS server");
    tracing::info!("Configuration loaded from: {}", config_path.display());

    // Validate configuration
    config.validate()
        .context("Configuration validation failed")?;

    // Load all zones
    let mut zone_store = ZoneStore::new();
    for zone_config in &config.zones {
        tracing::info!("Loading zone: {} from {}", zone_config.name, zone_config.file.display());

        let zone = zone::parse_zone_file(&zone_config.file, &zone_config.name)
            .context(format!("Failed to load zone {}", zone_config.name))?;

        let record_count: usize = zone.records.values()
            .map(|type_map| type_map.values().map(|v| v.len()).sum::<usize>())
            .sum();

        tracing::info!(
            "Zone {} loaded: {} records",
            zone_config.name,
            record_count
        );

        zone_store.add_zone(zone);
    }

    // Create query processor
    let processor = QueryProcessor::new(Arc::new(zone_store));

    // Create and run DNS server
    let server = DnsServer::new(processor, config.server.listen.clone());

    tracing::info!("DNS server starting on {}", config.server.listen);

    server.run().await
}
