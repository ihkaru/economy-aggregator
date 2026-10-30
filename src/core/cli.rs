//! Agent-ready CLI engine and command dispatcher.

use tracing::info;

use crate::core::config::LakehouseConfig;
use crate::core::types::AssetClass;
use crate::features::ingestion::{BinanceRestFetcher, EconomicCalendarFetcher, YahooFinanceFetcher};
use crate::features::storage::{LakeWriter, PartitionResolver};

pub async fn run_cli(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if args.is_empty() || args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "pull" => handle_pull(&args[1..]).await,
        "backfill-all-weather" => handle_backfill_all_weather(&args[1..]).await,
        "compact" => handle_compact(&args[1..]).await,
        "calendar" => handle_calendar(&args[1..]).await,
        "analyze" => crate::features::analytics::handle_analyze(&args[1..]).await,
        "status" => handle_status().await,
        other => {
            eprintln!("{{\"success\":false,\"error\":\"Unknown subcommand '{}'. Use 'help' for usage.\"}}", other);
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(r#"{{
  "name": "economy-aggregator",
  "version": "0.1.0",
  "subcommands": [
    "serve                      Start REST HTTP serving layer",
    "pull                       Pull candles from external source into Parquet lake",
    "backfill-all-weather       Backfill 7 core All-Weather assets & benchmarks into Parquet lake",
    "compact                    Compact micro-chunks into unified monthly Parquet files",
    "calendar                   Fetch live macroeconomic calendar releases",
    "analyze                    Run vectorized multi-asset portfolio and risk parity analytics",
    "status                     Inspect lakehouse partitions, files, and disk footprint",
    "help                       Show this help message"
  ],
  "pull_flags": [
    "--symbol <raw>             Raw ticker (e.g. '^GSPC', 'BBCA.JK', 'BTCUSDT', 'GC=F')",
    "--canonical <symbol>       Canonical symbol (e.g. 'GSPC', 'BBCA', 'BTC/USDT', 'XAU/USD')",
    "--asset <class>            Asset class: 'equities', 'crypto', 'commodities', 'forex'",
    "--range <period>           Yahoo range: '1mo', '1y', '5y', 'max' (default: '1y')",
    "--source <name>            Source: 'yahoo' (default) or 'binance'"
  ]
}}"#);
}

async fn handle_pull(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut raw_symbol = String::new();
    let mut canonical_symbol = String::new();
    let mut asset_str = "equities".to_string();
    let mut range = "1y".to_string();
    let mut source = "yahoo".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--symbol" if i + 1 < args.len() => { raw_symbol = args[i + 1].clone(); i += 2; }
            "--canonical" if i + 1 < args.len() => { canonical_symbol = args[i + 1].clone(); i += 2; }
            "--asset" if i + 1 < args.len() => { asset_str = args[i + 1].clone(); i += 2; }
            "--range" if i + 1 < args.len() => { range = args[i + 1].clone(); i += 2; }
            "--source" if i + 1 < args.len() => { source = args[i + 1].clone(); i += 2; }
            _ => { i += 1; }
        }
    }

    if raw_symbol.is_empty() {
        eprintln!("{{\"success\":false,\"error\":\"Missing required --symbol flag\"}}");
        std::process::exit(1);
    }
    if canonical_symbol.is_empty() {
        canonical_symbol = raw_symbol.trim_start_matches('^').to_string();
    }

    let asset_class = parse_asset_class(&asset_str);
    let config = LakehouseConfig::default();
    config.ensure_directories()?;
    let resolver = PartitionResolver::new(config.lake_dir.clone());
    let writer = LakeWriter::new(resolver);

    let candles = if source == "binance" {
        let fetcher = BinanceRestFetcher::default();
        fetcher.fetch_klines(&raw_symbol, &canonical_symbol, "1d", None, None, Some(1000)).await?
    } else {
        let fetcher = YahooFinanceFetcher::default();
        fetcher.fetch_daily_bars(&raw_symbol, &canonical_symbol, &range).await?
    };

    let files = writer.write_candles(&candles, asset_class)?;
    let written_count = candles.len();
    let file_paths: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();

    let json = serde_json::json!({
        "success": true,
        "symbol": raw_symbol,
        "canonical": canonical_symbol,
        "source": source,
        "candles_count": written_count,
        "files_written": file_paths,
    });
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

