use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::signal;
use tracing::info;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::types::{AssetClass, InstrumentType};
use economy_aggregator::features::serving::{create_router, AppState};
use economy_aggregator::features::storage::{
    InstrumentMapping, InstrumentRegistry, PartitionResolver,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] != "serve" {
        return economy_aggregator::core::cli::run_cli(&args[1..]).await.map_err(|e| -> Box<dyn std::error::Error> { e });
    }

    tracing_subscriber::fmt::init();

    let config = LakehouseConfig::default();
    config.ensure_directories()?;

    info!("🚀 Economy Aggregator Engine started (Full Rust)");
    info!("📂 Parquet Lake: {}", config.lake_dir.display());
    info!(
        "⏱️  Micro-batch flush interval: {}s (max {} rows)",
        config.micro_batch_flush_seconds, config.micro_batch_max_rows
    );

    // Initialize Symbology Registry
    let meta_file = config.lake_dir.join("meta").join("instruments.json");
    let mut registry = InstrumentRegistry::new();
    if meta_file.exists() {
        if let Err(e) = registry.load_from_file(&meta_file) {
            tracing::warn!("Failed loading existing instruments: {}", e);
        } else {
            info!("📖 Loaded {} instruments from metadata file", registry.len());
        }
    }

    if registry.is_empty() {
        // Seed default canonical instruments
        let mut btc_vendors = HashMap::new();
        btc_vendors.insert("binance".to_string(), "BTCUSDT".to_string());
        btc_vendors.insert("bybit".to_string(), "BTCUSDT".to_string());
        btc_vendors.insert("coinbase".to_string(), "BTC-USD".to_string());
        btc_vendors.insert("yahoo".to_string(), "BTC-USD".to_string());

        registry.register(InstrumentMapping {
            canonical_symbol: "BTC/USDT".to_string(),
            name: "Bitcoin / Tether USD".to_string(),
            asset_class: AssetClass::Crypto,
            instrument_type: InstrumentType::Spot,
            base_currency: "BTC".to_string(),
            quote_currency: "USDT".to_string(),
            market_timezone: "UTC".to_string(),
            is_24_7: true,
            vendor_symbols: btc_vendors,
            contract_address: None,
            network: None,
        });

        let mut eth_vendors = HashMap::new();
        eth_vendors.insert("binance".to_string(), "ETHUSDT".to_string());
        eth_vendors.insert("bybit".to_string(), "ETHUSDT".to_string());
        eth_vendors.insert("coinbase".to_string(), "ETH-USD".to_string());

        registry.register(InstrumentMapping {
            canonical_symbol: "ETH/USDT".to_string(),
            name: "Ethereum / Tether USD".to_string(),
            asset_class: AssetClass::Crypto,
            instrument_type: InstrumentType::Spot,
            base_currency: "ETH".to_string(),
            quote_currency: "USDT".to_string(),
            market_timezone: "UTC".to_string(),
            is_24_7: true,
            vendor_symbols: eth_vendors,
            contract_address: None,
            network: None,
        });

        let mut eur_vendors = HashMap::new();
        eur_vendors.insert("dukascopy".to_string(), "EURUSD".to_string());
        eur_vendors.insert("oanda".to_string(), "EUR_USD".to_string());
        eur_vendors.insert("yahoo".to_string(), "EURUSD=X".to_string());

        registry.register(InstrumentMapping {
            canonical_symbol: "EUR/USD".to_string(),
            name: "Euro / US Dollar".to_string(),
            asset_class: AssetClass::Forex,
            instrument_type: InstrumentType::Spot,
            base_currency: "EUR".to_string(),
            quote_currency: "USD".to_string(),
            market_timezone: "America/New_York".to_string(),
            is_24_7: false,
            vendor_symbols: eur_vendors,
            contract_address: None,
            network: None,
        });

        if let Some(parent) = meta_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = registry.save_to_file(&meta_file);
        info!("🌱 Seeded {} default instruments", registry.len());
    }

    let resolver = PartitionResolver::new(config.lake_dir.clone());
    let state = Arc::new(AppState {
        config: config.clone(),
        resolver,
        registry: RwLock::new(registry),
        start_time: Instant::now(),
    });

    let app = create_router(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    info!("🌐 REST Serving Layer listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("👋 Economy Aggregator Engine shutdown cleanly.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
