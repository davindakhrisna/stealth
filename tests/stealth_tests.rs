use serde_json::json;
use stealth::capture::png_dimensions;
use stealth::config::{
    parse_env, set_effort, set_model, validate_base_url, validate_effort, validate_model,
};
use stealth::engine::{clean_llm_response, parse_gemini_error, parse_gemini_response};
use stealth::error::redact;
use stealth::notify::escape_pango;

#[test]
fn test_parse_env_basic_and_comments() {
    let env_text = r#"
# Comment line
AGY_MODEL=gemini-3.8-flash-low
AGY_EFFORT=low
GEMINI_API_KEY=my-secret-key # inline comment
"#;
    let values = parse_env(env_text).unwrap();
    assert_eq!(values.get("AGY_MODEL").unwrap(), "gemini-3.8-flash-low");
    assert_eq!(values.get("AGY_EFFORT").unwrap(), "low");
    assert_eq!(values.get("GEMINI_API_KEY").unwrap(), "my-secret-key");
}

#[test]
fn test_parse_env_unquoted_spaces_fails() {
    let env_text = "KEY=unquoted value with spaces\n";
    let res = parse_env(env_text);
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Quote values containing spaces"));
}

#[test]
fn test_validate_model() {
    assert!(validate_model("gemini-3.8-flash-low").is_ok());
    assert!(validate_model("gemini-4-argon-medium").is_ok());
    assert!(validate_model("claude-sonnet-5-5-high").is_ok());
    assert!(validate_model("openai/gpt-4o").is_ok());
    assert!(validate_model("").is_err());
    assert!(validate_model("invalid model!").is_err());
}

#[test]
fn test_validate_effort() {
    assert_eq!(validate_effort("low").unwrap(), "low");
    assert_eq!(validate_effort("Medium").unwrap(), "medium");
    assert_eq!(validate_effort("HIGH").unwrap(), "high");
    assert_eq!(validate_effort("xhigh").unwrap(), "xhigh");
    assert_eq!(validate_effort("max").unwrap(), "max");
    assert!(validate_effort("ultra").is_err());
    assert!(validate_effort("").is_err());
}

#[test]
fn test_validate_base_url() {
    assert_eq!(
        validate_base_url("https://generativelanguage.googleapis.com/").unwrap(),
        "https://generativelanguage.googleapis.com"
    );
    assert_eq!(
        validate_base_url("http://127.0.0.1:8080").unwrap(),
        "http://127.0.0.1:8080"
    );
    assert!(validate_base_url("ftp://example.com").is_err());
    assert!(validate_base_url("http://user:pass@example.com").is_err());
    assert!(validate_base_url("http://example.com:0").is_err());
    assert!(validate_base_url("not a url").is_err());
}

#[test]
fn test_set_model_and_effort_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let env_file = dir.path().join(".env");
    std::fs::write(&env_file, "FOO=bar\nAGY_MODEL=OldModel\nAGY_EFFORT=low\n").unwrap();

    let path = set_model("gemini-4-argon-high", Some(&env_file)).unwrap();
    assert_eq!(path, env_file);

    set_effort("high", Some(&env_file)).unwrap();

    let content = std::fs::read_to_string(&env_file).unwrap();
    assert!(content.contains("FOO=bar"));
    assert!(content.contains("AGY_MODEL=gemini-4-argon-high"));
    assert!(!content.contains("AGY_MODEL=OldModel"));
    assert!(content.contains("AGY_EFFORT=high"));
}

#[test]
fn test_clean_llm_response() {
    assert_eq!(clean_llm_response("  Option B  "), "Option B");
    assert_eq!(clean_llm_response("```markdown\nOption C\n```"), "Option C");
    assert_eq!(clean_llm_response("```\n42\n```"), "42");
}

#[test]
fn test_redact_key() {
    let key = "sk-secret-12345";
    let text = "Failed to connect using key sk-secret-12345 in URL";
    let cleaned = redact(text, key);
    assert_eq!(cleaned, "Failed to connect using key [redacted] in URL");
    assert_eq!(redact(text, ""), text);
}

#[test]
fn test_png_dimensions_valid() {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&100u32.to_be_bytes());
    png.extend_from_slice(&200u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0]);
    png.extend_from_slice(&[0, 0, 0, 0]);
    let (w, h) = png_dimensions(&png).unwrap();
    assert_eq!(w, 100);
    assert_eq!(h, 200);
}

#[test]
fn test_png_dimensions_invalid() {
    assert!(png_dimensions(b"not a png").is_err());
    let mut truncated = b"\x89PNG\r\n\x1a\n".to_vec();
    truncated.extend_from_slice(b"short");
    assert!(png_dimensions(&truncated).is_err());
}

#[test]
fn test_escape_pango() {
    assert_eq!(escape_pango("x < 5 & y > 10"), "x &lt; 5 &amp; y &gt; 10");
}

#[test]
fn test_parse_gemini_response() {
    let valid = json!({
        "candidates": [
            {
                "finishReason": "STOP",
                "content": {
                    "parts": [
                        { "text": "Option B" }
                    ]
                }
            }
        ]
    });
    assert_eq!(parse_gemini_response(&valid).unwrap(), "Option B");

    let err_json = json!({
        "error": {
            "code": 400,
            "message": "API key invalid"
        }
    });
    assert!(parse_gemini_response(&err_json).is_err());
    assert_eq!(
        parse_gemini_error(&serde_json::to_string(&err_json).unwrap()).unwrap(),
        "API key invalid"
    );

    let safety = json!({
        "candidates": [
            {
                "finishReason": "SAFETY",
                "content": {
                    "parts": [
                        { "text": "Flagged" }
                    ]
                }
            }
        ]
    });
    assert!(parse_gemini_response(&safety).is_err());
}
