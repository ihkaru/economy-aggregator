//! Remote HTTP client and Agent-First CLI command dispatcher.
//!
//! When `base_url` is configured (via flag, env, or config file), CLI commands
//! talk to the remote `economy-aggregator` REST server with API key auth and
//! return standardized JSON envelopes with deterministic exit codes.

use std::time::Duration;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct RemoteCliError {
    pub exit_code: i32,
    pub message: String,
    pub status_code: Option<u16>,
}

impl RemoteCliError {
    pub fn config_or_auth(msg: impl Into<String>, status: Option<u16>) -> Self {
        Self { exit_code: 1, message: msg.into(), status_code: status }
    }

    pub fn network_or_rate_limit(msg: impl Into<String>, status: Option<u16>) -> Self {
        Self { exit_code: 2, message: msg.into(), status_code: status }
    }

    pub fn server(msg: impl Into<String>, status: Option<u16>) -> Self {
        Self { exit_code: 3, message: msg.into(), status_code: status }
    }

    pub fn to_envelope(&self) -> Value {
        serde_json::json!({
            "success": false,
            "error": self.message,
            "status_code": self.status_code,
            "exit_code": self.exit_code,
            "data": null
        })
    }
}

#[derive(Debug, Clone)]
pub struct RemoteClient {
    http: reqwest::Client,
    base_url: String,
}

impl RemoteClient {
    pub fn new(base_url: String, api_key: Option<String>) -> Result<Self, RemoteCliError> {
        let mut headers = HeaderMap::new();
        if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
            let bearer = format!("Bearer {}", key.trim());
            let auth_val = HeaderValue::from_str(&bearer)
                .map_err(|_| RemoteCliError::config_or_auth("Invalid characters in API key", None))?;
            let x_val = HeaderValue::from_str(key.trim())
                .map_err(|_| RemoteCliError::config_or_auth("Invalid characters in API key", None))?;
            headers.insert(AUTHORIZATION, auth_val);
            headers.insert("x-api-key", x_val);
        }

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| RemoteCliError::network_or_rate_limit(e.to_string(), None))?;

        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn get_json(&self, path: &str, query: &[(&str, String)]) -> Result<Value, RemoteCliError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .query(query)
            .send()
            .await
            .map_err(|e| RemoteCliError::network_or_rate_limit(format!("Request failed: {}", e), None))?;
        Self::parse_response(resp).await
    }

    pub async fn post_json(&self, path: &str, body: Option<&Value>) -> Result<Value, RemoteCliError> {
        let url = format!("{}{}", self.base_url, path);
        let req = self.http.post(&url);
        let req = if let Some(b) = body { req.json(b) } else { req };
        let resp = req
            .send()
            .await
            .map_err(|e| RemoteCliError::network_or_rate_limit(format!("Request failed: {}", e), None))?;
        Self::parse_response(resp).await
    }

    pub async fn delete_json(&self, path: &str, query: &[(&str, String)]) -> Result<Value, RemoteCliError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .delete(&url)
            .query(query)
            .send()
            .await
            .map_err(|e| RemoteCliError::network_or_rate_limit(format!("Request failed: {}", e), None))?;
        Self::parse_response(resp).await
    }

    async fn parse_response(resp: reqwest::Response) -> Result<Value, RemoteCliError> {
        let status = resp.status();
        let code = status.as_u16();
        let text = resp
            .text()
            .await
            .map_err(|e| RemoteCliError::network_or_rate_limit(format!("Read body failed: {}", e), Some(code)))?;

        if !status.is_success() {
            let msg = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| format!("HTTP {}: {}", code, text.trim()));
            return Err(match code {
                400 | 401 | 403 | 404 | 422 => RemoteCliError::config_or_auth(msg, Some(code)),
                429 => RemoteCliError::network_or_rate_limit(msg, Some(code)),
                _ => RemoteCliError::server(msg, Some(code)),
            });
        }

        serde_json::from_str::<Value>(&text)
            .map_err(|e| RemoteCliError::server(format!("Invalid JSON response: {}", e), Some(code)))
    }
}

fn flag_val(args: &[String], names: &[&str]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        for name in names {
            if let Some(v) = a.strip_prefix(&format!("{}=", name)) {
                return Some(v.to_string());
            }
            if a == *name && i + 1 < args.len() {
                return Some(args[i + 1].clone());
            }
        }
        i += 1;
    }
    None
}

fn wrap_ok(base_url: &str, data: Value, next_steps: Vec<&str>) -> Value {
    serde_json::json!({
        "success": true,
        "error": null,
        "mode": "remote",
        "base_url": base_url,
        "data": data,
        "next_steps": next_steps
    })
}

