#[derive(Debug, thiserror::Error)]
pub enum AcademicError {
    #[error("{0}")]
    Config(String),

    #[error("{0}")]
    Capture(String),

    #[error("{}", format_api_error(.message, *.status))]
    Api {
        message: String,
        status: Option<u16>,
    },

    #[error("{0}")]
    Notification(String),

    #[error("{0}")]
    Io(#[from] std::io::Error),
}

fn format_api_error(message: &str, status: Option<u16>) -> String {
    match status {
        Some(code) => format!("HTTP {code}: {message}"),
        None => format!("API error: {message}"),
    }
}

pub fn redact(text: &str, key: &str) -> String {
    if key.is_empty() {
        text.to_string()
    } else {
        text.replace(key, "[redacted]")
    }
}
