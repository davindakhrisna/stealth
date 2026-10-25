use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use crate::error::StealthError;

pub fn escape_pango(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn notify(text: &str) -> Result<(), StealthError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(StealthError::Notification(
            "Cannot send an empty notification.".into(),
        ));
    }

    let body = format!("<span size='8pt'>{}</span>", escape_pango(trimmed));
    let status = Command::new("notify-send")
        .args([
            "-a",
            "Stealth",
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
        .map_err(|e| StealthError::Notification(format!("Failed to execute notify-send: {e}")))?;

    if !status.success() {
        return Err(StealthError::Notification(format!(
            "notify-send failed (exit code {:?})",
            status.code()
        )));
    }
    Ok(())
}

pub fn clear() -> Result<(), StealthError> {
    let status = Command::new("dunstctl")
        .arg("close-all")
        .status()
        .map_err(|e| StealthError::Notification(format!("Failed to execute dunstctl: {e}")))?;

    if !status.success() {
        return Err(StealthError::Notification(format!(
            "dunstctl failed (exit code {:?})",
            status.code()
        )));
    }
    println!("Notifications cleared.");
    Ok(())
}

pub fn simulate() -> Result<(), StealthError> {
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