/// Dispatches a CLI subcommand against the remote REST server.
/// Returns `None` if the command is strictly local-only (e.g. `compact`, `analyze`).
pub async fn dispatch_remote(
    cmd: &str,
    args: &[String],
    client: &RemoteClient,
) -> Option<Result<Value, RemoteCliError>> {
    match cmd {
        "health" => Some(handle_remote_health(client).await),
        "status" => Some(handle_remote_status(client).await),
        "catalog" => Some(handle_remote_catalog(args, client).await),
        "candles" => Some(handle_remote_candles(args, client).await),
        "partitions" => Some(handle_remote_partitions(args, client).await),
        "calendar" => Some(handle_remote_calendar(args, client).await),
        "predictions" => Some(handle_remote_predictions(args, client).await),
        "search" => Some(handle_remote_search(args, client).await),
        "subscriptions" => Some(handle_remote_subscriptions(client).await),
        "subscribe" => Some(handle_remote_subscribe(args, client).await),
        "unsubscribe" => Some(handle_remote_unsubscribe(args, client).await),
        "catch-up" | "sync" => Some(handle_remote_catch_up(args, client).await),
        "compact" => Some(handle_remote_compact(args, client).await),
        "audit" => Some(handle_remote_simple_get("/v1/integrity/audit", client).await),
        "divergence" => Some(handle_remote_simple_get("/v1/analytics/divergence", client).await),
        _ => None,
    }
}

async fn handle_remote_health(client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let data = client.get_json("/v1/health", &[]).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator catalog --limit 10"]))
}

async fn handle_remote_status(client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let stats = client.get_json("/v1/storage/stats", &[]).await?;
    let ingestion = client.get_json("/v1/ingestion/status", &[]).await?;
    let data = serde_json::json!({ "storage": stats, "ingestion": ingestion });
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator partitions --limit 20"]))
}

async fn handle_remote_catalog(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut q = Vec::new();
    if let Some(v) = flag_val(args, &["--q", "--query", "-q"]) { q.push(("q", v)); }
    if let Some(v) = flag_val(args, &["--asset", "--asset-class", "-a"]) { q.push(("asset_class", v)); }
    if let Some(v) = flag_val(args, &["--source", "-s"]) { q.push(("source", v)); }
    if let Some(v) = flag_val(args, &["--limit", "-l"]) { q.push(("limit", v)); }
    if let Some(v) = flag_val(args, &["--offset"]) { q.push(("offset", v)); }
    let data = client.get_json("/v1/market/catalog", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec![
        "economy-aggregator candles --symbol <SYMBOL> --asset <CLASS> --tf 1d --limit 100"
    ]))
}

async fn handle_remote_candles(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let symbol = flag_val(args, &["--symbol", "-s"])
        .ok_or_else(|| RemoteCliError::config_or_auth("Missing required --symbol flag", None))?;
    let asset = flag_val(args, &["--asset", "--asset-class", "-a"]).unwrap_or_else(|| "equities".to_string());
    let tf = flag_val(args, &["--tf", "--timeframe"]).unwrap_or_else(|| "1d".to_string());

    let mut q = vec![("symbol", symbol), ("asset_class", asset), ("timeframe", tf)];
    if let Some(v) = flag_val(args, &["--from"]) { q.push(("from", v)); }
    if let Some(v) = flag_val(args, &["--to"]) { q.push(("to", v)); }
    if let Some(v) = flag_val(args, &["--start-date"]) { q.push(("start_date", v)); }
    if let Some(v) = flag_val(args, &["--end-date"]) { q.push(("end_date", v)); }
    if let Some(v) = flag_val(args, &["--limit", "-l"]) { q.push(("limit", v)); }

    let data = client.get_json("/v1/market/candles", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec![]))
}

async fn handle_remote_partitions(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut q = Vec::new();
    if let Some(v) = flag_val(args, &["--domain"]) { q.push(("domain", v)); }
    if let Some(v) = flag_val(args, &["--q", "--query", "-q"]) { q.push(("q", v)); }
    if let Some(v) = flag_val(args, &["--tf", "--timeframe"]) { q.push(("timeframe", v)); }
    if let Some(v) = flag_val(args, &["--year"]) { q.push(("year", v)); }
    if let Some(v) = flag_val(args, &["--month"]) { q.push(("month", v)); }
    if let Some(v) = flag_val(args, &["--limit", "-l"]) { q.push(("limit", v)); }
    let data = client.get_json("/v1/storage/partitions", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec![]))
}

