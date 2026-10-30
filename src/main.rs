use economy_aggregator::core::config::LakehouseConfig;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = LakehouseConfig::default();
    config.ensure_directories()?;

    info!("🚀 Economy Aggregator Engine started (Full Rust)");
    info!("📂 Parquet Lake: {}", config.lake_dir.display());
    info!("⏱️  Flush interval: {}s (max {} rows)", config.micro_batch_flush_seconds, config.micro_batch_max_rows);

    Ok(())
}
