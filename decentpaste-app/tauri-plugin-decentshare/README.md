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
3. Extension shows confirmation card, user taps "Done" to dismiss
4. User opens DecentPaste manually → app reads via `getPendingShare()`

> **Note:** iOS share extensions cannot reliably open the containing app due to sandbox restrictions.
> The shared content is saved to App Groups and will be processed when the user opens DecentPaste.

Files:

- `ios/Sources/DecentsharePlugin.swift` - Tauri plugin
- `ios/ShareExtension/ShareViewController.swift` - Extension controller
- `ios/ShareExtension/Info.plist` - Extension configuration

---

## iOS Setup Guide

iOS requires additional configuration because Share Extensions are separate app targets. We provide an **automated setup script** that handles most of the work.

### Prerequisites

1. **Apple Developer Account** with ability to create App Groups
2. **Xcode 14.0+** installed
3. **xcodegen** installed: `brew install xcodegen`

### One-Time Setup: Create App Group

Before first build, create the App Group in Apple Developer Portal:

1. Go to [Apple Developer Portal - Identifiers](https://developer.apple.com/account/resources/identifiers/list/applicationGroup)
2. Click **+** to register a new identifier
3. Select **App Groups** → Continue
4. Enter description: `DecentPaste Shared Data`
5. Enter identifier: `group.<your-app-identifier>` (e.g., `group.com.decentpaste.application`)
6. Click **Continue** → **Register**

> **Note:** The App Group identifier is derived from your `identifier` in `tauri.conf.json` with a `group.` prefix.

### Setup After `yarn tauri ios init`

Run these commands whenever you initialize or regenerate the iOS project:

```bash
# 1. Initialize iOS project (if not already done)
yarn tauri ios init

# 2. Run the Share Extension setup script
./tauri-plugin-decentshare/scripts/setup-ios-share-extension.sh

# 3. Open Xcode
open src-tauri/gen/apple/decentpaste-app.xcodeproj
```

### Configure Code Signing in Xcode

The setup script handles everything except code signing (which requires your Apple Developer account):

1. In Xcode, select target **decentpaste-app_iOS**
2. Go to **Signing & Capabilities** tab
3. Set **Team** to your development team
4. Select target **ShareExtension**
5. Go to **Signing & Capabilities** tab
6. Set **Team** to the **same** development team

### Build and Test

1. Connect a **physical iOS device** (Share Extensions don't work reliably in Simulator)
2. Select scheme: **decentpaste-app_iOS**
3. Select your device as destination
4. Build and run: **Cmd+R**

**Testing the Share Extension:**

1. Open Safari on the device
2. Navigate to any webpage
3. Select some text → Tap **Share**
4. Look for **DecentPaste** in the share sheet (scroll right or tap "More" if needed)
5. Tap DecentPaste → Should show confirmation card with "Content Saved!"
6. Tap **Done** to dismiss the extension
7. Open DecentPaste → shared content should sync to paired devices

### What the Setup Script Does

The `setup-ios-share-extension.sh` script reads configuration from `tauri.conf.json` and automates:

- ✅ Reads `version` and `identifier` from `tauri.conf.json`
- ✅ Derives extension bundle ID (`<identifier>.ShareExtension`) and App Group (`group.<identifier>`)
- ✅ Creates ShareExtension directory in `gen/apple/`
- ✅ Copies `ShareViewController.swift` and `Info.plist` from plugin source
- ✅ Creates entitlements files with App Groups for both targets
- ✅ Adds ShareExtension target to `project.yml`
- ✅ Embeds ShareExtension in main app
- ✅ Runs `xcodegen` to regenerate the Xcode project
- ✅ Restores correct files after xcodegen (Info.plist and entitlements)

**You only need to manually:**

- Configure code signing team (once per Xcode project open)
- Build and run

---

## Configuration Values

These values are derived from `tauri.conf.json`:

| Setting             | Derivation                        | Example (DecentPaste)                        |
|---------------------|-----------------------------------|----------------------------------------------|
| Main App Bundle ID  | `identifier` from tauri.conf.json | `com.decentpaste.application`                |
| Extension Bundle ID | `<identifier>.ShareExtension`     | `com.decentpaste.application.ShareExtension` |
| App Group           | `group.<identifier>`              | `group.com.decentpaste.application`          |
| App Version         | `version` from tauri.conf.json    | `0.4.2`                                      |

**Hardcoded values** (in Swift source files):

| Setting          | Value                 | Location                                               |
|------------------|-----------------------|--------------------------------------------------------|
| UserDefaults Key | `pendingShareContent` | `DecentsharePlugin.swift`, `ShareViewController.swift` |

---

## Troubleshooting

### "App Group container could not be accessed"

- Verify App Group `group.<your-identifier>` is created in Apple Developer Portal
- Verify both targets have the same Team selected in Signing & Capabilities
- Try refreshing provisioning profiles: Xcode → Preferences → Accounts → Download Manual Profiles

### Share Extension doesn't appear in share sheet

- **Delete the app from device** and reinstall (iOS caches extension registration)
- Verify you built with `decentpaste-app_iOS` scheme (not `ShareExtension`)
- Check device Settings → DecentPaste → Share Extension is enabled
- Try scrolling right in share sheet or tapping "More"

### "does not define an NSExtension dictionary" error

- The Info.plist was overwritten by xcodegen
- **Solution:** Re-run `./tauri-plugin-decentshare/scripts/setup-ios-share-extension.sh` (Step 7 restores the correct Info.plist)

### Extension appears but crashes

- Check Console.app for crash logs (filter by "ShareExtension")
- Verify both targets use the same code signing team
- Clean build folder (Cmd+Option+Shift+K) and rebuild

### Main app doesn't receive shared content

- Verify both targets have App Groups capability with `group.<your-identifier>`
- Check Console.app logs for "DecentsharePlugin" messages
- Verify frontend is calling `checkForPendingShare()`

### Build errors after regenerating gen/apple/

- This is expected - `yarn tauri ios init` regenerates the project from scratch
- **Solution:** Run `./tauri-plugin-decentshare/scripts/setup-ios-share-extension.sh` after every regeneration
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
decentpaste-app/
└── tauri-plugin-decentshare/
    ├── scripts/
    │   └── setup-ios-share-extension.sh  # iOS setup automation script
    ├── android/                          # Android implementation
    │   ├── src/main/java/DecentsharePlugin.kt
    │   └── src/main/AndroidManifest.xml
    ├── ios/                              # iOS implementation (source of truth)
    │   ├── Package.swift                # Swift package definition
    │   ├── Sources/
    │   │   └── DecentsharePlugin.swift  # Tauri plugin
    │   └── ShareExtension/
    │       ├── ShareViewController.swift # Extension controller
    │       └── Info.plist               # Extension config
    ├── src/                              # Rust plugin code
    │   ├── lib.rs
    │   ├── mobile.rs
    │   └── commands.rs
    └── guest-js/                         # TypeScript bindings
        └── index.ts
```

## License

Apache-2.0
