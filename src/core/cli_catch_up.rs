//! CLI handler for running the gap catch-up engine on-demand.

use crate::core::config::LakehouseConfig;
use crate::core::subscription::SubscriptionManager;
use crate::features::ingestion::catch_up::CatchUpEngine;

pub async fn handle_catch_up(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut symbol_filter = String::new();
    let mut force = false;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--symbol" | "-s" if i + 1 < args.len() => {
                symbol_filter = args[i + 1].clone();
                i += 2;
            }
            "--force" | "-f" => {
                force = true;
                i += 1;
            }
            "--json" => {
                json_mode = true;
                i += 1;
            }
            _ => {
                if symbol_filter.is_empty() && !args[i].starts_with('-') {
                    symbol_filter = args[i].clone();
                }
                i += 1;
            }
        }
    }

    let config = LakehouseConfig::default();
    let sub_manager = SubscriptionManager::new(SubscriptionManager::default_path(&config.base_dir));
    let subscriptions = sub_manager.list_active()?;

    let filtered: Vec<_> = if symbol_filter.is_empty() {
        subscriptions
    } else {
        subscriptions
            .into_iter()
            .filter(|s| s.canonical.eq_ignore_ascii_case(&symbol_filter))
            .collect()
    };

    if filtered.is_empty() {
        if json_mode {
            println!("{{\"success\":false,\"error\":\"No active subscriptions found matching filter\"}}");
        } else {
            println!("⚠️  No active subscriptions found matching filter '{}'", symbol_filter);
        }
        return Ok(());
    }

    let engine = CatchUpEngine::new(config);
    let mut all_summaries = Vec::new();

    if !json_mode {
        println!("🚀 Executing Autonomous Gap Catch-Up across {} subscriptions...", filtered.len());
        println!("{:-<95}", "");
        println!(
            "{:<14} {:<12} {:<10} {:<6} {:<10} {:<10} {:<20}",
            "CANONICAL", "ASSET", "SOURCE", "TF", "GAP (DAYS)", "INGESTED", "STATUS"
        );
        println!("{:-<95}", "");
    }

    for sub in &filtered {
        let summaries = engine.catch_up_subscription(sub, force).await;
        for s in &summaries {
            if !json_mode {
                let status_display = if let Some(err) = &s.error {
                    format!("ERR: {}", err)
                } else {
                    s.status.clone()
                };
                println!(
                    "{:<14} {:<12} {:<10} {:<6} {:<10} {:<10} {:<20}",
                    s.canonical_symbol, s.asset_class, s.source, s.timeframe, s.gap_days, s.candles_ingested, status_display
                );
            }
        }
        all_summaries.extend(summaries);
    }

    if json_mode {
        let total_ingested: usize = all_summaries.iter().map(|s| s.candles_ingested).sum();
        let envelope = serde_json::json!({
            "success": true,
            "target_count": filtered.len(),
            "total_candles_ingested": total_ingested,
            "summaries": all_summaries,
            "next_steps": [
                "To start continuous live ingestion daemon: economy-aggregator serve",
                "To verify lakehouse data integrity: economy-aggregator status"
            ]
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        let total_ingested: usize = all_summaries.iter().map(|s| s.candles_ingested).sum();
        println!("{:-<95}", "");
        println!("✨ Catch-up completed. Total new records/candles ingested: {}", total_ingested);
    }

    Ok(())
}
