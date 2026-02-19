use crate::error::Error;
use std::process::Command;

pub fn check_input_group() -> Result<bool, Error> {
    let output = Command::new("groups")
        .output()
        .map_err(|e| Error::PermissionDenied(format!("Failed to run groups command: {}", e)))?;

    if !output.status.success() {
        return Err(Error::PermissionDenied(
            "groups command returned non-zero exit".into(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let is_in_group = stdout.split_whitespace().any(|group| group == "input");

    Ok(is_in_group)
}

pub fn get_permission_error_message() -> String {
    let username = std::env::var("USER").unwrap_or_else(|_| "<username>".into());

    format!(
        "evdev requires input group permissions. Please run:\n\
         sudo usermod -aG input {}\n\
         Then log out and log back in for changes to take effect.",
        username
    )
}
