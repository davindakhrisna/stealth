use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use url::Url;

use regex::Regex;

use crate::error::StealthError;

pub const DEFAULT_AGY_MODEL: &str = "gemini-3.8-flash-low";
pub const DEFAULT_AGY_EFFORT: &str = "low";
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";
pub const DEFAULT_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

static KEY_PAT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9._-]+$").unwrap());
static MODEL_PAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]*(?:/[A-Za-z0-9][A-Za-z0-9._-]*)?(?::[A-Za-z0-9][A-Za-z0-9._-]*)?$").unwrap()
});
static ASSIGN_PAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=(.*)$").unwrap());

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub agy_model: String,
    pub agy_effort: String,
    pub gemini_key: String,
    pub gemini_model: String,
    pub gemini_base_url: String,
}

pub fn config_path() -> PathBuf {
    if let Ok(p) = env::var("STEALTH_ENV_FILE")
        .or_else(|_| env::var("ACADEMIC_ENV_FILE"))
        .or_else(|_| env::var("SHELLCUT_ENV_FILE"))
    {
        return PathBuf::from(p);
    }
    let local = Path::new(".env");
    let home = env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let fallback = home.join("nixos-config").join(".env");
    if local.is_file() || !fallback.is_file() {
        local.to_path_buf()
    } else {
        fallback
    }
}

pub fn parse_env(text: &str) -> Result<HashMap<String, String>, StealthError> {
    let mut values = HashMap::new();
    for (idx, line) in text.lines().enumerate() {
        let (num, trimmed) = (idx + 1, line.trim());
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(caps) = ASSIGN_PAT.captures(line) else {
            return Err(StealthError::Config(format!(
                "Invalid .env assignment on line {num}."
            )));
        };
        let raw = caps[2].trim();
        let val = if (raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2)
            || (raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2)
        {
            raw[1..raw.len() - 1].to_string()
        } else {
            let clean = raw.split('#').next().unwrap_or("").trim();
            if clean.split_whitespace().count() > 1 {
                return Err(StealthError::Config(format!(
                    "Quote values containing spaces in .env (line {num})."
                )));
            }
            clean.to_string()
        };
        values.insert(caps[1].to_string(), val);
    }
    Ok(values)
}

pub fn validate_model(m: &str) -> Result<&str, StealthError> {
    if !MODEL_PAT.is_match(m) {
        return Err(StealthError::Config("Invalid model name format.".into()));
    }
    Ok(m)
}

pub fn validate_effort(e: &str) -> Result<String, StealthError> {
    let normalized = e.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "low" | "medium" | "high" | "xhigh" | "max" => Ok(normalized),
        _ => Err(StealthError::Config(
            "Invalid effort level. Use one of: low, medium, high, xhigh, max.".into(),
        )),
    }
}

pub fn validate_base_url(value: &str) -> Result<String, StealthError> {
    let err = StealthError::Config("Invalid API base URL; use an HTTP(S) URL.".into());
    let Ok(u) = Url::parse(value) else {
        return Err(err);
    };
    if (u.scheme() != "http" && u.scheme() != "https")
        || !u.has_host()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || value.chars().any(|c| c.is_whitespace() || (c as u32) < 32)
        || u.port() == Some(0)
    {
        return Err(err);
    }
    Ok(value.trim_end_matches('/').to_string())
}

