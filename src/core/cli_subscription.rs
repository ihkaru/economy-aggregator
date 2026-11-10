//! CLI handlers for managing data subscriptions (subscribe, unsubscribe, list).

use crate::core::config::LakehouseConfig;
use crate::core::subscription::{SubscriptionItem, SubscriptionManager};

pub async fn handle_subscribe(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut symbol = String::new();
    let mut source = "yahoo".to_string();
    let mut asset_class = "equities".to_string();
    let mut timeframes = vec!["1d".to_string()];
    let mut poll_interval_secs = None;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--symbol" | "-s" if i + 1 < args.len() => {
                symbol = args[i + 1].clone();
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
            "--canonical" | "-c" if i + 1 < args.len() => {
                // Backward-compatibility no-op: canonical is ignored
                i += 2;
            }
            _ => {
                if symbol.is_empty() && !args[i].starts_with('-') {
                    symbol = args[i].clone();
                }
                i += 1;
            }
        }
    }

    if symbol.is_empty() {
        eprintln!("Error: Missing required --symbol flag. Example: economy-aggregator subscribe --source yahoo --symbol BBRI.JK");
        std::process::exit(1);
    }

    let config = LakehouseConfig::default();
    let sub_manager = SubscriptionManager::new(SubscriptionManager::default_path(&config.base_dir));

    let item = SubscriptionItem {
        source: source.clone(),
        symbol: symbol.clone(),
        asset_class: asset_class.clone(),
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
                format!("To pull historical catch-up now: economy-aggregator catch-up --source {} --symbol {}", source, symbol),
                "To list all active subscriptions: economy-aggregator subscriptions list"
            ]
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        let verb = if updated { "Updated" } else { "Created" };
        println!("✅ Successfully {} subscription for symbol '{}' (Source: {}, Asset: {}, TF: {:?})",
            verb, symbol, source, asset_class, timeframes
        );
    }

    Ok(())
}

pub async fn handle_unsubscribe(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut symbol = String::new();
    let mut source: Option<String> = None;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--symbol" | "-s" if i + 1 < args.len() => {
                symbol = args[i + 1].clone();
                i += 2;
            }
            "--source" if i + 1 < args.len() => {
                source = Some(args[i + 1].to_lowercase());
                i += 2;
            }
            "--canonical" | "-c" if i + 1 < args.len() => {
                if symbol.is_empty() {
                    symbol = args[i + 1].clone();
                }
                i += 2;
            }
            "--json" => {
                json_mode = true;
                i += 1;
            }
            _ => {
                if symbol.is_empty() && !args[i].starts_with('-') {
                    symbol = args[i].clone();
                }
                i += 1;
            }
        }
    }

    if symbol.is_empty() {
        eprintln!("Error: Missing required symbol. Example: economy-aggregator unsubscribe --source yahoo --symbol BBRI.JK");
        std::process::exit(1);
    }

    let config = LakehouseConfig::default();
    let sub_manager = SubscriptionManager::new(SubscriptionManager::default_path(&config.base_dir));
    let removed = sub_manager.unsubscribe(source.as_deref(), &symbol)?;

    if json_mode {
        let envelope = serde_json::json!({
            "success": removed,
            "source": source,
            "symbol": symbol,
            "message": if removed {
                if let Some(src) = &source {
                    format!("Subscription for symbol '{}' (source: {}) removed successfully", symbol, src)
                } else {
                    format!("All subscriptions for symbol '{}' removed successfully", symbol)
                }
            } else {
                "Subscription not found".to_string()
            }
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else if removed {
        if let Some(src) = &source {
            println!("🗑️  Successfully unsubscribed from symbol '{}' (source: {})", symbol, src);
        } else {
            println!("🗑️  Successfully unsubscribed from symbol '{}' across all sources", symbol);
        }
    } else {
        println!("⚠️  Subscription for symbol '{}' (source: {:?}) not found", symbol, source);
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
        println!("{:-<80}", "");
        println!("{:<10} {:<16} {:<12} {:<14} {:<10}", "SOURCE", "SYMBOL", "ASSET", "TIMEFRAMES", "STATUS");
        println!("{:-<80}", "");
        for it in &items {
            let status = if it.enabled { "ENABLED" } else { "DISABLED" };
            let tf_str = it.timeframes.join(",");
            println!("{:<10} {:<16} {:<12} {:<14} {:<10}", it.source, it.symbol, it.asset_class, tf_str, status);
        }
        println!("{:-<80}", "");
    }

    Ok(())
}
