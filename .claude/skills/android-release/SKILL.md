---
name: android-release
description: Build, sign, and prepare Android APK and AAB for release. Handles version bumping, building, zipalign (APK), and signing with JKS keystore. Outputs DecentPaste_x.x.x.apk for GitHub releases and signed AAB for Play Console.
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, AskUserQuestion
---

# Android Release

Builds signed APK (GitHub) and AAB (Play Console).

## Gather Info

Ask user for:
1. Version (x.x.x format)
2. JKS keystore path
3. Keystore alias

**Do NOT ask for password** - user will enter it when running signing commands.

## Update Versions

Same as bump-version skill - update all 4 files:
- `decentpaste-app/package.json`
- `decentpaste-app/src-tauri/Cargo.toml`
- `decentpaste-app/src-tauri/tauri.conf.json`
- `website/downloads.json`

## Build

```bash
cd decentpaste-app && yarn tauri android build && cd ..
```

Output paths (from project root):
- APK: `decentpaste-app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk`
- AAB: `decentpaste-app/src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab`

## Sign APK

**IMPORTANT**: Zipalign BEFORE signing. Run zipalign yourself, then provide commands for user to execute manually.

Zipalign (Claude runs this):
```bash
zipalign -v 4 <unsigned-apk-path> DecentPaste_VERSION_aligned.apk
```

APK signing (provide command, ask user to run it - contains password):
```bash
apksigner sign --ks <keystore> --ks-key-alias <alias> --out DecentPaste_VERSION.apk DecentPaste_VERSION_aligned.apk
```

After user confirms signing complete, clean up aligned file.

## Sign AAB

**IMPORTANT**: AABs must use `jarsigner` (not `apksigner`). AABs are JAR-like bundles without AndroidManifest.xml at root level.

No zipalign needed. Provide commands for user to run manually:
```bash
jarsigner -verbose -sigalg SHA256withRSA -digestalg SHA-256 -keystore <keystore> <aab-path> <alias>
```

Then copy to final location:
```bash
cp <aab-path> DecentPaste_VERSION.aab
```

## Verify & Report

After user confirms both signed, run verification:
```bash
apksigner verify --verbose DecentPaste_VERSION.apk
jarsigner -verify -verbose DecentPaste_VERSION.aab
```

Report: files created, versions updated, next steps (commit, tag, upload).