pub fn load_config() -> Result<Config, StealthError> {
    let path = config_path();
    let has_override = env::var("STEALTH_ENV_FILE").is_ok()
        || env::var("ACADEMIC_ENV_FILE").is_ok()
        || env::var("SHELLCUT_ENV_FILE").is_ok();
    let file_vals = if path.exists() {
        let content = fs::read_to_string(&path).map_err(|_| {
            StealthError::Config(format!("Cannot read configuration: {}", path.display()))
        })?;
        parse_env(&content)?
    } else if has_override {
        return Err(StealthError::Config(format!(
            "Configuration file does not exist: {}",
            path.display()
        )));
    } else {
        HashMap::new()
    };

    let agy_model_raw = file_vals
        .get("AGY_MODEL")
        .cloned()
        .or_else(|| env::var("AGY_MODEL").ok())
        .or_else(|| {
            file_vals
                .get("ROUTER_MODEL")
                .filter(|m| m.as_str() != "Academic")
                .cloned()
        })
        .or_else(|| {
            env::var("ROUTER_MODEL")
                .ok()
                .filter(|m| m.as_str() != "Academic")
        })
        .unwrap_or_else(|| DEFAULT_AGY_MODEL.to_string());
    let agy_model = validate_model(&agy_model_raw)?.to_string();

    let agy_effort_raw = file_vals
        .get("AGY_EFFORT")
        .cloned()
        .or_else(|| env::var("AGY_EFFORT").ok())
        .unwrap_or_else(|| DEFAULT_AGY_EFFORT.to_string());
    let agy_effort = validate_effort(&agy_effort_raw)?;

    let gemini_key = file_vals
        .get("GEMINI_API_KEY")
        .cloned()
        .or_else(|| env::var("GEMINI_API_KEY").ok())
        .unwrap_or_default();

    if !gemini_key.is_empty() && !KEY_PAT.is_match(&gemini_key) {
        return Err(StealthError::Config(
            "Invalid GEMINI_API_KEY format in configuration.".into(),
        ));
    }
    if gemini_key.starts_with("your_") {
        return Err(StealthError::Config(
            "Replace placeholder GEMINI_API_KEY in .env before solving.".into(),
        ));
    }

    let gemini_model_raw = file_vals
        .get("GEMINI_MODEL")
        .cloned()
        .or_else(|| env::var("GEMINI_MODEL").ok())
        .unwrap_or_else(|| DEFAULT_GEMINI_MODEL.to_string());
    let gemini_model = validate_model(&gemini_model_raw)?.to_string();

    let gemini_base_raw = file_vals
        .get("GEMINI_BASE_URL")
        .cloned()
        .or_else(|| env::var("GEMINI_BASE_URL").ok())
        .unwrap_or_else(|| DEFAULT_GEMINI_BASE_URL.to_string());
    let gemini_base_url = validate_base_url(&gemini_base_raw)?;

    Ok(Config {
        agy_model,
        agy_effort,
        gemini_key,
        gemini_model,
        gemini_base_url,
    })
}

pub fn set_key_value(
    key: &str,
    value: &str,
    target_path: Option<&Path>,
) -> Result<PathBuf, StealthError> {
    let path = target_path.map(PathBuf::from).unwrap_or_else(config_path);
    let original = if path.exists() {
        fs::read_to_string(&path).map_err(|_| {
            StealthError::Config(format!("Cannot update configuration: {}", path.display()))
        })?
    } else {
        String::new()
    };

    let mut lines: Vec<String> = original
        .lines()
        .filter(|l| ASSIGN_PAT.captures(l).map(|c| &c[1] != key).unwrap_or(true))
        .map(|s| s.to_string())
        .collect();
    lines.push(format!("{key}={value}"));
    let updated = lines.join("\n") + "\n";
    parse_env(&updated)?;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::Builder::new()
        .prefix(".env.tmp.")
        .tempfile_in(parent)
        .map_err(|_| {
            StealthError::Config(format!("Cannot update configuration: {}", path.display()))
        })?;
    temp.write_all(updated.as_bytes()).map_err(|_| {
        StealthError::Config(format!("Cannot update configuration: {}", path.display()))
    })?;
    temp.flush().map_err(|_| {
        StealthError::Config(format!("Cannot update configuration: {}", path.display()))
    })?;
    temp.persist(&path).map_err(|_| {
        StealthError::Config(format!("Cannot update configuration: {}", path.display()))
    })?;

    Ok(path)
}

pub fn set_model(model: &str, target_path: Option<&Path>) -> Result<PathBuf, StealthError> {
    validate_model(model)?;
    set_key_value("AGY_MODEL", model, target_path)
}

pub fn set_effort(effort: &str, target_path: Option<&Path>) -> Result<PathBuf, StealthError> {
    let validated = validate_effort(effort)?;
    set_key_value("AGY_EFFORT", &validated, target_path)
}
