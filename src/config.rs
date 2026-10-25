use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use url::Url;

use regex::Regex;

use crate::error::AcademicError;

pub const DEFAULT_MODEL: &str = "Academic";
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:20128/v1";

static KEY_PAT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9._-]+$").unwrap());
static MODEL_PAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]*(?:/[A-Za-z0-9][A-Za-z0-9._-]*)?(?::[A-Za-z0-9][A-Za-z0-9._-]*)?$").unwrap()
});
static ASSIGN_PAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=(.*)$").unwrap());

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub model: String,
    pub key: String,
    pub base_url: String,
}

pub fn config_path() -> PathBuf {
    if let Ok(p) = env::var("ACADEMIC_ENV_FILE").or_else(|_| env::var("SHELLCUT_ENV_FILE")) {
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

pub fn parse_env(text: &str) -> Result<HashMap<String, String>, AcademicError> {
    let mut values = HashMap::new();
    for (idx, line) in text.lines().enumerate() {
        let (num, trimmed) = (idx + 1, line.trim());
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(caps) = ASSIGN_PAT.captures(line) else {
            return Err(AcademicError::Config(format!(
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
                return Err(AcademicError::Config(format!(
                    "Quote values containing spaces in .env (line {num})."
                )));
            }
            clean.to_string()
        };
        values.insert(caps[1].to_string(), val);
    }
    Ok(values)
}

pub fn validate_model(m: &str) -> Result<&str, AcademicError> {
    if !MODEL_PAT.is_match(m) {
        return Err(AcademicError::Config(
            "Invalid 9Router model or combo name.".into(),
        ));
    }
    Ok(m)
}

pub fn validate_base_url(value: &str) -> Result<String, AcademicError> {
    let err = AcademicError::Config("Invalid ROUTER_BASE_URL; use an HTTP(S) API base URL.".into());
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

pub fn load_config() -> Result<Config, AcademicError> {
    let path = config_path();
    let has_override =
        env::var("ACADEMIC_ENV_FILE").is_ok() || env::var("SHELLCUT_ENV_FILE").is_ok();
    let file_vals = if path.exists() {
        let content = fs::read_to_string(&path).map_err(|_| {
            AcademicError::Config(format!("Cannot read configuration: {}", path.display()))
        })?;
        parse_env(&content)?
    } else if has_override {
        return Err(AcademicError::Config(format!(
            "Configuration file does not exist: {}",
            path.display()
        )));
    } else {
        HashMap::new()
    };

    let model_raw = file_vals
        .get("ROUTER_MODEL")
        .cloned()
        .or_else(|| env::var("ROUTER_MODEL").ok())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let model = validate_model(&model_raw)?.to_string();

    let base_raw = file_vals
        .get("ROUTER_BASE_URL")
        .cloned()
        .or_else(|| env::var("ROUTER_BASE_URL").ok())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let base_url = validate_base_url(&base_raw)?;

    let key = file_vals
        .get("ROUTER_API_KEY")
        .cloned()
        .or_else(|| env::var("ROUTER_API_KEY").ok())
        .unwrap_or_default();

    if !key.is_empty() && !KEY_PAT.is_match(&key) {
        return Err(AcademicError::Config(
            "Invalid API key format in configuration.".into(),
        ));
    }
    if key.starts_with("your_") {
        return Err(AcademicError::Config(
            "Replace placeholder API key in .env before solving.".into(),
        ));
    }

    Ok(Config {
        model,
        key,
        base_url,
    })
}

pub fn set_model(model: &str, target_path: Option<&Path>) -> Result<PathBuf, AcademicError> {
    validate_model(model)?;
    let path = target_path.map(PathBuf::from).unwrap_or_else(config_path);
    let original = if path.exists() {
        fs::read_to_string(&path).map_err(|_| {
            AcademicError::Config(format!("Cannot update configuration: {}", path.display()))
        })?
    } else {
        String::new()
    };

    let mut lines: Vec<String> = original
        .lines()
        .filter(|l| {
            ASSIGN_PAT
                .captures(l)
                .map(|c| &c[1] != "ROUTER_MODEL")
                .unwrap_or(true)
        })
        .map(|s| s.to_string())
        .collect();
    lines.push(format!("ROUTER_MODEL={model}"));
    let updated = lines.join("\n") + "\n";
    parse_env(&updated)?;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::Builder::new()
        .prefix(".env.tmp.")
        .tempfile_in(parent)
        .map_err(|_| {
            AcademicError::Config(format!("Cannot update configuration: {}", path.display()))
        })?;
    temp.write_all(updated.as_bytes()).map_err(|_| {
        AcademicError::Config(format!("Cannot update configuration: {}", path.display()))
    })?;
    temp.flush().map_err(|_| {
        AcademicError::Config(format!("Cannot update configuration: {}", path.display()))
    })?;
    temp.persist(&path).map_err(|_| {
        AcademicError::Config(format!("Cannot update configuration: {}", path.display()))
    })?;

    Ok(path)
}