async fn handle_backfill_all_weather(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut range = "5y".to_string();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--range" && i + 1 < args.len() {
            range = args[i + 1].clone();
            i += 2;
        } else {
            i += 1;
        }
    }

    info!("🌐 Starting All-Weather Portfolio backfill (range: {})...", range);
    let targets = vec![
        ("^GSPC", "GSPC", AssetClass::Equities, "yahoo"),
        ("^JKSE", "JKSE", AssetClass::Equities, "yahoo"),
        ("^N225", "N225", AssetClass::Equities, "yahoo"),
        ("BBCA.JK", "BBCA", AssetClass::Equities, "yahoo"),
        ("GC=F", "XAU/USD", AssetClass::Commodities, "yahoo"),
        ("USDIDR=X", "USD/IDR", AssetClass::Forex, "yahoo"),
        ("BTCUSDT", "BTC/USDT", AssetClass::Crypto, "binance"),
    ];

    let config = LakehouseConfig::default();
    config.ensure_directories()?;
    let resolver = PartitionResolver::new(config.lake_dir.clone());
    let writer = LakeWriter::new(resolver);

    let yahoo = YahooFinanceFetcher::default();
    let binance = BinanceRestFetcher::default();

    let mut results = Vec::new();
    for (raw, canonical, asset_class, src) in targets {
        let res = if src == "binance" && range != "max" {
            match binance.fetch_klines(raw, canonical, "1d", None, None, Some(1000)).await {
                Ok(c) => Ok(c),
                Err(_) => yahoo.fetch_daily_bars("BTC-USD", canonical, &range).await,
            }
        } else if src == "binance" {
            yahoo.fetch_daily_bars("BTC-USD", canonical, &range).await
        } else {
            yahoo.fetch_daily_bars(raw, canonical, &range).await
        };

        match res {
            Ok(candles) if !candles.is_empty() => {
                let count = candles.len();
                let files = writer.write_candles(&candles, asset_class)?;
                results.push(serde_json::json!({
                    "symbol": raw,
                    "canonical": canonical,
                    "asset_class": asset_class.as_str(),
                    "source": src,
                    "candles": count,
                    "files": files.len(),
                    "status": "success"
                }));
            }
            Ok(_) => {
                results.push(serde_json::json!({
                    "symbol": raw,
                    "canonical": canonical,
                    "status": "empty"
                }));
            }
            Err(e) => {
                results.push(serde_json::json!({
                    "symbol": raw,
                    "canonical": canonical,
                    "status": "error",
                    "error": e.to_string()
                }));
            }
        }
    }

    let json = serde_json::json!({
        "success": true,
        "action": "backfill-all-weather",
        "range": range,
        "summary": results
    });
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

async fn handle_compact(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut min_files = 1usize;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--min-files" && i + 1 < args.len() {
            if let Ok(v) = args[i + 1].parse() {
                min_files = v;
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    let config = LakehouseConfig::default();
    let compactor = crate::features::storage::LakeCompactor::default();
    let results = compactor.compact_all(&config.lake_dir, min_files)?;

    let total_compacted: usize = results.iter().map(|r| r.files_compacted).sum();
    let rows_before: usize = results.iter().map(|r| r.total_rows_before).sum();
    let rows_after: usize = results.iter().map(|r| r.total_rows_after).sum();

    let json = serde_json::json!({
        "success": true,
        "action": "compact",
        "partitions_compacted": results.len(),
        "files_merged": total_compacted,
        "rows_before": rows_before,
        "rows_after": rows_after,
        "deduplicated_rows": rows_before.saturating_sub(rows_after),
    });
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

async fn handle_calendar(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut country: Option<String> = None;
    let mut importance: Option<String> = Some("high".to_string());

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--country" if i + 1 < args.len() => { country = Some(args[i + 1].clone()); i += 2; }
            "--impact" if i + 1 < args.len() => { importance = Some(args[i + 1].clone()); i += 2; }
            _ => { i += 1; }
        }
    }

    let fetcher = EconomicCalendarFetcher::default();
    let events = fetcher.fetch_events(country.as_deref(), importance.as_deref()).await?;
    let json = serde_json::json!({
        "success": true,
        "total_events": events.len(),
        "events": events
    });
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

async fn handle_status() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = LakehouseConfig::default();
    let lake_dir = &config.lake_dir;

    let mut total_parquet_files = 0usize;
    let mut total_bytes = 0u64;

    if lake_dir.exists() {
        scan_dir(lake_dir, &mut total_parquet_files, &mut total_bytes);
    }

    let meta_file = lake_dir.join("meta").join("instruments.json");
    let instruments_count = if meta_file.exists() {
        std::fs::read_to_string(&meta_file)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0)
    } else {
        0
    };

    let json = serde_json::json!({
        "success": true,
        "lake_dir": lake_dir.display().to_string(),
        "total_parquet_files": total_parquet_files,
        "total_size_mb": (total_bytes as f64) / (1024.0 * 1024.0),
        "total_bytes": total_bytes,
        "registered_instruments": instruments_count
    });
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

fn scan_dir(dir: &std::path::Path, count: &mut usize, bytes: &mut u64) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_dir(&path, count, bytes);
            } else if path.extension().and_then(|e| e.to_str()) == Some("parquet") {
                *count += 1;
                if let Ok(meta) = entry.metadata() {
                    *bytes += meta.len();
                }
            }
        }
    }
}

fn parse_asset_class(s: &str) -> AssetClass {
    match s.to_lowercase().as_str() {
        "crypto" => AssetClass::Crypto,
        "commodities" | "commodity" => AssetClass::Commodities,
        "forex" | "fx" => AssetClass::Forex,
        "macro" => AssetClass::Macro,
        _ => AssetClass::Equities,
    }
}
