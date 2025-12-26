const COMMANDS: &[&str] = &["get_pending_content", "has_pending_content", "clear_pending_content"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
