//! Agent-First CLI Subcommands for Canonical Liquidity & Flow Observations (Phase 323).

use chrono::{DateTime, Utc};
use crate::core::cli_remote::{RemoteCliError, RemoteClient};
use crate::core::config::LakehouseConfig;
use crate::core::series_catalog::SeriesCatalog;
use crate::features::ingestion::liquidity_sync::sync_observations;
use crate::features::storage::{ObservationFilter, ObservationReader, PartitionResolver};

fn arg_val(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}

pub async fn handle_observations(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let action = args.first().map(|s| s.as_str()).unwrap_or("catalog");
    let config = LakehouseConfig::default();
    config.ensure_directories()?;
    let reader = ObservationReader::new(PartitionResolver::new(config.lake_dir.clone()));

    match action {
        "catalog" => {
            let catalog = SeriesCatalog::new_canonical();
            let coverage = reader.summarize_coverage();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "success": true,
                    "defined_series": catalog.list(),
                    "ingested_coverage": coverage
                }))?
            );
        }
        "sync" => {
            let target = arg_val(&args[1..], "--target").unwrap_or_else(|| "all".to_string());
            let report = sync_observations(&config, &target).await;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "success": true,
                    "report": report
                }))?
            );
        }
        "query" => {
            let series_id = arg_val(&args[1..], "--series");
            let compartment = arg_val(&args[1..], "--compartment");
            let limit = arg_val(&args[1..], "--limit").and_then(|s| s.parse::<usize>().ok());
            let as_of = arg_val(&args[1..], "--as-of")
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc)));
            let rows = reader.read_observations(&ObservationFilter {
                series_ids: series_id.map(|s| vec![s]).unwrap_or_default(),
                compartment,
                as_of_known_at: as_of,
                resolve_latest_revision: true,
                limit,
                ..Default::default()
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "success": true,
                    "count": rows.len(),
                    "data": rows
                }))?
            );
        }
        other => {
            eprintln!(
                "{{\"success\":false,\"error\":\"Unknown observations action '{}'. Use catalog|sync|query.\"}}",
                other
            );
            std::process::exit(1);
        }
    }
    Ok(())
}

pub async fn handle_observations_remote(
    args: &[String],
    client: &RemoteClient,
) -> Result<serde_json::Value, RemoteCliError> {
    let action = args.first().map(|s| s.as_str()).unwrap_or("catalog");
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };
    let data = match action {
        "catalog" => client.get_json("/v1/observations/catalog", &[]).await?,
        "sync" => {
            let mut q = Vec::new();
            if let Some(t) = arg_val(rest, "--target") {
                q.push(("target", t));
            }
            let path = if q.is_empty() {
                "/v1/observations/sync".to_string()
            } else {
                format!("/v1/observations/sync?target={}", q[0].1)
            };
            client.post_json(&path, None).await?
        }
        "query" => {
            let mut q = Vec::new();
            if let Some(v) = arg_val(rest, "--series") { q.push(("series_id", v)); }
            if let Some(v) = arg_val(rest, "--compartment") { q.push(("compartment", v)); }
            if let Some(v) = arg_val(rest, "--as-of") { q.push(("as_of_known_at", v)); }
            if let Some(v) = arg_val(rest, "--limit") { q.push(("limit", v)); }
            client.get_json("/v1/observations", &q).await?
        }
        "matrix" => {
            let mut q = Vec::new();
            if let Some(v) = arg_val(rest, "--series") { q.push(("series_ids", v)); }
            if let Some(v) = arg_val(rest, "--compartment") { q.push(("compartment", v)); }
            if let Some(v) = arg_val(rest, "--resample") { q.push(("resample", v)); }
            if let Some(v) = arg_val(rest, "--as-of") { q.push(("as_of_known_at", v)); }
            client.get_json("/v1/observations/matrix", &q).await?
        }
        other => {
            return Err(RemoteCliError::config_or_auth(
                format!("Unknown observations action '{}'. Use catalog|query|matrix|sync.", other),
                None,
            ));
        }
    };

    Ok(serde_json::json!({
        "success": true,
        "error": null,
        "mode": "remote",
        "base_url": client.base_url(),
        "data": data,
        "next_steps": [
            "economy-aggregator observations matrix --resample 1w",
            "economy-aggregator observations catalog"
        ]
    }))
}
