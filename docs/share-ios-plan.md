# iOS Share Extension Implementation Plan

## Executive Summary

This document provides a complete implementation plan for adding iOS Share Extension support to DecentPaste. The goal is to allow users to select text in any iOS app, tap "Share", and send that text to paired DecentPaste devices.

**Current State**: Android share intent already works via `tauri-plugin-decentshare`. iOS implementation is missing.

**Target State**: iOS share extension that mirrors Android functionality with identical API.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Data Flow](#data-flow)
3. [File Structure](#file-structure)
4. [Implementation Details](#implementation-details)
   - [Step 1: Install Tauri Deep-Link Plugin](#step-1-install-tauri-deep-link-plugin)
   - [Step 2: Create DecentsharePlugin.swift](#step-2-create-decentsharePluginswift)
   - [Step 3: Create ShareViewController.swift](#step-3-create-shareviewcontrollerswift)
   - [Step 4: Create Extension Info.plist](#step-4-create-extension-infoplist)
   - [Step 5: Create Helper Script](#step-5-create-helper-script)
   - [Step 6: Update package.json](#step-6-update-packagejson)
   - [Step 7: Update Frontend (Optional)](#step-7-update-frontend-optional)
   - [Step 8: Update Plugin README](#step-8-update-plugin-readme)
5. [Complete Xcode Setup Guide](#complete-xcode-setup-guide)
6. [Configuration Values](#configuration-values)
7. [Testing Checklist](#testing-checklist)
8. [Troubleshooting](#troubleshooting)

---

## Architecture Overview

iOS Share Extensions are fundamentally different from Android's share intents:

| Aspect | Android | iOS |
|--------|---------|-----|
| Process | Same process (MainActivity receives intent) | Separate process (extension runs independently) |
| Data Passing | Intent extras | App Groups (shared UserDefaults) |
| App Opening | Automatic (intent opens app) | URL scheme (must explicitly open) |
| Memory Limit | App's normal limit | ~120MB for extension |

### Key Components

1. **Share Extension** (`ShareExtension/`) - Separate iOS target that appears in share sheet
2. **Tauri Plugin** (`DecentsharePlugin.swift`) - Reads shared data from App Groups
3. **App Groups** - Shared storage container between extension and main app
4. **URL Scheme** (`decentpaste://`) - Opens main app from extension

---

## Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. User selects text in Safari/Notes/any app                    │
│    User taps Share → DecentPaste                                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. ShareExtension process starts (separate from main app)       │
│    ShareViewController.swift:                                   │
│    - viewDidLoad() called                                       │
│    - Extract text from extensionContext?.inputItems             │
│    - Handle NSExtensionItem → NSItemProvider → String/URL       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. Save to App Groups                                           │
│    UserDefaults(suiteName: "group.com.decentpaste.application") │
│    Key: "pendingShareContent"                                   │
│    Value: shared text string                                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. Show toast & attempt to open main app                        │
│    - Display: "Saved! Opening DecentPaste..."                   │
│    - Try: extensionContext?.open(URL("decentpaste://share"))    │
│    - Call: extensionContext?.completeRequest()                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. Main app opens (or user switches manually)                   │
│    - Tauri deep-link plugin handles URL scheme                  │
│    - OR: User manually opens DecentPaste                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 6. Frontend polls for pending share                             │
│    main.ts: checkForPendingShare() called on:                   │
│    - App init (line ~132)                                       │
│    - Visibility change (line ~163)                              │
│    - Deep link received (if onOpenUrl listener added)           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 7. Plugin reads from App Groups                                 │
│    DecentsharePlugin.swift: getPendingShare()                   │
│    - Read from shared UserDefaults                              │
│    - Clear after reading (atomic get-and-clear)                 │
│    - Return: { content: string | null, hasPending: boolean }    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 8. Frontend processes shared content                            │
│    main.ts: handleShareIntent(content)                          │
│    - If vault locked: store in pendingShare state               │
│    - If vault unlocked: call handleSharedContent(content)       │
│      → Encrypt per-peer                                         │
│      → Broadcast via gossipsub                                  │
│      → Add to clipboard history                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## File Structure

### Source Files to Create (in plugin directory)

```
decentpaste-app/tauri-plugin-decentshare/ios/
├── Package.swift                      # Swift package definition (required by Tauri)
├── Sources/
│   └── DecentsharePlugin.swift        # Tauri iOS plugin implementation
├── ShareExtension/
│   ├── ShareViewController.swift      # Extension entry point
│   └── Info.plist                     # Extension configuration
├── scripts/
│   └── copy-files.sh                  # Optional helper script
└── README.md                          # iOS setup documentation (update existing)
```

### Files to Modify (in main app)

```
decentpaste-app/
├── package.json                       # Add ios:copy-files script
└── src-tauri/
    ├── tauri.conf.json               # Add deep-link plugin config
    ├── Cargo.toml                    # Add tauri-plugin-deep-link
    └── src/lib.rs                    # Initialize deep-link plugin
```

### Generated Files (created via Xcode, NOT version controlled)

```
decentpaste-app/src-tauri/gen/apple/
├── ShareExtension/                    # Created by Xcode "New Target"
│   ├── ShareViewController.swift     # Replaced with our version
│   ├── Info.plist                    # Replaced with our version
│   └── ShareExtension.entitlements   # Created by Xcode
└── decentpaste-app_iOS/
    └── decentpaste-app_iOS.entitlements  # Modified by Xcode
```

---

## Implementation Details

### Step 1: Install Tauri Deep-Link Plugin

This plugin handles URL scheme registration automatically via `tauri.conf.json`.

**1.1 Install JavaScript package:**

```bash
cd decentpaste-app
yarn add @tauri-apps/plugin-deep-link
```

**1.2 Install Rust crate:**

Add to `src-tauri/Cargo.toml` in `[dependencies]`:

```toml
tauri-plugin-deep-link = "2"
```

**1.3 Configure in `src-tauri/tauri.conf.json`:**

Add to the `plugins` section (after `updater`):

```json
{
  "plugins": {
    "updater": { ... },
    "deep-link": {
      "mobile": [
        { "scheme": ["decentpaste"], "appLink": false }
      ]
    }
  }
}
```

**1.4 Initialize in `src-tauri/src/lib.rs`:**

Add to the plugin chain in the `run()` function:

```rust
.plugin(tauri_plugin_deep_link::init())
```

---

### Step 2: Create DecentsharePlugin.swift

**File:** `decentpaste-app/tauri-plugin-decentshare/ios/Sources/DecentsharePlugin.swift`

This is the main Tauri iOS plugin that reads shared content from App Groups.

```swift
import UIKit
import WebKit
import Tauri
import os.log

/// App Group identifier - MUST match the one configured in Xcode
private let appGroupIdentifier = "group.com.decentpaste.application"

/// UserDefaults key for pending share content
private let pendingShareKey = "pendingShareContent"

/// Tauri plugin for iOS share extension functionality.
///
/// This plugin mirrors the Android DecentsharePlugin API:
/// - `getPendingShare()` - Returns pending shared content
/// - `clearPendingShare()` - Clears pending shared content
///
/// Data flow:
/// 1. ShareExtension saves text to App Groups UserDefaults
/// 2. Main app calls getPendingShare() to retrieve it
/// 3. Content is cleared after retrieval (atomic get-and-clear)
class DecentsharePlugin: Plugin {
    private let logger = Logger(subsystem: "com.decentpaste.application", category: "DecentsharePlugin")

    /// Shared UserDefaults for App Group communication
    private var sharedDefaults: UserDefaults? {
        UserDefaults(suiteName: appGroupIdentifier)
    }

    /// Called when plugin is loaded into the WebView
    @objc public override func load(webview: WKWebView) {
        logger.info("DecentsharePlugin loaded")
    }

    /// Get pending shared content from the share extension.
    ///
    /// Response format (matches Android):
    /// ```json
    /// { "content": "shared text" | null, "hasPending": true | false }
    /// ```
    ///
    /// IMPORTANT: This clears the content after retrieval to prevent
    /// processing the same share multiple times.
    @objc public func getPendingShare(_ invoke: Invoke) {
        guard let defaults = sharedDefaults else {
            logger.error("Failed to access App Group UserDefaults - check App Group configuration")
            invoke.resolve([
                "content": NSNull(),
                "hasPending": false
            ])
            return
        }

        let content = defaults.string(forKey: pendingShareKey)

        // Clear after retrieval (atomic get-and-clear pattern)
        if content != nil {
            defaults.removeObject(forKey: pendingShareKey)
            defaults.synchronize() // Force immediate write
            logger.info("Retrieved and cleared pending share (\(content!.count) chars)")
        }

        invoke.resolve([
            "content": content as Any,
            "hasPending": content != nil
        ])
    }

    /// Clear any pending shared content.
    ///
    /// Called by frontend after successfully processing shared content.
    /// This is a safety mechanism - content should already be cleared
    /// by getPendingShare(), but this ensures cleanup.
    @objc public func clearPendingShare(_ invoke: Invoke) {
        guard let defaults = sharedDefaults else {
            logger.error("Failed to access App Group UserDefaults")
            invoke.resolve()
            return
        }

        defaults.removeObject(forKey: pendingShareKey)
        defaults.synchronize()
        logger.info("Cleared pending share content")
        invoke.resolve()
    }
}

/// Plugin initialization function called from Rust via FFI.
///
/// This function is referenced in mobile.rs:
/// ```rust
/// tauri::ios_plugin_binding!(init_plugin_decentshare);
/// ```
///
/// The @_cdecl attribute exports this function with C linkage so it can be
/// called from Rust code. It simply returns the plugin instance;
/// Tauri handles registration automatically.
@_cdecl("init_plugin_decentshare")
func initPlugin() -> Plugin {
    return DecentsharePlugin()
}
```

---

### Step 3: Create ShareViewController.swift

**File:** `decentpaste-app/tauri-plugin-decentshare/ios/ShareExtension/ShareViewController.swift`

This is the share extension entry point that receives shared content.

```swift
import UIKit
import UniformTypeIdentifiers
import os.log

/// App Group identifier - MUST match DecentsharePlugin.swift
private let appGroupIdentifier = "group.com.decentpaste.application"

/// UserDefaults key - MUST match DecentsharePlugin.swift
private let pendingShareKey = "pendingShareContent"

/// URL scheme for opening main app
private let appURLScheme = "decentpaste"

/// Share extension view controller.
///
/// This extension appears in the iOS share sheet when sharing text.
/// It saves the shared content to App Groups and attempts to open
/// the main DecentPaste app.
///
/// Flow:
/// 1. User shares text → iOS shows share sheet → User selects DecentPaste
/// 2. This view controller loads
/// 3. Extract text from extensionContext
/// 4. Save to App Groups UserDefaults
/// 5. Show toast notification
/// 6. Attempt to open main app via URL scheme
/// 7. Complete extension request
class ShareViewController: UIViewController {

    private let logger = Logger(subsystem: "com.decentpaste.application.ShareExtension", category: "ShareViewController")

    // MARK: - UI Elements

    private lazy var toastLabel: UILabel = {
        let label = UILabel()
        label.text = "Saved! Opening DecentPaste..."
        label.textColor = .white
        label.textAlignment = .center
        label.font = .systemFont(ofSize: 16, weight: .medium)
        label.backgroundColor = UIColor.black.withAlphaComponent(0.8)
        label.layer.cornerRadius = 12
        label.layer.masksToBounds = true
        label.translatesAutoresizingMaskIntoConstraints = false
        return label
    }()

    // MARK: - Lifecycle

    override func viewDidLoad() {
        super.viewDidLoad()

        // Semi-transparent background
        view.backgroundColor = UIColor.black.withAlphaComponent(0.3)

        // Setup toast
        setupToast()

        // Process shared content
        handleSharedContent()
    }

    // MARK: - UI Setup

    private func setupToast() {
        view.addSubview(toastLabel)

        NSLayoutConstraint.activate([
            toastLabel.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            toastLabel.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            toastLabel.widthAnchor.constraint(lessThanOrEqualTo: view.widthAnchor, multiplier: 0.8),
            toastLabel.heightAnchor.constraint(equalToConstant: 50)
        ])

        // Add padding to label
        toastLabel.layoutMargins = UIEdgeInsets(top: 12, left: 24, bottom: 12, right: 24)
    }

    // MARK: - Share Handling

    private func handleSharedContent() {
        guard let extensionItem = extensionContext?.inputItems.first as? NSExtensionItem,
              let attachments = extensionItem.attachments else {
            logger.error("No extension items or attachments found")
            closeExtension(success: false)
            return
        }

        // Find text attachment
        for itemProvider in attachments {
            // Try plain text first
            if itemProvider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                loadText(from: itemProvider, typeIdentifier: UTType.plainText.identifier)
                return
            }

            // Fall back to URL (shared links come as URLs)
            if itemProvider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                loadText(from: itemProvider, typeIdentifier: UTType.url.identifier)
                return
            }
        }

        logger.warning("No text or URL attachment found")
        closeExtension(success: false)
    }

    private func loadText(from itemProvider: NSItemProvider, typeIdentifier: String) {
        itemProvider.loadItem(forTypeIdentifier: typeIdentifier, options: nil) { [weak self] item, error in
            guard let self = self else { return }

            if let error = error {
                self.logger.error("Failed to load item: \(error.localizedDescription)")
                self.closeExtension(success: false)
                return
            }

            // Extract text from item
            var sharedText: String?

            if let text = item as? String {
                sharedText = text
            } else if let url = item as? URL {
                sharedText = url.absoluteString
            } else if let data = item as? Data {
                sharedText = String(data: data, encoding: .utf8)
            }

            guard let text = sharedText, !text.isEmpty else {
                self.logger.warning("Shared text is empty or nil")
                self.closeExtension(success: false)
                return
            }

            self.logger.info("Received shared text (\(text.count) chars)")

            // Save to App Groups
            self.savePendingShare(text)

            // Show toast and open main app
            DispatchQueue.main.async {
                self.showToastAndOpenApp()
            }
        }
    }

    // MARK: - App Groups Storage

    private func savePendingShare(_ content: String) {
        guard let sharedDefaults = UserDefaults(suiteName: appGroupIdentifier) else {
            logger.error("Failed to access App Group UserDefaults")
            return
        }

        sharedDefaults.set(content, forKey: pendingShareKey)
        sharedDefaults.synchronize() // Force immediate write
        logger.info("Saved pending share to App Group")
    }

    // MARK: - App Opening

    private func showToastAndOpenApp() {
        // Animate toast appearance
        toastLabel.alpha = 0
        toastLabel.transform = CGAffineTransform(scaleX: 0.8, y: 0.8)

        UIView.animate(withDuration: 0.3) {
            self.toastLabel.alpha = 1
            self.toastLabel.transform = .identity
        }

        // Attempt to open main app after brief delay
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
            self.openMainApp()
        }
    }

    private func openMainApp() {
        guard let url = URL(string: "\(appURLScheme)://share") else {
            logger.error("Failed to create app URL")
            closeExtension(success: true) // Still success - data is saved
            return
        }

        // Use responder chain to open URL (workaround for extension limitations)
        // Share extensions cannot use UIApplication.shared directly
        var responder: UIResponder? = self
        let selector = sel_registerName("openURL:")

        while responder != nil {
            if responder!.responds(to: selector) {
                responder!.perform(selector, with: url)
                logger.info("Attempted to open main app via URL scheme")
                break
            }
            responder = responder?.next
        }

        // Close extension after attempting to open
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            self.closeExtension(success: true)
        }
    }

    // MARK: - Extension Lifecycle

    private func closeExtension(success: Bool) {
        DispatchQueue.main.async {
            if success {
                self.extensionContext?.completeRequest(returningItems: nil, completionHandler: nil)
            } else {
                let error = NSError(domain: "com.decentpaste.ShareExtension", code: 1, userInfo: [
                    NSLocalizedDescriptionKey: "Failed to process shared content"
                ])
                self.extensionContext?.cancelRequest(withError: error)
            }
        }
    }
}
```

---

### Step 4: Create Extension Info.plist

**File:** `decentpaste-app/tauri-plugin-decentshare/ios/ShareExtension/Info.plist`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDisplayName</key>
    <string>DecentPaste</string>
    <key>CFBundleExecutable</key>
    <string>$(EXECUTABLE_NAME)</string>
    <key>CFBundleIdentifier</key>
    <string>$(PRODUCT_BUNDLE_IDENTIFIER)</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$(PRODUCT_NAME)</string>
    <key>CFBundlePackageType</key>
    <string>$(PRODUCT_BUNDLE_PACKAGE_TYPE)</string>
    <key>CFBundleShortVersionString</key>
    <string>$(MARKETING_VERSION)</string>
    <key>CFBundleVersion</key>
    <string>$(CURRENT_PROJECT_VERSION)</string>
    <key>NSExtension</key>
    <dict>
        <key>NSExtensionAttributes</key>
        <dict>
            <key>NSExtensionActivationRule</key>
            <dict>
                <!-- Accept plain text -->
                <key>NSExtensionActivationSupportsText</key>
                <true/>
                <!-- Accept URLs (shared links) -->
                <key>NSExtensionActivationSupportsWebURLWithMaxCount</key>
                <integer>1</integer>
            </dict>
        </dict>
        <key>NSExtensionPointIdentifier</key>
        <string>com.apple.share-services</string>
        <key>NSExtensionPrincipalClass</key>
        <string>$(PRODUCT_MODULE_NAME).ShareViewController</string>
    </dict>
</dict>
</plist>
```

---

### Step 5: Create Helper Script

**File:** `decentpaste-app/tauri-plugin-decentshare/ios/scripts/copy-files.sh`

This script is optional - it just copies files to a convenient location.

```bash
#!/bin/bash
#
# Copy iOS Share Extension source files for easy access in Xcode.
#
# This script does NOT modify the Xcode project - you still need to
# add the files manually in Xcode. See README.md for instructions.
#
# Usage: ./copy-files.sh
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$(dirname "$(dirname "$PLUGIN_DIR")")")"
DEST_DIR="$PROJECT_ROOT/src-tauri/gen/apple/ShareExtension-source"

echo "=== DecentPaste iOS Share Extension File Copy ==="
echo ""
echo "Source: $PLUGIN_DIR"
echo "Destination: $DEST_DIR"
echo ""

# Check if source files exist
if [ ! -f "$PLUGIN_DIR/ShareExtension/ShareViewController.swift" ]; then
    echo "ERROR: Source files not found. Make sure you're running from the correct directory."
    exit 1
fi

# Create destination directory
mkdir -p "$DEST_DIR"

# Copy ShareExtension files
echo "Copying ShareExtension files..."
cp "$PLUGIN_DIR/ShareExtension/ShareViewController.swift" "$DEST_DIR/"
cp "$PLUGIN_DIR/ShareExtension/Info.plist" "$DEST_DIR/"

# Copy plugin files
echo "Copying DecentsharePlugin files..."
mkdir -p "$DEST_DIR/Plugin"
cp "$PLUGIN_DIR/Sources/DecentsharePlugin/DecentsharePlugin.swift" "$DEST_DIR/Plugin/"

echo ""
echo "=== Files copied successfully ==="
echo ""
echo "Files are now at: $DEST_DIR"
echo ""
echo "NEXT STEPS:"
echo "1. Open Xcode: open $PROJECT_ROOT/src-tauri/gen/apple/decentpaste-app.xcodeproj"
echo "2. Follow the setup guide in README.md to add these files to the project"
echo ""
```

---

### Step 6: Update package.json

**File:** `decentpaste-app/package.json`

Add to the `scripts` section:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "tauri": "tauri",
    "format:fix": "prettier --write .",
    "ios:copy-files": "cd tauri-plugin-decentshare/ios/scripts && chmod +x copy-files.sh && ./copy-files.sh"
  }
}
```

---

### Step 7: Update Frontend (Optional)

**File:** `decentpaste-app/src/main.ts`

For faster response when the app opens via URL scheme, add a deep link listener. This is optional because the existing `checkForPendingShare()` polling on visibility change will also work.

Add after existing imports:

```typescript
import { onOpenUrl } from '@tauri-apps/plugin-deep-link';
```

Add in the initialization section (after `setupEventListeners()`):

```typescript
// Listen for deep link from share extension
try {
    await onOpenUrl((urls) => {
        if (urls.some(u => u.startsWith('decentpaste://'))) {
            console.log('[Share] Deep link received, checking for pending share');
            checkForPendingShare();
        }
    });
} catch (e) {
    // Deep link plugin may not be available on all platforms
    console.log('[Share] Deep link listener not available:', e);
}
```

---

### Step 8: Update Plugin README

**File:** `decentpaste-app/tauri-plugin-decentshare/README.md`

Add an iOS section with the complete Xcode setup guide. See the "Complete Xcode Setup Guide" section below for the content to add.

---

## Complete Xcode Setup Guide

This guide explains how to set up the iOS Share Extension in Xcode. Follow these steps after `yarn tauri ios init` or whenever `gen/apple/` is regenerated.

### Prerequisites

1. Apple Developer Account with ability to create App Groups
2. Xcode installed (14.0+)
3. iOS project initialized: `yarn tauri ios init`
4. Source files created (Steps 2-4 above)

### Step A: Create App Group in Apple Developer Portal

1. Go to [Apple Developer Portal - Identifiers](https://developer.apple.com/account/resources/identifiers/list/applicationGroup)
2. Click "+" to register a new identifier
3. Select "App Groups" → Continue
4. Enter description: "DecentPaste Shared Data"
5. Enter identifier: `group.com.decentpaste.application`
6. Click Continue → Register

### Step B: Open Xcode Project

```bash
cd decentpaste-app
open src-tauri/gen/apple/decentpaste-app.xcodeproj
```

### Step C: Add Share Extension Target

1. In Xcode menu: File → New → Target...
2. Select iOS tab → "Share Extension" → Next
3. Configure:
   - **Product Name:** `ShareExtension`
   - **Team:** (Your development team)
   - **Organization Identifier:** `com.decentpaste.application`
   - **Bundle Identifier:** `com.decentpaste.application.ShareExtension` (auto-filled)
   - **Language:** Swift
   - **Project:** decentpaste-app
   - **Embed in Application:** decentpaste-app_iOS
4. Click Finish
5. When prompted "Activate ShareExtension scheme?", click **Activate**

### Step D: Replace Generated Files with Custom Implementation

Xcode creates template files. Replace them with our implementation:

1. In Project Navigator, expand the `ShareExtension` group
2. **Delete** the auto-generated files:
   - Select `ShareViewController.swift` → Delete → Move to Trash
   - Select `MainInterface.storyboard` (if present) → Delete → Move to Trash
3. Right-click the `ShareExtension` group → "Add Files to 'decentpaste-app'..."
4. Navigate to `tauri-plugin-decentshare/ios/ShareExtension/`
   - Or if you ran the copy script: `src-tauri/gen/apple/ShareExtension-source/`
5. Select both files:
   - `ShareViewController.swift`
   - `Info.plist`
6. Configure:
   - ☑️ Copy items if needed
   - ☑️ Create groups
   - Add to targets: ☑️ ShareExtension
7. Click Add

### Step E: Configure App Groups Capability

**For main app target (decentpaste-app_iOS):**

1. In Project Navigator, click the project (blue icon at top)
2. Select target: `decentpaste-app_iOS`
3. Select "Signing & Capabilities" tab
4. Click "+ Capability" button (top left)
5. Search and select "App Groups"
6. Under App Groups section, click "+"
7. Select or enter: `group.com.decentpaste.application`
8. If it shows a warning, click "Fix Issue" to update provisioning

**For ShareExtension target:**

1. Select target: `ShareExtension`
2. Select "Signing & Capabilities" tab
3. Click "+ Capability"
4. Select "App Groups"
5. Click "+" and select the SAME group: `group.com.decentpaste.application`

### Step F: Configure Code Signing

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

### Step G: Verify Extension Embedding

1. Select target: `decentpaste-app_iOS`
2. Select "General" tab
3. Scroll to "Frameworks, Libraries, and Embedded Content"
4. Verify `ShareExtension.appex` is listed
   - If missing: Click "+" → Under "Embed App Extensions" select ShareExtension

### Step H: Verify Info.plist Configuration

1. Select the `ShareExtension/Info.plist` you added
2. Verify it contains:
   - `NSExtensionPointIdentifier`: `com.apple.share-services`
   - `NSExtensionPrincipalClass`: `$(PRODUCT_MODULE_NAME).ShareViewController`
   - `NSExtensionActivationSupportsText`: `YES`

### Step I: Build and Test

1. Connect a physical iOS device (Share Extensions don't work reliably in Simulator)
2. Select scheme: `decentpaste-app_iOS`
3. Select your device as destination
4. Build: Product → Build (Cmd+B)
5. If build succeeds, run: Product → Run (Cmd+R)

**Testing:**

1. Open Safari on the device
2. Navigate to any webpage
3. Select some text → Share button
4. DecentPaste should appear in share sheet
5. Tap DecentPaste → Should show toast → App should open

---

## Configuration Values

| Setting | Value |
|---------|-------|
| App Group Identifier | `group.com.decentpaste.application` |
| URL Scheme | `decentpaste` |
| Extension Bundle ID | `com.decentpaste.application.ShareExtension` |
| Main App Bundle ID | `com.decentpaste.application` |
| UserDefaults Key | `pendingShareContent` |
| iOS Deployment Target | 14.0 |
| Extension Display Name | "DecentPaste" |

---

## Testing Checklist

- [ ] **Cold start**: Kill app completely → Share text from Safari → App should open and process
- [ ] **Warm start**: Background app → Share text → App should resume and process
- [ ] **Vault locked**: Share text while vault is locked → Should prompt to unlock → Process after unlock
- [ ] **URL sharing**: Share a URL from Safari → URL text should sync correctly
- [ ] **Long text**: Share 10KB+ of text → Should handle without crash
- [ ] **Rapid shares**: Share multiple times quickly → Last share should be processed
- [ ] **Extension dismissal**: Share → Wait for toast → Extension should close cleanly
- [ ] **Manual app open**: Share → If URL scheme fails → Manually open app → Should still process

---

## Troubleshooting

### "App Group container could not be accessed"

- Verify App Group is created in Apple Developer Portal
- Verify SAME App Group ID is added to BOTH targets in Xcode
- Check the identifier matches exactly: `group.com.decentpaste.application`
- Try removing and re-adding the App Groups capability

### Share Extension doesn't appear in share sheet

- Verify extension is embedded in main app (Step G)
- Verify NSExtensionActivationRule accepts text
- Build and run the main app at least once
- Check device Settings → (App Name) → Share Extension is enabled

### Extension appears but crashes

- Check Console.app for crash logs (filter by "ShareExtension")
- Verify Info.plist NSExtensionPrincipalClass matches class name
- Ensure all Swift files are added to ShareExtension target (check Target Membership)

### Main app doesn't receive shared content

- Verify App Group IDs match in both Swift files
- Check UserDefaults key matches: `pendingShareContent`
- Add logging to DecentsharePlugin.swift to debug
- Verify frontend is calling `checkForPendingShare()`

### URL scheme doesn't open app

- This is expected behavior in some iOS versions
- The fallback is: user manually switches to DecentPaste
- Content is saved to App Groups regardless
- App will pick it up on visibility change

### Build errors after regenerating gen/apple/

- The Share Extension target is lost when regenerating
- Follow the complete Xcode Setup Guide again
- Source files are preserved in `tauri-plugin-decentshare/ios/`

---

## Reference: Existing Android Implementation

For reference, here's how the Android implementation works (in case you need to ensure API compatibility):

**Android Flow:**
1. User shares → System sends ACTION_SEND intent
2. `DecentsharePlugin.kt` receives via `onNewIntent()` or `load()`
3. Stores in `@Volatile pendingShareContent` variable
4. Frontend calls `getPendingShare()` command
5. Returns `{ content: string | null, hasPending: boolean }`

**Key files:**
- `tauri-plugin-decentshare/android/src/main/java/DecentsharePlugin.kt`
- `tauri-plugin-decentshare/android/src/main/AndroidManifest.xml`

The iOS implementation MUST use the same API response format for frontend compatibility.
