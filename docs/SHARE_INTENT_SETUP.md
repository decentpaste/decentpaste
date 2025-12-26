# Share Intent Setup Guide

This document outlines the steps needed to build and test the Share Intent feature.

## Overview

The Share Intent feature allows users to share text from any app directly to DecentPaste via the OS share menu. The plugin code is at `decentpaste-app/src-tauri/plugins/tauri-plugin-share-intent/`.

---

## Android Setup

### Automatic Configuration

The Android implementation is **fully automatic**. The plugin's `AndroidManifest.xml` contains an intent filter that gets merged with the app's manifest at build time:

```xml
<intent-filter>
    <action android:name="android.intent.action.SEND" />
    <category android:name="android.intent.category.DEFAULT" />
    <data android:mimeType="text/plain" />
</intent-filter>
```

### Build & Test

```bash
# Build Android APK
cd decentpaste-app
yarn tauri android build

# Or for development
yarn tauri android dev
```

### Testing with ADB

```bash
# Test cold start (app not running)
adb shell am start -a android.intent.action.SEND -t "text/plain" \
  --es android.intent.extra.TEXT "Hello from share test" \
  -n com.decentpaste.application/.MainActivity

# Test with longer text
adb shell am start -a android.intent.action.SEND -t "text/plain" \
  --es android.intent.extra.TEXT "This is a longer test message." \
  -n com.decentpaste.application/.MainActivity
```

### Manual Testing

1. Install the app on an Android device
2. Open Chrome/Firefox and navigate to any page
3. Select text and tap "Share"
4. Choose "DecentPaste" from the share menu
5. Verify:
   - App opens (or comes to foreground)
   - Toast shows "Content shared with your devices!" (if vault unlocked and peers available)
   - Toast shows "Unlock to share content" (if vault locked)
   - After unlock, content is automatically shared

---

## iOS Setup (Manual Steps Required)

iOS requires additional manual configuration in Xcode due to:
- App Groups for inter-process communication
- Share Extension as a separate target
- Entitlements and signing

### Step 1: Create App Group

1. Go to [Apple Developer Portal](https://developer.apple.com/account/resources/identifiers/list/applicationGroup)
2. Click **Identifiers** → **App Groups** → **+**
3. Create: `group.com.decentpaste.application`
4. Enable for both:
   - Main app: `com.decentpaste.application`
   - Share Extension: `com.decentpaste.application.share`

### Step 2: Add Share Extension Target in Xcode

1. Open iOS project:
   ```bash
   cd decentpaste-app/src-tauri/gen/apple
   open DecentPaste.xcodeproj
   ```

2. Add target: **File** → **New** → **Target** → **Share Extension**
   - Name: `ShareExtension`
   - Bundle ID: `com.decentpaste.application.share`

3. Copy extension files from plugin:
   ```bash
   cp src-tauri/plugins/tauri-plugin-share-intent/ios/ShareExtension/* \
      src-tauri/gen/apple/ShareExtension/
   ```

4. Add copied files to ShareExtension target in Xcode:
   - `ShareViewController.swift`
   - `Info.plist`
   - `ShareExtension.entitlements`

### Step 3: Configure Entitlements

#### Main App Entitlements

Add to main app's entitlements file:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "...">
<plist version="1.0">
<dict>
    <key>com.apple.security.application-groups</key>
    <array>
        <string>group.com.decentpaste.application</string>
    </array>
</dict>
</plist>
```

#### Share Extension Entitlements

The `ShareExtension.entitlements` from the plugin already contains:

```xml
<key>com.apple.security.application-groups</key>
<array>
    <string>group.com.decentpaste.application</string>
</array>
```

### Step 4: Configure Signing

1. Select **ShareExtension** target in Xcode
2. Go to **Signing & Capabilities**
3. Select your Development Team
4. Add **App Groups** capability with `group.com.decentpaste.application`

### Step 5: Build & Test

```bash
cd decentpaste-app
yarn tauri ios build

# Or for development
yarn tauri ios dev
```

### iOS Testing

1. Install app via Xcode
2. Open Notes app, type text, select it
3. Tap Share → "Share to DecentPaste"
4. Verify content is shared to paired devices

---

## Troubleshooting

### Android: Share menu doesn't show DecentPaste

- Rebuild completely: `yarn tauri android build`
- Check logcat: `adb logcat | grep -i decentpaste`
- Verify manifest merger ran successfully

### Android: Content not received

- Check logcat: `adb logcat | grep ShareIntent`
- Verify `onNewIntent` is being called
- Check that intent action is not cleared prematurely

### iOS: Extension doesn't appear

- Verify extension target in Xcode project
- Check bundle ID: `com.decentpaste.application.share`
- Verify signing with correct team/profile
- Check `NSExtensionActivationSupportsText` is `true` in Info.plist

### iOS: Content not reaching main app

- Verify App Groups on **both** targets
- Check group ID matches: `group.com.decentpaste.application`
- Check Xcode console for `[ShareExtension]` logs
- Check for `[ShareIntentPlugin]` logs in main app

### Vault locked handling not working

- Ensure content is stored before lock screen shows
- Verify `checkPendingShareContent()` is called after unlock
- Check console for "Processing pending share content" log

---

## Files Reference

```
decentpaste-app/src-tauri/plugins/tauri-plugin-share-intent/
├── Cargo.toml
├── build.rs
├── src/
│   ├── lib.rs
│   ├── commands.rs
│   └── error.rs
├── android/
│   ├── build.gradle.kts
│   ├── proguard-rules.pro
│   └── src/main/
│       ├── AndroidManifest.xml
│       └── kotlin/.../ShareIntentPlugin.kt
├── ios/
│   ├── Package.swift
│   ├── Sources/ShareIntentPlugin/
│   │   └── ShareIntentPlugin.swift
│   └── ShareExtension/
│       ├── Info.plist
│       ├── ShareExtension.entitlements
│       └── ShareViewController.swift
└── permissions/
    └── default.toml
```

### Frontend Integration Points

- `src/main.ts` - Event listener and share intent handlers
- `src/app.ts` - Vault unlock hook for pending content
- `src/api/commands.ts` - Command wrappers (optional fallback)

---

## Build System Notes

### Plugin Cargo.toml Requirements

The `links` field in `Cargo.toml` is **required** for Tauri's permission system to work:

```toml
[package]
name = "tauri-plugin-share-intent"
links = "tauri-plugin-share-intent"  # Required for permissions
```

### iOS Swift Package Integration

The plugin uses a **Swift Package** approach for iOS (not swift-rs). Key points:

1. **build.rs** should NOT call `.ios_path("ios")` - that triggers swift-rs compilation
2. The Swift Package (`ios/Package.swift`) is handled by Xcode
3. Plugin registration uses `@_cdecl("init_plugin_share_intent")` in Swift
4. This approach is more robust for plugins with complex iOS requirements

```rust
// build.rs - correct configuration
fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        // Note: No .ios_path() - Swift Package is handled by Xcode
        .build();
}
```

### Android Manifest Merging

The plugin's `AndroidManifest.xml` uses `activity-alias` which automatically merges with the app's manifest. The `${applicationId}` placeholder resolves to the app's package name at build time.

---

## See Also

- [SHARE_INTENT_IMPLEMENTATION.md](./SHARE_INTENT_IMPLEMENTATION.md) - Architecture and design decisions
