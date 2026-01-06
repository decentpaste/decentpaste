# AI Agent Prompt: Implement iOS Share Extension for DecentPaste

> **Status: IMPLEMENTED** - This prompt was used to guide the initial implementation. The iOS share extension is now fully working. See `tauri-plugin-decentshare/README.md` for the authoritative documentation.

## Your Mission

You are implementing iOS Share Extension support for DecentPaste, a cross-platform clipboard sharing app built with Tauri v2. Your task is to create the Swift source files and configuration needed for users to share text from any iOS app to DecentPaste.

## Important Context

- **Android share already works** - The `tauri-plugin-decentshare` plugin has a working Android implementation. Your iOS implementation must use the **same API** (`getPendingShare`, `clearPendingShare`) for frontend compatibility.

- **Don't modify gen/apple directly** - The `src-tauri/gen/apple/` directory can be regenerated. All source files must go in `tauri-plugin-decentshare/ios/` which is version controlled.

- **Documentation is key** - Users will need to run the setup script after regeneration. The README must have complete step-by-step instructions.

- **No URL scheme needed** - iOS share extensions cannot reliably open the containing app. The shared content is saved to App Groups and processed when the user manually opens the app.

- **Info.plist gotcha** - When adding ShareExtension files in Xcode, do NOT add `Info.plist` to the project. Xcode generates its own and adding ours causes "Multiple commands produce Info.plist" build errors.

## Detailed Plan

Read the complete implementation plan at: `docs/share-ios-plan.md`

This document contains:
- Architecture overview and data flow diagrams
- Complete Swift code for `DecentsharePlugin.swift` and `ShareViewController.swift`
- `Info.plist` configuration
- Step-by-step Xcode setup guide
- Configuration values and troubleshooting guide

## Files to Create

Create these files in the codebase:

### 1. Tauri iOS Plugin
**Path:** `decentpaste-app/tauri-plugin-decentshare/ios/Sources/DecentsharePlugin.swift`

This is the main plugin that the frontend communicates with. It reads shared content from App Groups UserDefaults.

Key requirements:
- Extend Tauri's `Plugin` class
- Implement `@objc func getPendingShare(_ invoke: Invoke)`
- Implement `@objc func clearPendingShare(_ invoke: Invoke)`
- Use App Group: `group.com.decentpaste.application`
- Use UserDefaults key: `pendingShareContent`
- Export via `@_cdecl("init_plugin_decentshare")` returning `Plugin`
- **Important:** The init function must return the plugin instance (not call registerPlugin)

### 2. Share Extension Controller
**Path:** `decentpaste-app/tauri-plugin-decentshare/ios/ShareExtension/ShareViewController.swift`

This is the extension that appears in the iOS share sheet.

Key requirements:
- Extend `UIViewController`
- Extract text from `extensionContext?.inputItems`
- Handle both `String` and `URL` item types
- Save to App Groups UserDefaults
- Show confirmation card: "Content Saved!" with "Done" button
- Wait for user to tap "Done" to dismiss (no auto-dismiss)
- Call `extensionContext?.completeRequest()` on dismiss

### 3. Extension Info.plist
**Path:** `decentpaste-app/tauri-plugin-decentshare/ios/ShareExtension/Info.plist`

Key configurations:
- `NSExtensionPointIdentifier`: `com.apple.share-services`
- `NSExtensionPrincipalClass`: `$(PRODUCT_MODULE_NAME).ShareViewController`
- `NSExtensionActivationSupportsText`: `true`

## Files to Modify

### 4. Update Plugin README
**Path:** `decentpaste-app/tauri-plugin-decentshare/README.md`

Add comprehensive iOS section with:
- Platform support table
- Complete Xcode setup guide
- Troubleshooting section

## Verification Steps

After implementation, verify:

1. **Files exist:**
   - `tauri-plugin-decentshare/ios/Sources/DecentsharePlugin.swift`
   - `tauri-plugin-decentshare/ios/ShareExtension/ShareViewController.swift`
   - `tauri-plugin-decentshare/ios/ShareExtension/Info.plist`

2. **Setup script exists:**
   - `scripts/setup-ios-share-extension.sh`

3. **API compatibility:**
   - `getPendingShare()` returns `{ content: string | null, hasPending: boolean }`
   - Same format as Android implementation

4. **Documentation complete:**
   - README.md has iOS section with full setup guide

## Key Configuration Values

| Setting | Value |
|---------|-------|
| App Group | `group.com.decentpaste.application` |
| Extension Bundle ID | `com.decentpaste.application.ShareExtension` |
| UserDefaults Key | `pendingShareContent` |

## Reference Files

Before implementing, read these existing files to understand patterns:

1. **Android plugin** (reference for API):
   `decentpaste-app/tauri-plugin-decentshare/android/src/main/java/DecentsharePlugin.kt`

2. **Rust mobile bridge**:
   `decentpaste-app/tauri-plugin-decentshare/src/mobile.rs`

3. **Frontend share handling**:
   `decentpaste-app/src/main.ts` (search for `checkForPendingShare`)

4. **Current plugin README**:
   `decentpaste-app/tauri-plugin-decentshare/README.md`

## Success Criteria

The implementation is complete when:

1. All Swift source files are created in the correct locations
2. Setup script automates Xcode project configuration
3. README.md contains complete iOS setup documentation
4. The code compiles (user will verify in Xcode after running setup script)
5. API matches Android implementation for frontend compatibility

## Notes

- The user will handle Xcode configuration manually (adding target, App Groups, signing)
- Focus on creating correct, well-documented source files
- Include comprehensive comments in Swift code
- The detailed plan in `docs/share-ios-plan.md` has the complete Swift code - use it as reference
