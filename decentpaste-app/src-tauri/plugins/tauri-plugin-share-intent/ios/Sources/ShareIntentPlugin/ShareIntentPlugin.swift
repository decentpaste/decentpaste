import Foundation
import Tauri
import WebKit

class ShareIntentPlugin: Plugin {

    // App Group identifier - must match extension
    private static let appGroupId = "group.com.decentpaste.application"
    private static let pendingContentKey = "shareIntent_pendingContent"
    private static let pendingTimestampKey = "shareIntent_timestamp"

    // Track the timestamp of the last processed content to prevent duplicate processing
    // Using timestamp instead of content value allows same text to be shared multiple times
    private var lastProcessedTimestamp: Date?

    // MARK: - Plugin Lifecycle

    override func load(webview: WKWebView) {
        super.load(webview: webview)

        // Register for app lifecycle notifications
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(applicationDidBecomeActive),
            name: UIApplication.didBecomeActiveNotification,
            object: nil
        )

        // Check for pending content from Share Extension (cold start)
        checkPendingContent()
    }

    /// Called when app becomes active (warm start, returning from background)
    @objc func applicationDidBecomeActive() {
        checkPendingContent()
    }

    // MARK: - Content Handling

    /// Check for pending content in App Group shared storage
    private func checkPendingContent() {
        guard let defaults = UserDefaults(suiteName: Self.appGroupId) else {
            print("[ShareIntentPlugin] Failed to access App Group defaults")
            return
        }

        // Check if there's pending content
        guard let content = defaults.string(forKey: Self.pendingContentKey) else {
            return
        }

        // Get the timestamp when content was stored
        guard let timestamp = defaults.object(forKey: Self.pendingTimestampKey) as? Date else {
            // No timestamp means malformed data, clear it
            clearPendingContentFromDefaults()
            return
        }

        // Expire content after 5 minutes
        let fiveMinutesAgo = Date().addingTimeInterval(-300)
        if timestamp < fiveMinutesAgo {
            clearPendingContentFromDefaults()
            return
        }

        // Use timestamp to detect if this is new content (allows same text to be shared multiple times)
        if let lastProcessed = lastProcessedTimestamp, timestamp <= lastProcessed {
            return
        }
        lastProcessedTimestamp = timestamp

        // Emit event to frontend
        trigger("share-intent-received", data: [
            "content": content,
            "source": "ios"
        ])
    }

    /// Clear pending content from shared storage
    private func clearPendingContentFromDefaults() {
        guard let defaults = UserDefaults(suiteName: Self.appGroupId) else { return }
        defaults.removeObject(forKey: Self.pendingContentKey)
        defaults.removeObject(forKey: Self.pendingTimestampKey)
        defaults.synchronize()
    }

    // MARK: - Commands

    /// Get and consume pending content
    @objc public func getPendingContent(_ invoke: Invoke) throws {
        guard let defaults = UserDefaults(suiteName: Self.appGroupId) else {
            invoke.resolve(["content": NSNull(), "source": NSNull()])
            return
        }

        if let content = defaults.string(forKey: Self.pendingContentKey) {
            // Clear after reading (consume)
            clearPendingContentFromDefaults()

            invoke.resolve([
                "content": content,
                "source": "ios"
            ])
        } else {
            invoke.resolve(["content": NSNull(), "source": NSNull()])
        }
    }

    /// Check if there's pending content
    @objc public func hasPendingContent(_ invoke: Invoke) throws {
        guard let defaults = UserDefaults(suiteName: Self.appGroupId),
              let _ = defaults.string(forKey: Self.pendingContentKey) else {
            invoke.resolve(["hasPending": false])
            return
        }
        invoke.resolve(["hasPending": true])
    }

    /// Clear pending content without processing
    @objc public func clearPendingContent(_ invoke: Invoke) throws {
        clearPendingContentFromDefaults()
        lastProcessedTimestamp = nil
        invoke.resolve([:])
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }
}

@_cdecl("init_plugin_share_intent")
func initPlugin() -> Plugin {
    return ShareIntentPlugin()
}
