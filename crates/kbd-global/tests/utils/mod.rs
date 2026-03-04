use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use kbd_global::HotkeyManager;
use kbd_global::HotkeyManagerBuilder;

pub fn test_builder() -> HotkeyManagerBuilder {
    HotkeyManager::builder().with_input_directory_for_testing(test_input_directory())
}

pub fn test_manager() -> HotkeyManager {
    test_builder().build().expect("manager should initialize")
}

fn test_input_directory() -> &'static Path {
    static INPUT_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
    INPUT_DIRECTORY
        .get_or_init(|| {
            let path = std::env::temp_dir().join("kbd-global-noinput-tests");
            std::fs::create_dir_all(&path)
                .expect("test input directory should be created successfully");
            path
        })
        .as_path()
}
