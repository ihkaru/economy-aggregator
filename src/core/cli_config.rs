//! 3-tier Agent-First CLI configuration and secret resolution.
//!
//! Resolution hierarchy (arc42 Section 8.8):
//! 1. Command-line flags (`--base-url`, `--api-key`)
//! 2. Environment variables (`ECONOMY_AGGREGATOR_*` / `AGGREGATOR_*`)
//! 3. Persistent config file (`~/.config/economy-aggregator/config.json` or `ECONOMY_AGGREGATOR_CONFIG`)

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GlobalCliOptions {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub force_local: bool,
    pub remaining: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedClientConfig {
    pub base_url: Option<String>,
    pub base_url_source: &'static str,
    pub api_key: Option<String>,
    pub api_key_source: &'static str,
    pub force_local: bool,
    pub config_path: PathBuf,
}

impl ResolvedClientConfig {
    pub fn effective_base_url(&self) -> Option<&str> {
        if self.force_local {
            None
        } else {
            self.base_url.as_deref()
        }
    }
}

/// Masks a secret for safe terminal/JSON output (never prints raw tokens).
pub fn mask_secret(secret: &str) -> String {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 8 {
        "***".to_string()
    } else {
        let prefix: String = chars[..4].iter().collect();
        let suffix: String = chars[chars.len() - 2..].iter().collect();
        format!("{}***{}", prefix, suffix)
    }
}

/// Normalizes and validates a remote HTTP(S) base URL.
pub fn normalize_base_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(format!(
            "Invalid base_url '{}': must start with http:// or https://",
            raw
        ));
    }
    if trimmed.len() <= "https://".len() && trimmed.starts_with("https://")
        || trimmed.len() <= "http://".len() && trimmed.starts_with("http://")
    {
        return Err(format!("Invalid base_url '{}': missing host", raw));
    }
    Ok(trimmed.to_string())
}

/// Resolves the persistent config file path.
pub fn default_config_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("ECONOMY_AGGREGATOR_CONFIG") {
        if !override_path.trim().is_empty() {
            return PathBuf::from(override_path);
        }
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.trim().is_empty() {
            return PathBuf::from(xdg)
                .join("economy-aggregator")
                .join("config.json");
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("economy-aggregator")
        .join("config.json")
}

pub fn load_config_file(path: &Path) -> ClientConfig {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<ClientConfig>(&s).ok())
        .unwrap_or_default()
}

