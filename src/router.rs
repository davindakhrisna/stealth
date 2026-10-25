use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::json;
use std::io::Read;
use std::time::Duration;

use crate::config::Config;
use crate::error::{redact, AcademicError};

pub const REQUEST_LIMIT: usize = 20_000_000;
pub const RESPONSE_LIMIT: usize = 4_194_304;

pub const ANSWER_PROMPT: &str = "\
Analyze the problem shown in the image and return ONLY the final answer(s).
Do not include steps, headings, reasoning, conversational text, Markdown, or LaTeX.
For multiple choice with letters, return the correct letter(s), e.g. A & C.
For multiple choice without letters, return Option 1, Option 2, etc., top to bottom.
Use standard Unicode math symbols for mathematical answers.
If the problem is incomplete or unreadable, return: Cannot read problem reliably.";

pub struct RouterClient {
    config: Config,
    agent: ureq::Agent,
}

impl RouterClient {
    pub fn new(config: Config) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(120))
            .build();
        Self { config, agent }
    }

    pub fn generate(&self, image: &[u8]) -> Result<String, AcademicError> {
        let base64_img = BASE64.encode(image);
        let data_url = format!("data:image/png;base64,{base64_img}");

        let payload = json!({
            "model": self.config.model,
            "stream": false,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": ANSWER_PROMPT},
                        {"type": "image_url", "image_url": {"url": data_url}}
                    ]
                }
            ]
        });

        let body_bytes = serde_json::to_vec(&payload)
            .map_err(|e| AcademicError::Config(format!("Failed to serialize payload: {e}")))?;

        if body_bytes.len() >= REQUEST_LIMIT {
            return Err(AcademicError::Config(
                "Screenshot is too large for the app's 20 MB request budget.".into(),
            ));
        }

        let endpoint = format!("{}/chat/completions", self.config.base_url);
        let mut req = self
            .agent
            .post(&endpoint)
            .set("Content-Type", "application/json");
        if !self.config.key.is_empty() {
            req = req.set("Authorization", &format!("Bearer {}", self.config.key));
        }

        let response = match req.send_bytes(&body_bytes) {
            Ok(resp) => resp,
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                let msg =
                    parse_error_message(&body).unwrap_or_else(|| "API request failed.".into());
                return Err(AcademicError::Api {
                    message: redact(&msg, &self.config.key),
                    status: Some(code),
                });
            }
            Err(ureq::Error::Transport(t)) => {
                let msg = if t.kind() == ureq::ErrorKind::Io {
                    "Request timed out.".into()
                } else {
                    format!("Network request failed: {t}")
                };
                return Err(AcademicError::Api {
                    message: redact(&msg, &self.config.key),
                    status: None,
                });
            }
        };

        let mut reader = response.into_reader().take((RESPONSE_LIMIT + 1) as u64);
        let mut resp_bytes = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut resp_bytes)?;

        if resp_bytes.len() > RESPONSE_LIMIT {
            return Err(AcademicError::Api {
                message: "API response exceeded the size limit.".into(),
                status: None,
            });
        }

        let json_val: serde_json::Value =
            serde_json::from_slice(&resp_bytes).map_err(|_| AcademicError::Api {
                message: "API returned an empty or malformed response.".into(),
                status: None,
            })?;

        extract_complete_text(&json_val).map_err(|mut err| {
            if let AcademicError::Api {
                ref mut message, ..
            } = err
            {
                *message = redact(message, &self.config.key);
            }
            err
        })
    }
}

fn parse_error_message(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    json.get("error")?
        .get("message")?
        .as_str()
        .map(|s| s.to_string())
}

pub fn extract_complete_text(body: &serde_json::Value) -> Result<String, AcademicError> {
    let candidate =
        body.get("choices")
            .and_then(|c| c.get(0))
            .ok_or_else(|| AcademicError::Api {
                message: "No complete answer received (blocked, empty, or malformed response)."
                    .into(),
                status: None,
            })?;

    let finish_reason = candidate
        .get("finish_reason")
        .and_then(|f| f.as_str())
        .unwrap_or("unknown");
    if finish_reason != "stop" {
        return Err(AcademicError::Api {
            message: format!("No complete answer received ({finish_reason})."),
            status: None,
        });
    }

    let message = candidate.get("message").ok_or_else(|| AcademicError::Api {
        message: "No complete answer received (empty response).".into(),
        status: None,
    })?;

    if message.get("refusal").and_then(|r| r.as_str()).is_some() {
        return Err(AcademicError::Api {
            message: "API declined to answer the request.".into(),
            status: None,
        });
    }

    let text = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim();
    if text.is_empty() {
        return Err(AcademicError::Api {
            message: "No complete answer received (empty response).".into(),
            status: None,
        });
    }

    Ok(text.to_string())
}
