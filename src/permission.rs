pub(crate) fn get_permission_error_message() -> String {
    let username = std::env::var("USER").unwrap_or_else(|_| "<username>".into());

    format!(
        "The evdev backend requires access to /dev/input/event* devices. If access is denied, try:\n\
         sudo usermod -aG input {username}\n\
         Then log out and log back in for changes to take effect."
    )
}
