use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use crate::error::AcademicError;

const DUNST_CONF: &str = "\
[academia]
    appname = \"Academia\"
    background = \"#00000000\"
    foreground = \"#e3e8f2\"
    frame_color = \"#00000000\"
    format = \"%b\"
";

pub fn escape_pango(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn write_dunst_config() -> Option<PathBuf> {
    let path = env::temp_dir().join("academia-dunst.conf");
    fs::write(&path, DUNST_CONF).ok()?;
    fs::canonicalize(path).ok()
}

fn ensure_dunst_config() {
    let Some(conf_path) = write_dunst_config() else {
        return;
    };
    if let Ok(output) = Command::new("dunstctl").args(["rules", "--json"]).output() {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            if out_str.contains("\"appname\" : \"Academia\"")
                || out_str.contains("\"name\" : \"academia\"")
            {
                return;
            }
        }
    }

    let home = env::var("HOME").unwrap_or_else(|_| ".".into());
    let sys_dunstrc = PathBuf::from(&home).join(".config/dunst/dunstrc");
    let mut args = vec!["reload".to_string()];
    if sys_dunstrc.is_file() {
        args.push(sys_dunstrc.to_string_lossy().to_string());
    }
    let dropin_dir = PathBuf::from(&home).join(".config/dunst/dunstrc.d");
    if let Ok(entries) = fs::read_dir(dropin_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|ext| ext == "conf") {
                args.push(p.to_string_lossy().to_string());
            }
        }
    }
    args.push(conf_path.to_string_lossy().to_string());
    let _ = Command::new("dunstctl").args(&args).output();
}

pub fn notify(text: &str) -> Result<(), AcademicError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AcademicError::Notification(
            "Cannot send an empty notification.".into(),
        ));
    }

    ensure_dunst_config();

    let body = format!("<span size='8pt'>{}</span>", escape_pango(trimmed));
    let status = Command::new("notify-send")
        .args([
            "-a",
            "Academia",
            "-u",
            "low",
            "-t",
            "60000",
            "-h",
            "string:bgcolor:#00000000",
            "-h",
            "string:fgcolor:#e3e8f2",
            "-h",
            "string:frcolor:#00000000",
            "--",
            "",
            &body,
        ])
        .status()
        .map_err(|e| AcademicError::Notification(format!("Failed to execute notify-send: {e}")))?;

    if !status.success() {
        return Err(AcademicError::Notification(format!(
            "notify-send failed (exit code {:?})",
            status.code()
        )));
    }
    Ok(())
}

pub fn clear() -> Result<(), AcademicError> {
    let status = Command::new("dunstctl")
        .arg("close-all")
        .status()
        .map_err(|e| AcademicError::Notification(format!("Failed to execute dunstctl: {e}")))?;

    if !status.success() {
        return Err(AcademicError::Notification(format!(
            "dunstctl failed (exit code {:?})",
            status.code()
        )));
    }
    println!("Notifications cleared.");
    Ok(())
}

pub fn simulate() -> Result<(), AcademicError> {
    let samples = [
        "∫ x² dx = x³/3 + C",
        "0 < x < 5",
        "Error: API Quota Exceeded",
        "Error: Timeout Exceeded",
        "Error: Bad Request",
        "Error: Screen Capture Failed",
    ];
    for sample in samples {
        notify(sample)?;
        sleep(Duration::from_secs(2));
    }
    Ok(())
}

pub fn report_error(message: &str) {
    eprintln!("Error: {message}");
    let _ = notify(&format!("Error: {message}"));
}
