//! CLI handler and terminal reporter for lakehouse candle reconciliation.

use std::fs;
use chrono::{Duration, Utc};
use serde_json::json;

use crate::core::config::LakehouseConfig;
use crate::core::types::TimeFrame;
use crate::features::analytics::reconciliation::{ReconciliationEngine, ReconciliationReport};
use crate::features::ingestion::{BinanceRestFetcher, DukascopyParser, TradingViewParser, YahooFinanceFetcher};
use crate::features::storage::LakeReader;

pub async fn handle_reconcile(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut symbol = "GSPC".to_string();
    let mut raw_symbol = String::new();
    let mut asset_class: Option<String> = None;
    let mut source = "yahoo".to_string();
    let mut payload_file = String::new();
    let mut tolerance_pct = 0.5f64;
    let mut days = 30i64;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--symbol" if i + 1 < args.len() => {
                symbol = args[i + 1].clone();
                i += 2;
            }
            "--raw" if i + 1 < args.len() => {
                raw_symbol = args[i + 1].clone();
                i += 2;
            }
            "--asset" if i + 1 < args.len() => {
                asset_class = Some(args[i + 1].clone());
                i += 2;
            }
            "--source" if i + 1 < args.len() => {
                source = args[i + 1].to_lowercase();
                i += 2;
            }
            "--payload" if i + 1 < args.len() => {
                payload_file = args[i + 1].clone();
                i += 2;
            }
            "--tolerance" if i + 1 < args.len() => {
                if let Ok(val) = args[i + 1].parse::<f64>() {
                    tolerance_pct = val;
                }
                i += 2;
            }
            "--days" if i + 1 < args.len() => {
                if let Ok(val) = args[i + 1].parse::<i64>() {
                    days = val;
                }
                i += 2;
            }
            "--json" => {
                json_mode = true;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    if raw_symbol.is_empty() {
        raw_symbol = match symbol.as_str() {
            "GSPC" => "^GSPC".to_string(),
            "JKSE" => "^JKSE".to_string(),
            "N225" => "^N225".to_string(),
            "BBCA" => "BBCA.JK".to_string(),
            "BTC_USDT" | "BTC/USDT" => "BTCUSDT".to_string(),
            "XAU_USD" | "XAU/USD" => "GC=F".to_string(),
            "USD_IDR" | "USD/IDR" => "IDR=X".to_string(),
            other => other.to_string(),
        };
    }

    let config = LakehouseConfig::default();
    let now = Utc::now();
    let start_dt = now - Duration::days(days);
    let start_date = Some(start_dt.date_naive());

    // 1. Read lakehouse candles
    let lake_candles = LakeReader::read_candles(
        &config.lake_dir,
        asset_class.as_deref(),
        &symbol,
        "1d",
        start_date,
        None,
    )?;

    if lake_candles.is_empty() {
        let err_msg = format!("No lakehouse candles found for symbol '{}' in {}", symbol, config.lake_dir.display());
        if json_mode {
            println!("{}", json!({ "success": false, "error": err_msg }));
        } else {
            eprintln!("❌ {}", err_msg);
        }
        return Ok(());
    }

    // 2. Fetch or parse live candles
    let live_candles = match source.as_str() {
        "yahoo" => {
            let fetcher = YahooFinanceFetcher::default();
            let range = if days <= 35 {
                "1mo"
            } else if days <= 100 {
                "3mo"
            } else if days <= 200 {
                "6mo"
            } else if days <= 370 {
                "1y"
            } else {
                "2y"
            };
            fetcher.fetch_daily_bars(&raw_symbol, &symbol, range).await?
        }
        "binance" => {
            let fetcher = BinanceRestFetcher::default();
            let limit = days.min(1000) as u32;
            fetcher.fetch_klines(&raw_symbol, &symbol, "1d", None, None, Some(limit)).await?
        }
        "dukascopy" => {
            if payload_file.is_empty() {
                return Err("Dukascopy requires --payload <file.json> containing bar data".into());
            }
            let content = fs::read_to_string(&payload_file)?;
            DukascopyParser::parse_json(&content, &symbol, TimeFrame::D1)?
        }
        "tradingview" => {
            if payload_file.is_empty() {
                return Err("TradingView requires --payload <file.json> containing UDF response".into());
            }
            let content = fs::read_to_string(&payload_file)?;
            TradingViewParser::parse_udf_json(&content, &symbol, TimeFrame::D1)?
        }
        other => return Err(format!("Unsupported reconciliation source '{}'", other).into()),
    };

    // 3. Reconcile
    let report = ReconciliationEngine::compare_candles(&symbol, &lake_candles, &live_candles, tolerance_pct);

    // 4. Output
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_terminal_report(&report);
    }

    Ok(())
}

fn print_terminal_report(r: &ReconciliationReport) {
    println!("================================================================================");
    println!("🔍 LAKEHOUSE CANDLE RECONCILIATION AUDIT: {}", r.canonical_symbol);
    println!("================================================================================");
    println!("Lake Bars:        {} (source: {})", r.total_lake_bars, r.lake_source);
    println!("Live Bars:        {} (source: {})", r.total_live_bars, r.live_source);
    println!("Common Matched:   {}", r.matched_bars);
    println!("Exact Matches:    {} ({:.1}%)", r.exact_matches, if r.matched_bars > 0 { (r.exact_matches as f64 / r.matched_bars as f64) * 100.0 } else { 0.0 });
    println!("Tolerance:        ±{:.3}%", r.tolerance_pct);
    println!("Price Drifts:     {} bars", r.drifted_bars_count);
    println!("Missing in Lake:  {} bars", r.missing_in_lake_count);
    println!("Missing in Live:  {} bars", r.missing_in_live_count);
    println!("Max Close Drift:  {:.4}%", r.max_close_drift_pct);
    println!("Mean Close Drift: {:.4}%", r.mean_close_drift_pct);
    println!("--------------------------------------------------------------------------------");
    if r.is_valid {
        println!("Status:           ✅ PASSED (Lakehouse data is 100% valid & aligned with {})", r.live_source);
    } else {
        println!("Status:           ⚠️ FAILED (Discrepancies detected exceeding tolerance)");
    }
    println!("================================================================================");

    if !r.discrepancies.is_empty() {
        println!("\nTop Discrepancies (first 10):");
        println!("{:<12} {:<12} {:<12} {:<12} {:<15} {}", "Date", "Lake Close", "Live Close", "Drift %", "Type", "Details");
        println!("{}", "-".repeat(80));
        for d in r.discrepancies.iter().take(10) {
            println!(
                "{:<12} {:<12.4} {:<12.4} {:<12.3}% {:<15?} {}",
                d.date, d.lake_close, d.live_close, d.drift_pct, d.discrepancy_type, d.details
            );
        }
        if r.discrepancies.len() > 10 {
            println!("... and {} more discrepancies omitted.", r.discrepancies.len() - 10);
        }
    }
}