pub fn save_config_file(path: &Path, cfg: &ClientConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed creating config directory: {}", e))?;
    }
    let serialized = serde_json::to_string_pretty(cfg)
        .map_err(|e| format!("Failed serializing config: {}", e))?;
    fs::write(path, serialized).map_err(|e| format!("Failed writing config file: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Extracts global flags (`--base-url`, `--api-key`, `--local`) from any position in `args`.
pub fn extract_global_options(args: &[String]) -> GlobalCliOptions {
    let mut opts = GlobalCliOptions {
        base_url: None,
        api_key: None,
        force_local: false,
        remaining: Vec::with_capacity(args.len()),
    };

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if let Some(val) = arg
            .strip_prefix("--base-url=")
            .or_else(|| arg.strip_prefix("--url="))
        {
            opts.base_url = Some(val.to_string());
            i += 1;
        } else if let Some(val) = arg
            .strip_prefix("--api-key=")
            .or_else(|| arg.strip_prefix("--key="))
        {
            opts.api_key = Some(val.to_string());
            i += 1;
        } else if matches!(arg, "--base-url" | "--url") && i + 1 < args.len() {
            opts.base_url = Some(args[i + 1].clone());
            i += 2;
        } else if matches!(arg, "--api-key" | "--key") && i + 1 < args.len() {
            opts.api_key = Some(args[i + 1].clone());
            i += 2;
        } else if arg == "--local" {
            opts.force_local = true;
            i += 1;
        } else {
            opts.remaining.push(args[i].clone());
            i += 1;
        }
    }
    opts
}

/// Resolves effective client config using strict 3-tier precedence: Flags > Env > Config File.
pub fn resolve_client_config(
    global: &GlobalCliOptions,
    config_path: Option<PathBuf>,
) -> Result<ResolvedClientConfig, String> {
    let path = config_path.unwrap_or_else(default_config_path);
    let file_cfg = load_config_file(&path);

    let env_url = std::env::var("ECONOMY_AGGREGATOR_BASE_URL")
        .or_else(|_| std::env::var("AGGREGATOR_BASE_URL"))
        .ok()
        .filter(|s| !s.trim().is_empty());

    let env_key = std::env::var("ECONOMY_AGGREGATOR_API_KEY")
        .or_else(|_| std::env::var("AGGREGATOR_API_KEY"))
        .ok()
        .filter(|s| !s.trim().is_empty());

    let (raw_url, url_src) = if let Some(u) = global.base_url.as_ref().filter(|s| !s.trim().is_empty()) {
        (Some(u.clone()), "flag")
    } else if let Some(u) = env_url {
        (Some(u), "env")
    } else if let Some(u) = file_cfg.base_url.filter(|s| !s.trim().is_empty()) {
        (Some(u), "file")
    } else {
        (None, "unset")
    };

    let normalized_url = match raw_url {
        Some(u) => Some(normalize_base_url(&u)?),
        None => None,
    };

    let (api_key, key_src) = if let Some(k) = global.api_key.as_ref().filter(|s| !s.trim().is_empty()) {
        (Some(k.trim().to_string()), "flag")
    } else if let Some(k) = env_key {
        (Some(k.trim().to_string()), "env")
    } else if let Some(k) = file_cfg.api_key.filter(|s| !s.trim().is_empty()) {
        (Some(k.trim().to_string()), "file")
    } else {
        (None, "unset")
    };

    Ok(ResolvedClientConfig {
        base_url: normalized_url,
        base_url_source: url_src,
        api_key,
        api_key_source: key_src,
        force_local: global.force_local,
        config_path: path,
    })
}

/// Handles `economy-aggregator config <set|show|clear>` with standardized JSON output.
pub fn handle_config(
    args: &[String],
    global: &GlobalCliOptions,
    config_path: Option<PathBuf>,
) -> Result<serde_json::Value, String> {
    let path = config_path.unwrap_or_else(default_config_path);
    let action = args.first().map(|s| s.as_str()).unwrap_or("show");

    match action {
        "set" => {
            if global.base_url.is_none() && global.api_key.is_none() {
                return Err(
                    "Provide at least --base-url <url> or --api-key <key> to set".to_string(),
                );
            }
            let mut cfg = load_config_file(&path);
            if let Some(url) = &global.base_url {
                cfg.base_url = Some(normalize_base_url(url)?);
            }
            if let Some(key) = &global.api_key {
                let k = key.trim();
                if k.is_empty() {
                    return Err("API key cannot be empty".to_string());
                }
                cfg.api_key = Some(k.to_string());
            }
            save_config_file(&path, &cfg)?;
            Ok(serde_json::json!({
                "success": true,
                "error": null,
                "data": {
                    "action": "config.set",
                    "config_path": path.display().to_string(),
                    "base_url": cfg.base_url,
                    "api_key_masked": cfg.api_key.as_deref().map(mask_secret)
                },
                "next_steps": [
                    "economy-aggregator health",
                    "economy-aggregator catalog --limit 10"
                ]
            }))
        }
        "clear" => {
            let clear_all = args.iter().any(|a| a == "--all")
                || (!args.iter().any(|a| a == "--base-url" || a == "--api-key"));
            let mut cfg = load_config_file(&path);
            if clear_all || args.iter().any(|a| a == "--base-url") {
                cfg.base_url = None;
            }
            if clear_all || args.iter().any(|a| a == "--api-key") {
                cfg.api_key = None;
            }
            save_config_file(&path, &cfg)?;
            Ok(serde_json::json!({
                "success": true,
                "error": null,
                "data": {
                    "action": "config.clear",
                    "config_path": path.display().to_string(),
                    "base_url": cfg.base_url,
                    "api_key_masked": cfg.api_key.as_deref().map(mask_secret)
                },
                "next_steps": [
                    "economy-aggregator config show"
                ]
            }))
        }
        "show" => {
            let resolved = resolve_client_config(global, Some(path))?;
            Ok(serde_json::json!({
                "success": true,
                "error": null,
                "data": {
                    "config_path": resolved.config_path.display().to_string(),
                    "mode": if resolved.effective_base_url().is_some() { "remote" } else { "local" },
                    "base_url": resolved.base_url,
                    "base_url_source": resolved.base_url_source,
                    "api_key_configured": resolved.api_key.is_some(),
                    "api_key_masked": resolved.api_key.as_deref().map(mask_secret),
                    "api_key_source": resolved.api_key_source
                },
                "next_steps": [
                    "economy-aggregator config set --base-url https://your-host --api-key <key>",
                    "economy-aggregator health"
                ]
            }))
        }
        other => Err(format!(
            "Unknown config action '{}'. Use 'set', 'show', or 'clear'.",
            other
        )),
    }
}

pub fn handle_hash_password(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pw = args.first().map(|s| s.as_str()).unwrap_or("");
    match crate::features::serving::session::hash_password_phc(pw) {
        Ok(hash) => {
            println!("{}", serde_json::json!({"success": true, "argon2id_hash": hash}));
            Ok(())
        }
        Err(e) => {
            eprintln!("{}", serde_json::json!({"success": false, "error": e}));
            std::process::exit(1);
        }
    }
}
