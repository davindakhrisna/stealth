use std::process::Command;

use crate::error::StealthError;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub fn png_dimensions(image: &[u8]) -> Result<(u32, u32), StealthError> {
    if image.len() < 33 || &image[0..8] != PNG_SIGNATURE || &image[12..16] != b"IHDR" {
        return Err(StealthError::Capture(
            "Screen capture did not produce a PNG image.".into(),
        ));
    }
    let width = u32::from_be_bytes(image[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(image[20..24].try_into().unwrap());
    if width == 0 || height == 0 {
        return Err(StealthError::Capture(
            "Screen capture has invalid dimensions.".into(),
        ));
    }
    Ok((width, height))
}

pub fn capture_png() -> Result<Vec<u8>, StealthError> {
    let output = Command::new("grim")
        .args(["-t", "png", "-"])
        .output()
        .map_err(|e| {
            StealthError::Capture(format!(
                "Cannot run grim; install it and ensure you are in a Wayland/Hyprland session: {e}"
            ))
        })?;

    if !output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr);
        return Err(StealthError::Capture(format!(
            "grim failed (exit code {:?}): {}",
            output.status.code(),
            msg.trim()
        )));
    }

    let bytes = output.stdout;
    png_dimensions(&bytes)?;
    Ok(bytes)
}
