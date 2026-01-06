---
name: ios-release
description: Build, archive, and prepare iOS app for TestFlight/App Store. Handles version bumping, Share Extension setup, Xcode archiving, and provides upload instructions. Use this when preparing a new iOS release for TestFlight beta testing or App Store submission.
---

# iOS Release

Build and prepare iOS app for TestFlight/App Store.

## 1. Gather Info

Ask user for: version (x.x.x).
Confirm if this is for TestFlight or App Store submission.

## 2. Update Versions

Use `/bump-version` workflow to update all config files.

## 3. Setup Share Extension

Ensure Share Extension is configured (required after any `yarn tauri ios init`):

```bash
cd decentpaste-app
./tauri-plugin-decentshare/scripts/setup-ios-share-extension.sh
```

## 4. Build

```bash
cd decentpaste-app && yarn tauri ios build --release && cd ..
```

Build output: `src-tauri/gen/apple/build/` directory with compiled app.

## 5. Archive & Upload

**Open Xcode for archiving** (Claude cannot automate Xcode GUI):

```bash
open decentpaste-app/src-tauri/gen/apple/decentpaste-app.xcodeproj
```

**User performs in Xcode:**
1. Select scheme: `decentpaste-app_iOS`
2. Select destination: `Any iOS Device (arm64)`
3. Menu: **Product → Archive** (⇧⌘A)
4. Wait for archive (~5-15 min)
5. In Organizer: **Distribute App → App Store Connect → Upload**
6. Check "Upload your app's symbols"
7. Use automatic signing
8. Click **Upload**

## 6. TestFlight Configuration

After upload completes (~5-15 min processing):

1. Go to [App Store Connect](https://appstoreconnect.apple.com/) → TestFlight
2. Select the new build
3. Fill **Test Information** (what to test)
4. Answer **Export Compliance** (usually "No" for standard encryption)
5. For external testers: Wait for Beta App Review (~24-48h)

## 7. Report

Report: version updated, build completed, next steps (add testers, submit for review).

## Key Files

- ExportOptions.plist: `src-tauri/gen/apple/ExportOptions.plist` (Team ID: SNDGGHFSJ2)
- Bundle ID: `com.decentpaste.application`
- Extension Bundle ID: `com.decentpaste.application.ShareExtension`
- App Group: `group.com.decentpaste.application`

## Troubleshooting

- **Code signing errors**: Open Xcode, verify both targets have Team selected in Signing & Capabilities
- **Share Extension missing**: Re-run `setup-ios-share-extension.sh`
- **Upload fails**: Check Xcode → Preferences → Accounts for valid credentials
