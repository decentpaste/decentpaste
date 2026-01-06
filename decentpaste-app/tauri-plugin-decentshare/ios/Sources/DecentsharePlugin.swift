import UIKit
import WebKit
import Tauri
import os.log

/// App Group identifier - MUST match the one configured in Xcode for both
/// the main app and ShareExtension targets.
private let appGroupIdentifier = "group.com.decentpaste.application"

/// UserDefaults key for pending share content.
/// ShareExtension writes to this key, and this plugin reads from it.
private let pendingShareKey = "pendingShareContent"

/// Tauri plugin for iOS share extension functionality.
///
/// This plugin mirrors the Android DecentsharePlugin API:
/// - `getPendingShare()` - Returns pending shared content from App Groups
/// - `clearPendingShare()` - Clears pending shared content
///
/// ## Data Flow
/// 1. User shares text via iOS share sheet → ShareExtension
/// 2. ShareExtension saves text to App Groups UserDefaults
/// 3. ShareExtension shows confirmation, user taps "Done" to dismiss
/// 4. User opens DecentPaste manually → visibility change triggers check
/// 5. Frontend calls `getPendingShare()` to retrieve the content
/// 6. Content is cleared after retrieval (atomic get-and-clear pattern)
///
/// ## API Response Format (matches Android)
/// ```json
/// {
///   "content": "shared text" | null,
///   "hasPending": true | false
/// }
/// ```
class DecentsharePlugin: Plugin {
    private let logger = Logger(subsystem: "com.decentpaste.application", category: "DecentsharePlugin")

    /// Shared UserDefaults for App Group communication between main app and extension.
    private var sharedDefaults: UserDefaults? {
        UserDefaults(suiteName: appGroupIdentifier)
    }

    /// Called when plugin is loaded into the WebView.
    @objc public override func load(webview: WKWebView) {
        logger.info("DecentsharePlugin loaded")
    }

    /// Get pending shared content from the share extension.
    ///
    /// This command is called by the frontend to check for content that was shared
    /// via the iOS share sheet. The content is stored in App Groups UserDefaults
    /// by the ShareExtension.
    ///
    /// ## Response Format
    /// ```json
    /// { "content": "shared text" | null, "hasPending": true | false }
    /// ```
    ///
    /// ## Important
    /// This method clears the content after retrieval to prevent processing
    /// the same share multiple times (atomic get-and-clear pattern).
    @objc public func getPendingShare(_ invoke: Invoke) {
        guard let defaults = sharedDefaults else {
            logger.error("Failed to access App Group UserDefaults - verify App Group is configured in both targets")
            invoke.resolve([
                "content": NSNull(),
                "hasPending": false
            ])
            return
        }

        let content = defaults.string(forKey: pendingShareKey)

        // Clear after retrieval (atomic get-and-clear pattern)
        // This prevents processing the same share multiple times
        if content != nil {
            defaults.removeObject(forKey: pendingShareKey)
            defaults.synchronize() // Force immediate write to disk
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
    /// by `getPendingShare()`, but this ensures cleanup if needed.
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
/// #[cfg(target_os = "ios")]
/// tauri::ios_plugin_binding!(init_plugin_decentshare);
/// ```
///
/// The @_cdecl attribute exports this function with C linkage so it can be
/// called from Rust code. It simply creates and returns the plugin instance;
/// Tauri handles the registration automatically.
@_cdecl("init_plugin_decentshare")
func initPlugin() -> Plugin {
    return DecentsharePlugin()
}
