//! Analytical CLI Runner for multi-decade empirical portfolio simulations.
//!
//! Enforces Zero-Synthetic Data Invariant & Statistical Hampel Outlier Cleaning.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use arrow::array::{Float64Array, TimestampMicrosecondArray, TimestampNanosecondArray};
use arrow::datatypes::{DataType, TimeUnit};
use chrono::{DateTime, Datelike, NaiveDate};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use super::{
    FxNormalizer, HampelFilter, PortfolioEngine, PortfolioSpec, TradingCalendarAligner,
};
use crate::core::config::LakehouseConfig;

pub async fn handle_analyze(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut min_year = 2004i32;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--start-year" if i + 1 < args.len() => {
                if let Ok(y) = args[i + 1].parse::<i32>() { min_year = y; }
                i += 2;
            }
            _ => { i += 1; }
        }
    }

    let config = LakehouseConfig::default();
    let lake_dir = &config.lake_dir;

    // Load Declarative Portfolio Specification (100% Real Empirical Market Instruments)
    let spec = PortfolioSpec::all_weather_empirical();

    // 1. Load and statistically clean FX Exchange Rate (USD/IDR)
    let usdidr_raw = read_lake_symbol_series(lake_dir, "forex", "USD_IDR", min_year)?;
    let usdidr_pts = HampelFilter::filter_outliers(&usdidr_raw, 7, 3.5);

    if usdidr_pts.is_empty() {
        let err = serde_json::json!({
            "success": false,
            "error": "Required FX time-series (USD/IDR) missing in lakehouse. Run 'cargo run -- pull --symbol USDIDR=X --canonical USD/IDR --asset forex --range max' first.",
            "next_steps": ["cargo run -- pull --symbol USDIDR=X --canonical USD/IDR --asset forex --range max"]
        });
        println!("{}", serde_json::to_string_pretty(&err)?);
        return Ok(());
    }

    // 2. Load all constituent instruments and apply statistical cleaning + FX normalization
    let mut series_map = HashMap::new();
    let mut weights = HashMap::new();

    for asset in &spec.assets {
        let raw_pts = read_lake_symbol_series(lake_dir, &asset.asset_class, &asset.symbol, min_year)?;
        if raw_pts.is_empty() {
            let err = serde_json::json!({
                "success": false,
                "error": format!("Empirical instrument '{}' missing in lakehouse. Zero synthetic data permitted.", asset.symbol),
                "next_steps": [format!("cargo run -- pull --symbol {} --canonical {} --asset {} --range max", asset.symbol, asset.symbol, asset.asset_class)]
            });
            println!("{}", serde_json::to_string_pretty(&err)?);
            return Ok(());
        }

        // Apply non-parametric Hampel filter (zero hardcoded thresholds)
        let cleaned_pts = HampelFilter::filter_outliers(&raw_pts, 7, 3.5);

        // FX Currency Normalization if quoted in foreign currency
        let base_pts = if spec.base_currency == "IDR" && asset.currency == "USD" {
            FxNormalizer::normalize_by_multiplication(&cleaned_pts, &usdidr_pts)
        } else {
            cleaned_pts
        };

        // Filter by min_year
        let final_pts: Vec<(NaiveDate, f64)> = base_pts
            .into_iter()
            .filter(|(d, _)| d.year() >= min_year)
            .collect();

        let target_name = format!("{}_{}", asset.canonical_name, spec.base_currency);
        weights.insert(target_name.clone(), asset.weight);
        series_map.insert(target_name, final_pts);
    }

    // 3. Load Benchmark if specified
    if let Some(ref bench_symbol) = spec.benchmark {
        let bench_raw = read_lake_symbol_series(lake_dir, "equities", bench_symbol, min_year)?;
        if !bench_raw.is_empty() {
            let bench_clean = HampelFilter::filter_outliers(&bench_raw, 7, 3.5);
            let bench_final: Vec<(NaiveDate, f64)> = bench_clean
                .into_iter()
                .filter(|(d, _)| d.year() >= min_year)
                .collect();
            series_map.insert(format!("{}_BENCHMARK", bench_symbol), bench_final);
        }
    }

    // 4. Synchronize Multi-Exchange Calendars
    let panel = TradingCalendarAligner::align(&series_map, true)
        .map_err(|e| format!("Alignment error: {}", e))?;

    // 5. Evaluate Empirical Portfolio
    let metrics = PortfolioEngine::evaluate(&panel, &weights, 0.0)
        .map_err(|e| format!("Portfolio evaluation error: {}", e))?;

    let json = serde_json::json!({
        "success": true,
        "portfolio": spec.name,
        "base_currency": spec.base_currency,
        "data_fidelity": "100% Empirical Market Instruments (Zero Synthetic Data)",
        "outlier_cleaning": "Hampel Filter (Rolling MAD window=15, 3.5 sigmas)",
        "metrics": metrics,
        "next_steps": [
            "economy-aggregator status",
            "economy-aggregator compact"
        ]
    });

    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

fn read_lake_symbol_series(
    lake_dir: &Path,
    asset_class: &str,
    symbol: &str,
    min_year: i32,
) -> Result<Vec<(NaiveDate, f64)>, Box<dyn std::error::Error + Send + Sync>> {
    let clean_symbol = symbol.to_uppercase().replace('/', "_").replace(':', "_");
    let sym_dir = lake_dir
        .join("market")
        .join(format!("asset_class={}", asset_class))
        .join(format!("symbol={}", clean_symbol))
        .join("timeframe=1d");

    if !sym_dir.exists() {
        return Ok(Vec::new());
    }

    let mut parquet_files = Vec::new();
    find_parquet_since_year(&sym_dir, min_year, &mut parquet_files);

    let mut points: Vec<(NaiveDate, f64)> = Vec::new();

    for path in parquet_files {
        if let Ok(file) = File::open(&path) {
            if let Ok(builder) = ParquetRecordBatchReaderBuilder::try_new(file) {
                if let Ok(mut reader) = builder.build() {
                    while let Some(Ok(batch)) = reader.next() {
                        let ts_col = batch.column(0);
                        let close_col = batch.column(6);
                        let close_arr = close_col.as_any().downcast_ref::<Float64Array>();

                        if let Some(closes) = close_arr {
                            for i in 0..batch.num_rows() {
                                let date_opt = match ts_col.data_type() {
                                    DataType::Timestamp(TimeUnit::Nanosecond, _) => {
                                        ts_col.as_any().downcast_ref::<TimestampNanosecondArray>()
                                            .map(|a| DateTime::from_timestamp_nanos(a.value(i)).date_naive())
                                    }
                                    DataType::Timestamp(TimeUnit::Microsecond, _) => {
                                        ts_col.as_any().downcast_ref::<TimestampMicrosecondArray>()
                                            .and_then(|a| DateTime::from_timestamp_micros(a.value(i)).map(|dt| dt.date_naive()))
                                    }
                                    _ => None,
                                };

                                if let Some(d) = date_opt {
                                    let price = closes.value(i);
                                    if price > 0.0 {
                                        points.push((d, price));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    points.sort_by_key(|p| p.0);
    points.dedup_by_key(|p| p.0);

    Ok(points)
}

fn find_parquet_since_year(dir: &Path, min_year: i32, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("year=") {
                        if let Ok(y) = name.trim_start_matches("year=").parse::<i32>() {
                            if y < min_year {
                                continue;
                            }
                        }
                    }
                }
                find_parquet_since_year(&p, min_year, files);
            } else if p.extension().map_or(false, |ext| ext == "parquet") {
                files.push(p);
            }
        }
    }
}