async fn handle_remote_calendar(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut q = Vec::new();
    if let Some(v) = flag_val(args, &["--country"]) { q.push(("country", v)); }
    if let Some(v) = flag_val(args, &["--impact", "--importance"]) { q.push(("importance", v)); }
    if let Some(v) = flag_val(args, &["--indicator"]) { q.push(("indicator", v)); }
    if let Some(v) = flag_val(args, &["--source"]) { q.push(("source", v)); }
    if let Some(v) = flag_val(args, &["--year"]) { q.push(("year", v)); }
    if let Some(v) = flag_val(args, &["--month"]) { q.push(("month", v)); }
    if let Some(v) = flag_val(args, &["--limit", "-l"]) { q.push(("limit", v)); }
    let data = client.get_json("/v1/macro/calendar", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec![]))
}

async fn handle_remote_predictions(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let mut q = Vec::new();
    if let Some(v) = flag_val(args, &["--venue"]) { q.push(("venue", v)); }
    if let Some(v) = flag_val(args, &["--q", "--query", "-q"]) { q.push(("q", v)); }
    if let Some(v) = flag_val(args, &["--status"]) { q.push(("status", v)); }
    if let Some(v) = flag_val(args, &["--limit", "-l"]) { q.push(("limit", v)); }
    let data = client.get_json("/v1/prediction-markets/events", &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec![]))
}

async fn handle_remote_search(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let query = flag_val(args, &["--query", "--q", "-q"])
        .or_else(|| args.iter().find(|a| !a.starts_with('-')).cloned())
        .ok_or_else(|| RemoteCliError::config_or_auth("Missing required --query parameter", None))?;
    let source = flag_val(args, &["--source", "-s"]).unwrap_or_else(|| "all".to_string());
    let data = client.get_json("/v1/discovery/search", &[("q", query), ("source", source)]).await?;
    Ok(wrap_ok(client.base_url(), data, vec![
        "economy-aggregator subscribe --source <src> --symbol <sym> --asset <class>"
    ]))
}

async fn handle_remote_subscriptions(client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let data = client.get_json("/v1/subscriptions", &[]).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator catch-up"]))
}

async fn handle_remote_subscribe(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let symbol = flag_val(args, &["--symbol", "-s"])
        .ok_or_else(|| RemoteCliError::config_or_auth("Missing required --symbol flag", None))?;
    let source = flag_val(args, &["--source"]).unwrap_or_else(|| "yahoo".to_string());
    let asset = flag_val(args, &["--asset", "-a"]).unwrap_or_else(|| "equities".to_string());
    let timeframes: Vec<String> = flag_val(args, &["--tf", "--timeframes"])
        .unwrap_or_else(|| "1d".to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let poll_interval_secs = flag_val(args, &["--interval"]).and_then(|v| v.parse::<u64>().ok());
    let body = serde_json::json!({
        "source": source,
        "symbol": symbol,
        "asset_class": asset,
        "timeframes": timeframes,
        "enabled": true,
        "poll_interval_secs": poll_interval_secs,
        "created_at_utc": chrono::Utc::now().to_rfc3339()
    });
    let data = client.post_json("/v1/subscriptions", Some(&body)).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator catch-up"]))
}

async fn handle_remote_unsubscribe(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let symbol = flag_val(args, &["--symbol", "-s"])
        .or_else(|| args.iter().find(|a| !a.starts_with('-')).cloned())
        .ok_or_else(|| RemoteCliError::config_or_auth("Missing required --symbol flag", None))?;
    let mut q = Vec::new();
    if let Some(src) = flag_val(args, &["--source"]) { q.push(("source", src)); }
    let path = format!("/v1/subscriptions/{}", symbol);
    let data = client.delete_json(&path, &q).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator subscriptions"]))
}

async fn handle_remote_catch_up(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let symbol = flag_val(args, &["--symbol", "-s"])
        .or_else(|| args.iter().find(|a| !a.starts_with('-')).cloned());
    let source = flag_val(args, &["--source"]);
    let force = args.iter().any(|a| matches!(a.as_str(), "--force" | "-f"));
    let run_async = args.iter().any(|a| matches!(a.as_str(), "--async" | "--background"));

    let body = serde_json::json!({
        "symbol": symbol,
        "source": source,
        "force": force,
        "async": run_async
    });
    let data = client.post_json("/v1/ingestion/catch-up", Some(&body)).await?;
    Ok(wrap_ok(client.base_url(), data, vec![
        "economy-aggregator status",
        "economy-aggregator partitions --limit 20"
    ]))
}

async fn handle_remote_compact(args: &[String], client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let min_files = flag_val(args, &["--min-files"]).and_then(|v| v.parse::<usize>().ok());
    let body = serde_json::json!({ "min_files": min_files });
    let data = client.post_json("/v1/storage/compact", Some(&body)).await?;
    Ok(wrap_ok(client.base_url(), data, vec!["economy-aggregator status"]))
}

async fn handle_remote_simple_get(path: &str, client: &RemoteClient) -> Result<Value, RemoteCliError> {
    let data = client.get_json(path, &[]).await?;
    Ok(wrap_ok(client.base_url(), data, vec![]))
}
