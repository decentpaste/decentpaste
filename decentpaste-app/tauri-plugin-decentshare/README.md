# Tauri Plugin decentshare

Cross-platform share intent plugin for DecentPaste. Enables receiving shared text from other apps via the system share sheet.

## Platform Support

| Platform | Status          | Implementation                                  |
|----------|-----------------|-------------------------------------------------|
| Android  | Fully supported | Kotlin plugin with `ACTION_SEND` intent handler |
| iOS      | Fully supported | Swift plugin with Share Extension               |
| Desktop  | N/A             | Desktop platforms don't have share sheets       |

## API

Both platforms expose the same API:

```typescript
import { getPendingShare, clearPendingShare } from 'tauri-plugin-decentshare-api';

// Check for pending shared content
const result = await getPendingShare();
// { content: "shared text" | null, hasPending: boolean }

// Clear pending content (called after processing)
await clearPendingShare();
```

## Architecture

### Android

Android uses a Kotlin plugin that handles `ACTION_SEND` intents:

1. User shares text → Android system shows share sheet
2. User selects DecentPaste → `onNewIntent()` receives intent
3. Plugin stores content in `pendingShareContent`
4. Frontend polls via `getPendingShare()`

Files:
- `android/src/main/java/DecentsharePlugin.kt` - Intent handler
- `android/src/main/AndroidManifest.xml` - Intent filter registration

### iOS

iOS uses a Share Extension (separate target) + App Groups for data passing:

1. User shares text → iOS share sheet shows DecentPaste
2. ShareExtension saves to App Groups UserDefaults
3. Extension opens main app via `decentpaste://share` URL scheme
4. Main app's plugin reads from App Groups via `getPendingShare()`

Files:
- `ios/Sources/DecentsharePlugin.swift` - Tauri plugin
- `ios/ShareExtension/ShareViewController.swift` - Extension controller
- `ios/ShareExtension/Info.plist` - Extension configuration

---

## iOS Setup Guide

iOS requires additional Xcode configuration because Share Extensions are separate app targets. Follow this guide after `yarn tauri ios init` or whenever `gen/apple/` is regenerated.

### Prerequisites

1. **Apple Developer Account** with ability to create App Groups
2. **Xcode** installed (14.0+)
3. iOS project initialized: `yarn tauri ios init`
4. **Tauri deep-link plugin** configured in `tauri.conf.json` (handles URL scheme automatically)

> **Note:** The `tauri-plugin-deep-link` automatically injects `CFBundleURLTypes` into Info.plist during the build phase. You do NOT need to manually add URL schemes - the `decentpaste://` scheme is configured automatically based on your `tauri.conf.json`.

### Step 1: Create App Group in Apple Developer Portal

