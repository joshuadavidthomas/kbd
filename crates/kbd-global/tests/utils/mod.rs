use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use kbd_global::HotkeyManager;
use kbd_global::HotkeyManagerBuilder;

pub fn test_builder() -> HotkeyManagerBuilder {
    initialize_test_input_directory_override();
    HotkeyManager::builder()
}

fn initialize_test_input_directory_override() {
    static INITIALIZED: OnceLock<()> = OnceLock::new();
    INITIALIZED.get_or_init(|| {
        // SAFETY: this runs once before constructing any HotkeyManager in
        // these integration tests and always sets the same deterministic value.
        unsafe {
            std::env::set_var(
                "_KBD_GLOBAL_INTERNAL_TEST_INPUT_DIR",
                test_input_directory(),
            );
        }
    });
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
