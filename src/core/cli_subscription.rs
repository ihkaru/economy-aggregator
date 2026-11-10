//! CLI handlers for managing data subscriptions (subscribe, unsubscribe, list).

use crate::core::config::LakehouseConfig;
use crate::core::subscription::{SubscriptionItem, SubscriptionManager};

pub async fn handle_subscribe(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut canonical = String::new();
    let mut raw_symbol = String::new();
    let mut source = "yahoo".to_string();
    let mut asset_class = "equities".to_string();
    let mut timeframes = vec!["1d".to_string()];
    let mut poll_interval_secs = None;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--canonical" | "-c" if i + 1 < args.len() => {
                canonical = args[i + 1].clone();
                i += 2;
            }
            "--symbol" | "-s" if i + 1 < args.len() => {
                raw_symbol = args[i + 1].clone();
                i += 2;
            }
            "--source" if i + 1 < args.len() => {
                source = args[i + 1].to_lowercase();
                i += 2;
            }
            "--asset" | "-a" if i + 1 < args.len() => {
                asset_class = args[i + 1].to_lowercase();
                i += 2;
            }
            "--timeframes" | "--tf" if i + 1 < args.len() => {
                timeframes = args[i + 1].split(',').map(|s| s.trim().to_string()).collect();
                i += 2;
            }
            "--interval" if i + 1 < args.len() => {
                poll_interval_secs = args[i + 1].parse::<u64>().ok();
                i += 2;
            }
            "--json" => {
                json_mode = true;
                i += 1;
            }
            _ => { i += 1; }
        }
    }

    if raw_symbol.is_empty() {
        eprintln!("Error: Missing required --symbol flag. Example: economy-aggregator subscribe --symbol BBRI.JK --canonical BBRI");
        std::process::exit(1);
    }
    if canonical.is_empty() {
        canonical = raw_symbol.trim_start_matches('^').replace(".JK", "").replace("=X", "").to_uppercase();
    }

    let config = LakehouseConfig::default();
    let sub_manager = SubscriptionManager::new(SubscriptionManager::default_path(&config.base_dir));

    let item = SubscriptionItem {
        canonical: canonical.clone(),
        raw_symbol: raw_symbol.clone(),
        asset_class: asset_class.clone(),
        source: source.clone(),
        timeframes: timeframes.clone(),
        enabled: true,
        poll_interval_secs,
        created_at_utc: chrono::Utc::now().to_rfc3339(),
    };

    let updated = sub_manager.subscribe(item.clone())?;

    if json_mode {
        let envelope = serde_json::json!({
            "success": true,
            "action": if updated { "updated" } else { "created" },
            "subscription": item,
            "next_steps": [
                format!("To pull historical catch-up now: economy-aggregator catch-up --symbol {}", canonical),
                "To list all active subscriptions: economy-aggregator subscriptions list"
            ]
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        let verb = if updated { "Updated" } else { "Created" };
        println!("✅ Successfully {} subscription for canonical '{}' (Raw: {}, Source: {}, TF: {:?})",
            verb, canonical, raw_symbol, source, timeframes
        );
    }

    Ok(())
}

pub async fn handle_unsubscribe(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut canonical = String::new();
    let mut source: Option<String> = None;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--canonical" | "-c" if i + 1 < args.len() => {
                canonical = args[i + 1].clone();
                i += 2;
            }
            "--source" | "-s" if i + 1 < args.len() => {
                source = Some(args[i + 1].to_lowercase());
                i += 2;
            }
            "--json" => {
                json_mode = true;
                i += 1;
            }
            _ => {
                if canonical.is_empty() && !args[i].starts_with('-') {
                    canonical = args[i].clone();
                }
                i += 1;
            }
        }
    }

    if canonical.is_empty() {
        eprintln!("Error: Missing required canonical symbol. Example: economy-aggregator unsubscribe --canonical BBRI [--source yahoo]");
        std::process::exit(1);
    }

    let config = LakehouseConfig::default();
    let sub_manager = SubscriptionManager::new(SubscriptionManager::default_path(&config.base_dir));
    let removed = sub_manager.unsubscribe(source.as_deref(), &canonical)?;

    if json_mode {
        let envelope = serde_json::json!({
            "success": removed,
            "canonical": canonical,
            "source": source,
            "message": if removed {
                if let Some(src) = &source {
                    format!("Subscription for canonical '{}' (source: {}) removed successfully", canonical, src)
                } else {
                    format!("All subscriptions for canonical '{}' removed successfully", canonical)
                }
            } else {
                "Subscription not found".to_string()
            }
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else if removed {
        if let Some(src) = &source {
            println!("🗑️  Successfully unsubscribed from canonical '{}' (source: {})", canonical, src);
        } else {
            println!("🗑️  Successfully unsubscribed from canonical '{}' across all sources", canonical);
        }
    } else {
        println!("⚠️  Subscription for canonical '{}' (source: {:?}) not found", canonical, source);
    }

    Ok(())
}

pub async fn handle_subscriptions_list(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let json_mode = args.iter().any(|a| a == "--json");
    let config = LakehouseConfig::default();
    let sub_manager = SubscriptionManager::new(SubscriptionManager::default_path(&config.base_dir));
    let items = sub_manager.load()?;

    if json_mode {
        let envelope = serde_json::json!({
            "success": true,
            "count": items.len(),
            "subscriptions": items,
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("📋 Active Ingestion Subscriptions (Total: {}):", items.len());
        println!("{:-<90}", "");
        println!("{:<14} {:<16} {:<12} {:<10} {:<12} {:<10}", "CANONICAL", "RAW SYMBOL", "ASSET", "SOURCE", "TIMEFRAMES", "STATUS");
        println!("{:-<90}", "");
        for it in &items {
            let status = if it.enabled { "ENABLED" } else { "DISABLED" };
            let tf_str = it.timeframes.join(",");
            println!("{:<14} {:<16} {:<12} {:<10} {:<12} {:<10}", it.canonical, it.raw_symbol, it.asset_class, it.source, tf_str, status);
        }
        println!("{:-<90}", "");
    }

    Ok(())
}
