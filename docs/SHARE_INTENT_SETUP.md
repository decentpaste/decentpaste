# Share Intent Setup Guide

This document outlines the steps needed to finalize the Share Intent feature after the plugin has been created.

## Overview

The Share Intent feature allows users to share text from any app directly to DecentPaste via the OS share menu. The plugin code is already in place at `decentpaste-app/src-tauri/plugins/tauri-plugin-share-intent/`.

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
  --es android.intent.extra.TEXT "This is a longer test message that should be shared across all paired devices." \
  -n com.decentpaste.application/.MainActivity
```

### Manual Testing

1. Install the app on an Android device
2. Open Chrome/Firefox and navigate to any page
3. Select text and tap "Share"
4. Choose "DecentPaste" from the share menu
5. Verify:
   - App opens (or comes to foreground)
   - Toast shows "Content shared with X devices" (if vault unlocked)
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
3. Create a new App Group with identifier: `group.com.decentpaste.application`
4. Enable this App Group for both:
   - Main app identifier (`com.decentpaste.application`)
   - Share Extension identifier (`com.decentpaste.application.share`)

### Step 2: Add Share Extension Target in Xcode

1. Open the iOS project in Xcode:
   ```bash
   cd decentpaste-app/src-tauri/gen/apple
   open DecentPaste.xcodeproj
   ```

2. Add a new target:
   - **File** → **New** → **Target**
   - Choose **Share Extension**
   - Name: `ShareExtension`
   - Bundle Identifier: `com.decentpaste.application.share`

3. Copy the Share Extension files from the plugin:
   ```bash
   # From the plugin directory
   cp src-tauri/plugins/tauri-plugin-share-intent/ios/ShareExtension/* \
      src-tauri/gen/apple/ShareExtension/
   ```

4. In Xcode, add the copied files to the ShareExtension target:
   - `ShareViewController.swift`
   - `Info.plist`
   - `ShareExtension.entitlements`

### Step 3: Configure Entitlements

#### Main App Entitlements

Add to the main app's entitlements file (create if needed):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
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

The `ShareExtension.entitlements` file should already contain:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.application-groups</key>
    <array>
        <string>group.com.decentpaste.application</string>
    </array>
</dict>
</plist>
```

### Step 4: Add URL Scheme to Main App

Add to the main app's `Info.plist`:

```xml
<key>CFBundleURLTypes</key>
<array>
    <dict>
        <key>CFBundleURLSchemes</key>
        <array>
            <string>decentpaste</string>
        </array>
        <key>CFBundleURLName</key>
        <string>com.decentpaste.application</string>
    </dict>
</array>
```

### Step 5: Configure Signing

1. In Xcode, select the **ShareExtension** target
2. Go to **Signing & Capabilities**
3. Select your Development Team
4. Ensure **App Groups** capability is added with `group.com.decentpaste.application`

### Step 6: Build & Test

```bash
# Build iOS app (requires macOS with Xcode)
cd decentpaste-app
yarn tauri ios build

# Or for development
yarn tauri ios dev
```

### iOS Testing

1. Install app via Xcode (must be signed with proper entitlements)
2. Open Notes app, type some text, select it
3. Tap Share, choose "Share to DecentPaste"
4. Verify:
   - Extension UI appears
   - App opens after sharing
   - Content is shared to paired devices

---

## How It Works

### Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                          OTHER APP                               │
│                     (User selects text)                          │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                        OS SHARE MENU                             │
│                  (User selects DecentPaste)                      │
└─────────────────────────────────────────────────────────────────┘
                                │
         ┌──────────────────────┴──────────────────────┐
         ▼                                             ▼
┌─────────────────────┐                  ┌─────────────────────────┐
│      ANDROID        │                  │          iOS            │
│  ShareIntentPlugin  │                  │   Share Extension +     │
│     (Kotlin)        │                  │   ShareIntentPlugin     │
└─────────────────────┘                  └─────────────────────────┘
         │                                             │
         │ onNewIntent / onCreate                      │ App Groups
         │ ACTION_SEND                                 │ UserDefaults
         ▼                                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PLUGIN RUST CORE                            │
│                  tauri-plugin-share-intent                       │
│                                                                  │
│   • Emits "share-intent-received" event to frontend              │
│   • Provides get_pending_content command                         │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                     FRONTEND (TypeScript)                        │
│                        main.ts / app.ts                          │
│                                                                  │
│   listen('share-intent-received') → handleShareIntent(content)  │
└─────────────────────────────────────────────────────────────────┘
                                │
                 ┌──────────────┴──────────────┐
                 ▼                             ▼
       ┌─────────────────┐           ┌─────────────────┐
       │  VAULT LOCKED   │           │ VAULT UNLOCKED  │
       └─────────────────┘           └─────────────────┘
                 │                             │
                 ▼                             ▼
       Store pending content          Reconnect peers
       Show unlock screen                     ▼
       ─────────────────►             Call share_clipboard_content
       After unlock, process                  ▼
                                      Show success toast
                                      "Content shared!"
```

### Two Launch Scenarios

**Cold Start** (app not running):
1. Android: `onCreate()` receives intent with `ACTION_SEND`
2. iOS: Extension writes to App Groups, main app reads on launch
3. Plugin emits event after WebView loads
4. Frontend handles event

**Warm Start** (app in background):
1. Android: `onNewIntent()` receives intent
2. iOS: Main app checks App Groups on `visibilitychange`
3. Plugin emits event immediately
4. Frontend handles event

---

## Edge Cases

| Scenario | Expected Behavior |
|----------|-------------------|
| Vault not setup | Store content, show onboarding, share after setup |
| Vault locked | Store content, show unlock screen, share after unlock |
| No paired peers | Show error: "No paired devices to share with" |
| Content > 1MB | Show error: "Content too large (max 1MB)" |
| Network disconnected | Attempt reconnect, retry, show error if fails |
| Rapid multiple shares | Only process the latest content |

---

## Troubleshooting

### Android: Share menu doesn't show DecentPaste

- Rebuild the app completely: `yarn tauri android build`
- Check logcat for errors: `adb logcat | grep -i decentpaste`
- Verify the manifest merger ran successfully

### Android: Content not received

- Check logcat: `adb logcat | grep ShareIntent`
- Verify `onNewIntent` is being called
- Check that intent action is not being cleared prematurely

### iOS: Extension doesn't appear

- Verify extension target is properly added to project
- Check that extension bundle ID matches: `com.decentpaste.application.share`
- Verify signing with correct team/provisioning profile
- Check that `NSExtensionActivationSupportsText` is `true` in Info.plist

### iOS: Content not reaching main app

- Verify App Groups are configured on **both** targets
- Check that group identifier matches exactly: `group.com.decentpaste.application`
- Add logging to extension: check Xcode console for `[ShareExtension]` messages
- Add logging to main app: check for `[ShareIntentPlugin]` messages

### Both: Vault locked handling not working

- Ensure content is stored before showing lock screen
- Verify `checkPendingShareContent()` is called after unlock
- Check console for "Processing pending share content" log

---

## Files Reference

### Plugin Location
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
│       └── kotlin/com/decentpaste/plugins/shareintent/
│           └── ShareIntentPlugin.kt
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

### Frontend Integration
- `src/main.ts` - Event listener and handlers
- `src/app.ts` - Vault unlock hook
- `src/api/commands.ts` - Command wrappers
