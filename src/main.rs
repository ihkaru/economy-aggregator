use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tracing::info;

use economy_aggregator::core::config::LakehouseConfig;
use economy_aggregator::core::types::{AssetClass, InstrumentType};
use economy_aggregator::features::serving::{create_router, AppState};
use economy_aggregator::features::storage::{
    InstrumentMapping, InstrumentRegistry, LakeReader, PartitionResolver,
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
            isin: None,
            figi: Some("BBG0028KLS37".to_string()),
            perm_id: None,
            aliases: Vec::new(),
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
            isin: None,
            figi: Some("BBG001S5TD05".to_string()),
            perm_id: None,
            aliases: Vec::new(),
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
            isin: None,
            figi: None,
            perm_id: None,
            aliases: Vec::new(),
        });

        if let Some(parent) = meta_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = registry.save_to_file(&meta_file);
        info!("🌱 Seeded {} default instruments", registry.len());
    }

    let resolver = PartitionResolver::new(config.lake_dir.clone());
    let state = Arc::new(AppState::new(config.clone(), resolver, registry));

    // Asynchronous background cache pre-warmer for core instruments
    let prewarm_state = state.clone();
    tokio::spawn(async move {
        let targets = [
            ("equities", "BBCA"),
            ("equities", "GSPC"),
            ("crypto", "BTC_USDT"),
            ("forex", "USD_IDR"),
            ("commodities", "XAU_USD"),
        ];
        for (ac, sym) in targets {
            let _ = LakeReader::read_candles_with_limit(
                &prewarm_state.config.lake_dir,
                Some(ac),
                sym,
                "1d",
                None,
                None,
                Some(2000),
            ).map(|candles| {
                if !candles.is_empty() {
                    let key = format!("{}:{}:1d", ac, sym);
                    if let Ok(mut cache) = prewarm_state.candle_cache.write() {
                        cache.insert(key, (std::time::Instant::now(), candles));
                    }
                }
            });
        }
        info!("🔥 In-memory lake cache pre-warmed for core instruments");
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
