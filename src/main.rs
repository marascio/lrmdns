mod config;
mod metrics;
mod protocol;
mod ratelimit;
mod server;
mod zone;

use anyhow::{Context, Result};
use config::Config;
use metrics::Metrics;
use protocol::QueryProcessor;
use ratelimit::RateLimiter;
use server::DnsServer;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
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
    let zone_store = Arc::new(RwLock::new(load_zones(&config)?));

    // Create metrics
    let metrics = Arc::new(Metrics::new());

    // Create rate limiter if configured
    let rate_limiter = config.server.rate_limit.map(|limit| Arc::new(RateLimiter::new(limit)));

    // Create query processor
    let processor = QueryProcessor::new(zone_store.clone());

    // Create and run DNS server
    let server = DnsServer::new(
        processor,
        config.server.listen.clone(),
        metrics.clone(),
        rate_limiter.clone(),
    );

    tracing::info!("DNS server starting on {}", config.server.listen);

    // Set up signal handlers
    let config_for_reload = config.clone();
    let zone_store_for_reload = zone_store.clone();
    let metrics_for_stats = metrics.clone();

    // Spawn signal handler tasks
    tokio::spawn(async move {
        handle_signals(config_for_reload, zone_store_for_reload, metrics_for_stats).await;
    });

    // Run the DNS server
    server.run().await
}

fn load_zones(config: &Config) -> Result<ZoneStore> {
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
    Ok(zone_store)
}

async fn handle_signals(
    config: Config,
    zone_store: Arc<RwLock<ZoneStore>>,
    metrics: Arc<Metrics>,
) {
    loop {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sighup = signal(SignalKind::hangup()).expect("Failed to register SIGHUP handler");
            let mut sigusr1 = signal(SignalKind::user_defined1()).expect("Failed to register SIGUSR1 handler");

            tokio::select! {
                _ = sighup.recv() => {
                    tracing::info!("Received SIGHUP, reloading zones...");
                    match load_zones(&config) {
                        Ok(new_store) => {
                            let mut store = zone_store.write().await;
                            *store = new_store;
                            tracing::info!("Zones reloaded successfully");
                        }
                        Err(e) => {
                            tracing::error!("Failed to reload zones: {}", e);
                        }
                    }
                }
                _ = sigusr1.recv() => {
                    tracing::info!("Received SIGUSR1, logging metrics...");
                    metrics.log_summary();
                }
            }
        }

        #[cfg(not(unix))]
        {
            // On non-Unix platforms, just wait for Ctrl+C
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("Shutting down...");
            metrics.log_summary();
            std::process::exit(0);
        }
    }
}
