//! Remote CLI Handlers for Diagnostic Trees, Regime Transitions, Continuous Analytics,
//! and Prospective OOS Scorecards (Phases 349 & 356).

use serde_json::Value;

use crate::core::cli_remote::{flag_val, wrap_ok, RemoteCliError, RemoteClient};

pub async fn handle_remote_spillover(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut q = Vec::new();
    if let Some(v) = flag_val(args, &["--start", "--start-date"]) { q.push(("start_date", v)); }
    if let Some(v) = flag_val(args, &["--end", "--end-date"]) { q.push(("end_date", v)); }
    if let Some(v) = flag_val(args, &["--lags"]) { q.push(("lags", v)); }
    if let Some(v) = flag_val(args, &["--horizon"]) { q.push(("horizon", v)); }
    if let Some(v) = flag_val(args, &["--liquidity-mode"]) { q.push(("liquidity_mode", v)); }
    if let Some(v) = flag_val(args, &["--null-mode"]) { q.push(("null_mode", v)); }
    if let Some(v) = flag_val(args, &["--permutations"]) { q.push(("permutations", v)); }
    if args.iter().any(|a| a == "--exclude-mmf") { q.push(("exclude_mmf", "true".to_string())); }
    let data = client.get_json("/v1/analytics/liquidity-spillover", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator liquidity-panel"]))
}

pub async fn handle_remote_diagnose(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut q = Vec::new();
    if let Some(v) = flag_val(args, &["--as-of"]) { q.push(("as_of", v)); }
    if let Some(v) = flag_val(args, &["--window-weeks", "--weeks"]) { q.push(("window_weeks", v)); }
    if args.iter().any(|a| a == "--persist") { q.push(("persist", "true".to_string())); }
    if args.iter().any(|a| a == "--no-spillover") { q.push(("include_spillover", "false".to_string())); }
    let data = client.get_json("/v1/analytics/liquidity-tree", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator diagnose-history", "economy-aggregator transitions"]))
}

pub async fn handle_remote_diagnose_history(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut q = Vec::new();
    if let Some(v) = flag_val(args, &["--limit"]) { q.push(("limit", v)); }
    let data = client.get_json("/v1/analytics/liquidity-tree/history", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator diagnose-run <run_id>"]))
}

pub async fn handle_remote_diagnose_run(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let id = args.first().filter(|a| !a.starts_with('-')).cloned()
        .ok_or_else(|| RemoteCliError::config_or_auth("Usage: economy-aggregator diagnose-run <run_id>", None))?;
    let data = client.get_json(&format!("/v1/analytics/liquidity-tree/runs/{}", id), &[]).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator diagnose-history"]))
}

pub async fn handle_remote_transitions(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut q = Vec::new();
    if let Some(v) = flag_val(args, &["--window-weeks", "--weeks"]) { q.push(("window_weeks", v)); }
    if let Some(v) = flag_val(args, &["--limit"]) { q.push(("limit", v)); }
    let data = client.get_json("/v1/analytics/liquidity-tree/transitions", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator continuous-status"]))
}

pub async fn handle_remote_prospective(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut q = Vec::new();
    if let Some(v) = flag_val(args, &["--as-of"]) { q.push(("as_of", v)); }
    if args.iter().any(|a| a == "--persist") { q.push(("persist", "true".to_string())); }
    if args.iter().any(|a| a == "--no-spillover") { q.push(("include_spillover", "false".to_string())); }
    let data = client.get_json("/v1/analytics/prospective-scorecard", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator continuous-status"]))
}

pub async fn handle_remote_continuous_status(client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let data = client.get_json("/v1/analytics/continuous/status", &[]).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator continuous-run", "economy-aggregator transitions"]))
}

pub async fn handle_remote_continuous_run(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut path = "/v1/analytics/continuous/run".to_string();
    let mut params = Vec::new();
    if let Some(v) = flag_val(args, &["--as-of"]) { params.push(format!("as_of={}", v)); }
    if args.iter().any(|a| a == "--force") { params.push("force=true".to_string()); }
    if args.iter().any(|a| a == "--no-spillover") { params.push("include_spillover=false".to_string()); }
    if !params.is_empty() {
        path.push('?');
        path.push_str(&params.join("&"));
    }
    let data = client.post_json(&path, None).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator continuous-status", "economy-aggregator transitions"]))
}

pub async fn handle_remote_atlas(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut q = Vec::new();
    if let Some(v) = flag_val(args, &["--as-of"]) { q.push(("as_of", v)); }
    if let Some(v) = flag_val(args, &["--range"]) { q.push(("range", v)); }
    if let Some(v) = flag_val(args, &["--window-weeks", "--weeks"]) { q.push(("window_weeks", v)); }
    if args.iter().any(|a| a == "--include-spillover") { q.push(("include_spillover", "true".to_string())); }
    let data = client.get_json("/v1/analytics/liquidity-atlas", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator diagnose", "economy-aggregator transitions"]))
}
