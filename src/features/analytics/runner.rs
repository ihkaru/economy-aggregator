//! Analytical CLI Runner for multi-decade portfolio simulations.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use arrow::array::{Float64Array, TimestampMicrosecondArray, TimestampNanosecondArray};
use arrow::datatypes::{DataType, TimeUnit};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use super::{
    DayCountConvention, FxNormalizer, PortfolioEngine,
    SyntheticBondAccumulator, TradingCalendarAligner,
};
use crate::core::config::LakehouseConfig;

pub async fn handle_analyze(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut portfolio_name = "all-weather".to_string();
    let mut min_year = 2004i32;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--portfolio" if i + 1 < args.len() => { portfolio_name = args[i + 1].clone(); i += 2; }
            "--start-year" if i + 1 < args.len() => {
                if let Ok(y) = args[i + 1].parse::<i32>() { min_year = y; }
                i += 2;
            }
            _ => { i += 1; }
        }
    }

    let config = LakehouseConfig::default();
    let lake_dir = &config.lake_dir;

    // Load available core market series from lake (filtered by min_year for high speed)
    let bbca_pts = read_lake_symbol_series(lake_dir, "equities", "BBCA", min_year)?;
    let gspc_pts = read_lake_symbol_series(lake_dir, "equities", "GSPC", min_year)?;
    let gold_pts = read_lake_symbol_series(lake_dir, "commodities", "XAU_USD", min_year)?;
    let usdidr_pts = read_lake_symbol_series(lake_dir, "forex", "USD_IDR", min_year)?;
    let jkse_pts = read_lake_symbol_series(lake_dir, "equities", "JKSE", min_year).unwrap_or_default();

    if bbca_pts.is_empty() || gspc_pts.is_empty() || gold_pts.is_empty() || usdidr_pts.is_empty() {
        let err = serde_json::json!({
            "success": false,
            "error": "Required lakehouse data missing. Run 'economy-aggregator backfill-all-weather --range max' first.",
            "next_steps": ["cargo run -- backfill-all-weather --range max", "cargo run -- compact"]
        });
        println!("{}", serde_json::to_string_pretty(&err)?);
        return Ok(());
    }

    // Determine common start/end
    let start_date = NaiveDate::from_ymd_opt(min_year, 1, 1).unwrap();
    let end_date = Utc::now().date_naive();

    // 1. Synthetic Fixed Income Assets
    let sbn_tri = SyntheticBondAccumulator::generate_constant_yield_tri(
        start_date, end_date, 0.072, 100.0, DayCountConvention::Actual365,
    );
    let corp_tri = SyntheticBondAccumulator::generate_constant_yield_tri(
        start_date, end_date, 0.085, 100.0, DayCountConvention::Actual365,
    );
    let usd_rdpt_raw = SyntheticBondAccumulator::generate_constant_yield_tri(
        start_date, end_date, 0.060, 100.0, DayCountConvention::Actual365,
    );
    let usd_rdpu_raw = SyntheticBondAccumulator::generate_constant_yield_tri(
        start_date, end_date, 0.045, 100.0, DayCountConvention::Actual365,
    );

    // 2. FX Normalization (USD assets into IDR)
    let gspc_idr = FxNormalizer::normalize_by_multiplication(&gspc_pts, &usdidr_pts);
    let gold_idr = FxNormalizer::normalize_by_multiplication(&gold_pts, &usdidr_pts);
    let usd_rdpt_idr = FxNormalizer::normalize_by_multiplication(&usd_rdpt_raw, &usdidr_pts);
    let usd_rdpu_idr = FxNormalizer::normalize_by_multiplication(&usd_rdpu_raw, &usdidr_pts);

    // Filter by min_year
    let filter_date = |v: Vec<(NaiveDate, f64)>| -> Vec<(NaiveDate, f64)> {
        v.into_iter().filter(|(d, _)| d.year() >= min_year).collect()
    };

    let mut series_map = HashMap::new();
    series_map.insert("SBN_10Y".to_string(), filter_date(sbn_tri));
    series_map.insert("CORP_BOND".to_string(), filter_date(corp_tri));
    series_map.insert("BBCA_IDR".to_string(), filter_date(bbca_pts));
    series_map.insert("SP500_IDR".to_string(), filter_date(gspc_idr));
    series_map.insert("GOLD_IDR".to_string(), filter_date(gold_idr));
    series_map.insert("USD_RDPT_IDR".to_string(), filter_date(usd_rdpt_idr));
    series_map.insert("USD_RDPU_IDR".to_string(), filter_date(usd_rdpu_idr));

    if !jkse_pts.is_empty() {
        series_map.insert("IHSG_BENCHMARK".to_string(), filter_date(jkse_pts));
    }

    // 3. Calendar Alignment
    let panel = TradingCalendarAligner::align(&series_map, true)
        .map_err(|e| format!("Alignment error: {}", e))?;

    // 4. All-Weather Weights (Sander's Portfolio)
    let mut weights = HashMap::new();
    weights.insert("SBN_10Y".to_string(), 0.25);
    weights.insert("CORP_BOND".to_string(), 0.25);
    weights.insert("BBCA_IDR".to_string(), 0.125);
    weights.insert("SP500_IDR".to_string(), 0.125);
    weights.insert("GOLD_IDR".to_string(), 0.10);
    weights.insert("USD_RDPT_IDR".to_string(), 0.075);
    weights.insert("USD_RDPU_IDR".to_string(), 0.075);

    // 5. Portfolio Evaluation
    let metrics = PortfolioEngine::evaluate(&panel, &weights, 0.0)
        .map_err(|e| format!("Portfolio evaluation error: {}", e))?;

    let json = serde_json::json!({
        "success": true,
        "portfolio": portfolio_name,
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
                                        // Reject dirty quotes (e.g. historical USD/IDR glitch quotes < 5000)
                                        if clean_symbol == "USD_IDR" && price < 5000.0 {
                                            continue;
                                        }
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
