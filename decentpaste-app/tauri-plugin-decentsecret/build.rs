const COMMANDS: &[&str] = &[
    "check_availability",
    "store_secret",
    "retrieve_secret",
    "delete_secret",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