1. Go to [Apple Developer Portal - Identifiers](https://developer.apple.com/account/resources/identifiers/list/applicationGroup)
2. Click "+" to register a new identifier
3. Select "App Groups" → Continue
4. Enter description: "DecentPaste Shared Data"
5. Enter identifier: `group.com.decentpaste.application`
6. Click Continue → Register

### Step 2: Open Xcode Project

```bash
open src-tauri/gen/apple/decentpaste-app.xcodeproj
```

### Step 3: Add Share Extension Target

1. In Xcode menu: **File → New → Target...**
2. Select iOS tab → **Share Extension** → Next
3. Configure:
   - **Product Name:** `ShareExtension`
   - **Team:** (Your development team)
   - **Organization Identifier:** `com.decentpaste.application`
   - **Bundle Identifier:** `com.decentpaste.application.ShareExtension` (auto-filled)
   - **Language:** Swift
   - **Project:** decentpaste-app
   - **Embed in Application:** decentpaste-app_iOS
4. Click **Finish**
5. When prompted "Activate ShareExtension scheme?", click **Activate**

### Step 4: Replace Generated Swift File

Xcode creates a template `ShareViewController.swift`. Replace it with our implementation:

1. In Project Navigator, expand the `ShareExtension` group
2. **Delete** the auto-generated files:
   - Select `ShareViewController.swift` → Delete → **Move to Trash**
   - Select `MainInterface.storyboard` (if present) → Delete → **Move to Trash**
   - ⚠️ **Do NOT delete `Info.plist`** - Xcode needs this for build settings
3. Right-click the `ShareExtension` group → **Add Files to "decentpaste-app"...**
4. Navigate to: `tauri-plugin-decentshare/ios/ShareExtension/`
5. Select **ONLY** `ShareViewController.swift`
   - ⚠️ **Do NOT add `Info.plist`** - Adding it causes "Multiple commands produce Info.plist" build error
6. Configure:
   - ☑️ Copy items if needed
   - ☑️ Create groups
   - Add to targets: ☑️ **ShareExtension** only
7. Click **Add**

> **Important:** The `Info.plist` must only exist in Xcode's build settings (`INFOPLIST_FILE`), not in "Copy Bundle Resources". Adding it as a file causes duplicate output errors.

### Step 5: Configure App Groups (Both Targets)

**For main app target (decentpaste-app_iOS):**

1. Select the project in Navigator (blue icon at top)
2. Select target: `decentpaste-app_iOS`
3. Select **Signing & Capabilities** tab
4. Click **+ Capability** button
5. Select **App Groups**
6. Under App Groups section, click **+**
7. Select or enter: `group.com.decentpaste.application`

**For ShareExtension target:**

1. Select target: `ShareExtension`
2. Select **Signing & Capabilities** tab
3. Click **+ Capability**
4. Select **App Groups**
5. Click **+** and select the SAME group: `group.com.decentpaste.application`

### Step 6: Configure Code Signing

**For main app target:**

1. Select target: `decentpaste-app_iOS`
2. In "Signing & Capabilities":
   - ☑️ Automatically manage signing
   - Team: (Select your team)
3. Verify no signing errors appear

**For ShareExtension target:**

1. Select target: `ShareExtension`
2. In "Signing & Capabilities":
   - ☑️ Automatically manage signing
   - Team: (Select SAME team as main app)
3. Verify no signing errors appear

### Step 7: Verify Extension Embedding

1. Select target: `decentpaste-app_iOS`
2. Select **General** tab
3. Scroll to **Frameworks, Libraries, and Embedded Content**
4. Verify `ShareExtension.appex` is listed with "Embed & Sign"
   - If missing: Click "+" → Under "Embed App Extensions" select ShareExtension

### Step 8: Build and Test

1. Connect a physical iOS device (Share Extensions don't work reliably in Simulator)
2. Select scheme: `decentpaste-app_iOS`
3. Select your device as destination
4. Build: **Product → Build** (Cmd+B)
5. If build succeeds, run: **Product → Run** (Cmd+R)

**Testing:**

1. Open Safari on the device
2. Navigate to any webpage
3. Select some text → Share button
4. DecentPaste should appear in share sheet
5. Tap DecentPaste → Should show toast → App should open

---

## Configuration Values

| Setting             | Value                                        |
|---------------------|----------------------------------------------|
| App Group           | `group.com.decentpaste.application`          |
| URL Scheme          | `decentpaste`                                |
| Extension Bundle ID | `com.decentpaste.application.ShareExtension` |
| Main App Bundle ID  | `com.decentpaste.application`                |
| UserDefaults Key    | `pendingShareContent`                        |

---

## Troubleshooting

### "App Group container could not be accessed"

- Verify App Group is created in Apple Developer Portal
- Verify SAME App Group ID is added to BOTH targets in Xcode
- Check the identifier matches exactly: `group.com.decentpaste.application`
- Try removing and re-adding the App Groups capability

### Share Extension doesn't appear in share sheet

- Verify extension is embedded in main app (Step 7)
- Verify `NSExtensionActivationRule` in Info.plist accepts text
- Build and run the main app at least once
- Check device Settings → (App Name) → Share Extension is enabled

### Extension appears but crashes

- Check Console.app for crash logs (filter by "ShareExtension")
- Verify Info.plist `NSExtensionPrincipalClass` matches class name
- Ensure all Swift files are added to ShareExtension target (check Target Membership)

### Main app doesn't receive shared content

- Verify App Group IDs match in both Swift files
- Check UserDefaults key matches: `pendingShareContent`
- Verify frontend is calling `checkForPendingShare()`

### URL scheme doesn't open app

- This is expected on some iOS versions (security restriction)
- Content is still saved to App Groups
- User can manually switch to DecentPaste
- App will pick up content on visibility change

### "Multiple commands produce Info.plist" build error

- This happens when `Info.plist` is added to "Copy Bundle Resources" build phase
- **Solution:** In Xcode, select ShareExtension target → Build Phases → Copy Bundle Resources
- Remove `Info.plist` from the list if present (click `-` button)
- Info.plist should only be referenced in Build Settings (`INFOPLIST_FILE`), not copied as a resource

### Build errors after regenerating gen/apple/

- The Share Extension target is lost when regenerating
- Follow this setup guide again from Step 3
- Source files are preserved in `tauri-plugin-decentshare/ios/`

---

## Development

### Rebuilding Plugin JS Bindings

After modifying the TypeScript API:

```bash
cd tauri-plugin-decentshare
yarn build
```

### File Locations

```
tauri-plugin-decentshare/
├── android/                          # Android implementation
│   ├── src/main/java/DecentsharePlugin.kt
│   └── src/main/AndroidManifest.xml
├── ios/                              # iOS implementation (source of truth)
│   ├── Package.swift                # Swift package definition
│   ├── Sources/
│   │   └── DecentsharePlugin.swift  # Tauri plugin
│   └── ShareExtension/
│       ├── ShareViewController.swift # Extension controller
│       └── Info.plist               # Extension config (reference only)
├── src/                              # Rust plugin code
│   ├── lib.rs
│   ├── mobile.rs
│   └── commands.rs
└── guest-js/                         # TypeScript bindings
    └── index.ts
```
