use academic::capture::png_dimensions;
use academic::config::{parse_env, set_model, validate_base_url, validate_model};
use academic::error::redact;
use academic::notify::escape_pango;
use academic::router::extract_complete_text;
use serde_json::json;

#[test]
fn test_parse_env_basic_and_comments() {
    let env_text = r#"
# Comment line
ROUTER_BASE_URL=http://localhost:20128/v1
export ROUTER_MODEL="Academic"
ROUTER_API_KEY=my-secret-key # inline comment
"#;
    let values = parse_env(env_text).unwrap();
    assert_eq!(
        values.get("ROUTER_BASE_URL").unwrap(),
        "http://localhost:20128/v1"
    );
    assert_eq!(values.get("ROUTER_MODEL").unwrap(), "Academic");
    assert_eq!(values.get("ROUTER_API_KEY").unwrap(), "my-secret-key");
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
    assert!(validate_model("Academic").is_ok());
    assert!(validate_model("openai/gpt-4o").is_ok());
    assert!(validate_model("provider/model:free").is_ok());
    assert!(validate_model("").is_err());
    assert!(validate_model("invalid model!").is_err());
}

#[test]
fn test_validate_base_url() {
    assert_eq!(
        validate_base_url("http://127.0.0.1:20128/v1/").unwrap(),
        "http://127.0.0.1:20128/v1"
    );
    assert_eq!(
        validate_base_url("https://api.example.com").unwrap(),
        "https://api.example.com"
    );
    assert!(validate_base_url("ftp://example.com").is_err());
    assert!(validate_base_url("http://user:pass@example.com").is_err());
    assert!(validate_base_url("http://example.com:0").is_err());
    assert!(validate_base_url("not a url").is_err());
}

#[test]
fn test_set_model_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let env_file = dir.path().join(".env");
    std::fs::write(&env_file, "FOO=bar\nROUTER_MODEL=OldModel\n").unwrap();

    let path = set_model("NewModel", Some(&env_file)).unwrap();
    assert_eq!(path, env_file);

    let content = std::fs::read_to_string(&env_file).unwrap();
    assert!(content.contains("FOO=bar"));
    assert!(content.contains("ROUTER_MODEL=NewModel"));
    assert!(!content.contains("ROUTER_MODEL=OldModel"));
}

#[test]
fn test_redact_key() {
    let key = "sk-secret-12345";
    let text = "Failed to connect using Bearer sk-secret-12345 in header";
    let cleaned = redact(text, key);
    assert_eq!(
        cleaned,
        "Failed to connect using Bearer [redacted] in header"
    );
    assert_eq!(redact(text, ""), text);
}

#[test]
fn test_png_dimensions_valid() {
    // Minimal 1x1 8-bit RGBA PNG
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    // IHDR chunk: len (4 bytes), type (4 bytes), data (13 bytes), crc (4 bytes)
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&100u32.to_be_bytes()); // width = 100
    png.extend_from_slice(&200u32.to_be_bytes()); // height = 200
    png.extend_from_slice(&[8, 6, 0, 0, 0]); // bit depth, colortype, comp, filter, interlace
    png.extend_from_slice(&[0, 0, 0, 0]); // dummy CRC
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
fn test_extract_complete_text() {
    let valid = json!({
        "choices": [
            {
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": "Option B"
                }
            }
        ]
    });
    assert_eq!(extract_complete_text(&valid).unwrap(), "Option B");

    let refusal = json!({
        "choices": [
            {
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "refusal": "Cannot assist"
                }
            }
        ]
    });
    assert!(extract_complete_text(&refusal).is_err());

    let non_stop = json!({
        "choices": [
            {
                "finish_reason": "length",
                "message": {
                    "role": "assistant",
                    "content": "Truncated"
                }
            }
        ]
    });
    assert!(extract_complete_text(&non_stop).is_err());
}
