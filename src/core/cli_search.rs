//! CLI handler for multi-source symbol search and discovery.

use crate::features::ingestion::search::{SearchResultItem, UnifiedSearchEngine};

pub async fn handle_search(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut query = String::new();
    let mut source = "all".to_string();
    let mut limit = 5usize;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--query" | "-q" if i + 1 < args.len() => {
                query = args[i + 1].clone();
                i += 2;
            }
            "--source" | "-s" if i + 1 < args.len() => {
                source = args[i + 1].to_lowercase();
                i += 2;
            }
            "--limit" | "-l" if i + 1 < args.len() => {
                if let Ok(l) = args[i + 1].parse::<usize>() {
                    limit = l;
                }
                i += 2;
            }
            "--json" => {
                json_mode = true;
                i += 1;
            }
            _ => {
                if query.is_empty() && !args[i].starts_with('-') {
                    query = args[i].clone();
                }
                i += 1;
            }
        }
    }

    if query.trim().is_empty() {
        if json_mode {
            println!("{{\"success\":false,\"error\":\"Missing required --query parameter\"}}");
        } else {
            eprintln!("Error: Missing required --query parameter. Example: economy-aggregator search --query BBRI");
        }
        std::process::exit(1);
    }

    let engine = UnifiedSearchEngine::default();
    let results: Vec<SearchResultItem> = match source.as_str() {
        "yahoo" | "equities" => engine.search_yahoo(&query, limit).await,
        "binance" | "crypto" => engine.search_binance(&query, limit).await,
        "polymarket" => engine.search_polymarket(&query, limit).await,
        "kalshi" => engine.search_kalshi(&query, limit).await,
        _ => engine.search_all(&query, limit).await,
    };

    if json_mode {
        let envelope = serde_json::json!({
            "success": true,
            "query": query,
            "source": source,
            "count": results.len(),
            "results": results,
            "next_steps": [
                "To subscribe to an instrument: economy-aggregator subscribe --source <source> --symbol <symbol> --asset <asset_class>",
                "To inspect lake data: economy-aggregator status"
            ]
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("🔍 Search Results for '{}' (Source: {}, Found: {}):", query, source, results.len());
        println!("{:-<85}", "");
        println!(
            "{:<10} {:<16} {:<12} {:<12} {:<30}",
            "SOURCE", "SYMBOL", "ASSET", "EXCHANGE", "NAME"
        );
        println!("{:-<85}", "");
        for r in &results {
            let ex = r.exchange.as_deref().unwrap_or("-");
            let truncated_name = if r.name.len() > 30 {
                format!("{}...", &r.name[..27])
            } else {
                r.name.clone()
            };
            println!(
                "{:<10} {:<16} {:<12} {:<12} {:<30}",
                r.source, r.raw_symbol, r.asset_class, ex, truncated_name
            );
        }
        println!("{:-<85}", "");
    }

    Ok(())
}
