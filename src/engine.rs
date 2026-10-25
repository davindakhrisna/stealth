use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::json;
use std::io::{Read, Write};
use std::process::Command;
use std::time::Duration;

use crate::config::Config;
use crate::error::{redact, StealthError};

pub const REQUEST_LIMIT: usize = 20_000_000;
pub const RESPONSE_LIMIT: usize = 4_194_304;

pub const ANSWER_PROMPT: &str = "\
Analyze the problem shown in the image and return ONLY the final answer(s).
Do not include steps, headings, reasoning, conversational text, Markdown, or LaTeX.
For multiple choice with letters, return the correct letter(s), e.g. A & C.
For multiple choice without letters, return Option 1, Option 2, etc., top to bottom.
Use standard Unicode math symbols for mathematical answers.
If the problem is incomplete or unreadable, return: Cannot read problem reliably.";

pub struct EngineClient {
    config: Config,
    agent: ureq::Agent,
}

impl EngineClient {
    pub fn new(config: Config) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(120))
            .build();
        Self { config, agent }
    }

    pub fn generate(&self, image: &[u8]) -> Result<String, StealthError> {
        match self.generate_agy(image) {
            Ok(answer) if !answer.trim().is_empty() => Ok(answer),
            Err(agy_err) => {
                if !self.config.gemini_key.is_empty() {
                    self.generate_gemini(image)
                        .map_err(|gem_err| StealthError::Api {
                            message: format!(
                                "AGY failed ({agy_err}); Gemini fallback failed: {gem_err}"
                            ),
                            status: None,
                        })
                } else {
                    Err(agy_err)
                }
            }
            Ok(_) => {
                if !self.config.gemini_key.is_empty() {
                    self.generate_gemini(image)
                } else {
                    Err(StealthError::Api {
                        message: "AGY returned empty response.".into(),
                        status: None,
                    })
                }
            }
        }
    }

    pub fn generate_agy(&self, image: &[u8]) -> Result<String, StealthError> {
        let mut tmp_file = tempfile::Builder::new()
            .prefix("stealth_")
            .suffix(".png")
            .tempfile()
            .map_err(|e| {
                StealthError::Config(format!("Failed to create temporary screenshot file: {e}"))
            })?;

        tmp_file.write_all(image).map_err(|e| {
            StealthError::Config(format!("Failed to write screenshot to temp file: {e}"))
        })?;
        tmp_file.flush().map_err(|e| {
            StealthError::Config(format!("Failed to flush screenshot temp file: {e}"))
        })?;

        let tmp_path = tmp_file.path().to_string_lossy().to_string();
        let prompt = format!(
            "Read the image at {tmp_path} using read_file with StartLine: 1 and EndLine: 50.\n{ANSWER_PROMPT}"
        );

        let output = Command::new("agy")
            .arg("--dangerously-skip-permissions")
            .arg("--model")
            .arg(&self.config.agy_model)
            .arg("--effort")
            .arg(&self.config.agy_effort)
            .arg("-p")
            .arg(&prompt)
            .output()
            .map_err(|e| StealthError::Api {
                message: format!("Failed to spawn agy command: {e}"),
                status: None,
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(StealthError::Api {
                message: format!(
                    "agy exited with status {}: {stderr} {stdout}",
                    output.status
                ),
                status: output.status.code().map(|c| c as u16),
            });
        }

        let raw = String::from_utf8_lossy(&output.stdout);
        let cleaned = clean_llm_response(&raw);
        if cleaned.is_empty() {
            return Err(StealthError::Api {
                message: "agy returned an empty response.".into(),
                status: None,
            });
        }

        Ok(cleaned)
    }

    pub fn generate_gemini(&self, image: &[u8]) -> Result<String, StealthError> {
        let base64_img = BASE64.encode(image);

        let payload = json!({
            "contents": [
                {
                    "parts": [
                        { "text": ANSWER_PROMPT },
                        {
                            "inline_data": {
                                "mime_type": "image/png",
                                "data": base64_img
                            }
                        }
                    ]
                }
            ]
        });

        let body_bytes = serde_json::to_vec(&payload)
            .map_err(|e| StealthError::Config(format!("Failed to serialize payload: {e}")))?;

        if body_bytes.len() >= REQUEST_LIMIT {
            return Err(StealthError::Config(
                "Screenshot is too large for the 20 MB request budget.".into(),
            ));
        }

        let endpoint = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.config.gemini_base_url, self.config.gemini_model, self.config.gemini_key
        );

        let req = self
            .agent
            .post(&endpoint)
            .set("Content-Type", "application/json");

        let response = match req.send_bytes(&body_bytes) {
            Ok(resp) => resp,
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                let msg = parse_gemini_error(&body)
                    .unwrap_or_else(|| "Gemini API request failed.".into());
                return Err(StealthError::Api {
                    message: redact(&msg, &self.config.gemini_key),
                    status: Some(code),
                });
            }
            Err(ureq::Error::Transport(t)) => {
                let msg = if t.kind() == ureq::ErrorKind::Io {
                    "Request timed out.".into()
                } else {
                    format!("Network request failed: {t}")
                };
                return Err(StealthError::Api {
                    message: redact(&msg, &self.config.gemini_key),
                    status: None,
                });
            }
        };

        let mut reader = response.into_reader().take((RESPONSE_LIMIT + 1) as u64);
        let mut resp_bytes = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut resp_bytes)?;

        if resp_bytes.len() > RESPONSE_LIMIT {
            return Err(StealthError::Api {
                message: "Gemini API response exceeded size limit.".into(),
                status: None,
            });
        }

        let json_val: serde_json::Value =
            serde_json::from_slice(&resp_bytes).map_err(|_| StealthError::Api {
                message: "Gemini API returned malformed response.".into(),
                status: None,
            })?;

        parse_gemini_response(&json_val).map_err(|mut err| {
            if let StealthError::Api {
                ref mut message, ..
            } = err
            {
                *message = redact(message, &self.config.gemini_key);
            }
            err
        })
    }
}

pub fn clean_llm_response(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() >= 2 {
            let inner = lines[1..lines.len() - 1].join("\n");
            return inner.trim().to_string();
        }
    }
    trimmed.to_string()
}

pub fn parse_gemini_error(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    json.get("error")?
        .get("message")?
        .as_str()
        .map(|s| s.to_string())
}

pub fn parse_gemini_response(body: &serde_json::Value) -> Result<String, StealthError> {
    if let Some(err) = body.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Gemini API error");
        let code = err.get("code").and_then(|c| c.as_u64()).map(|c| c as u16);
        return Err(StealthError::Api {
            message: msg.to_string(),
            status: code,
        });
    }

    let candidate = body
        .get("candidates")
        .and_then(|c| c.get(0))
        .ok_or_else(|| StealthError::Api {
            message:
                "No complete answer received from Gemini (blocked, empty, or malformed response)."
                    .into(),
            status: None,
        })?;

    if let Some(finish_reason) = candidate.get("finishReason").and_then(|f| f.as_str()) {
        if finish_reason != "STOP" && finish_reason != "MAX_TOKENS" {
            return Err(StealthError::Api {
                message: format!("Gemini did not complete answer (finishReason: {finish_reason})."),
                status: None,
            });
        }
    }

    let text = candidate
        .get("content")
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| StealthError::Api {
            message: "No text received in Gemini candidate content.".into(),
            status: None,
        })?;

    let cleaned = clean_llm_response(text);
    if cleaned.is_empty() {
        return Err(StealthError::Api {
            message: "Gemini returned empty text.".into(),
            status: None,
        });
    }

    Ok(cleaned)
}
