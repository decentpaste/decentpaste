const COMMANDS: &[&str] = &["get_pending_content", "has_pending_content", "clear_pending_content"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        // Note: iOS uses Swift Package Manager (Package.swift) which is handled by Xcode directly.
        // The Swift plugin registers itself via @_cdecl("init_plugin_share_intent").
        // We don't call .ios_path() here because that triggers swift-rs compilation,
        // which conflicts with the Swift Package approach.
        .build();
}
