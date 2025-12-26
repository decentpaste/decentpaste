import UIKit
import Social
import UniformTypeIdentifiers

class ShareViewController: SLComposeServiceViewController {

    private static let appGroupId = "group.com.decentpaste.application"
    private static let pendingContentKey = "shareIntent_pendingContent"
    private static let pendingTimestampKey = "shareIntent_timestamp"

    // MARK: - Lifecycle

    override func isContentValid() -> Bool {
        // Validate that we have text content
        return !contentText.isEmpty || hasAttachmentOfType(UTType.plainText)
    }

    override func didSelectPost() {
        // Get shared text content
        extractTextContent { [weak self] text in
            guard let self = self, let text = text else {
                self?.extensionContext?.completeRequest(returningItems: nil, completionHandler: nil)
                return
            }

            // Combine with any text from compose box
            var fullText = text
            if !self.contentText.isEmpty {
                fullText = "\(self.contentText)\n\n\(text)"
            }

            // Store in shared container for main app to pick up
            self.storeSharedContent(fullText)

            // Complete the extension
            // Note: Main app will detect content via App Groups when it becomes active
            self.extensionContext?.completeRequest(returningItems: nil, completionHandler: nil)
        }
    }

    override func configurationItems() -> [Any]! {
        // Add configuration items if needed
        return []
    }

    // MARK: - Content Extraction

    private func hasAttachmentOfType(_ type: UTType) -> Bool {
        guard let extensionItem = extensionContext?.inputItems.first as? NSExtensionItem,
              let attachments = extensionItem.attachments else {
            return false
        }
        return attachments.contains { $0.hasItemConformingToTypeIdentifier(type.identifier) }
    }

    private func extractTextContent(completion: @escaping (String?) -> Void) {
        guard let extensionItem = extensionContext?.inputItems.first as? NSExtensionItem,
              let itemProvider = extensionItem.attachments?.first else {
            completion(contentText.isEmpty ? nil : contentText)
            return
        }

        // Try to get plain text
        if itemProvider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
            itemProvider.loadItem(forTypeIdentifier: UTType.plainText.identifier, options: nil) { item, error in
                if let text = item as? String {
                    completion(text)
                } else {
                    completion(nil)
                }
            }
        } else if itemProvider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
            // Handle URLs as text
            itemProvider.loadItem(forTypeIdentifier: UTType.url.identifier, options: nil) { item, error in
                if let url = item as? URL {
                    completion(url.absoluteString)
                } else {
                    completion(nil)
                }
            }
        } else {
            completion(contentText.isEmpty ? nil : contentText)
        }
    }

    // MARK: - Storage

    private func storeSharedContent(_ content: String) {
        guard let defaults = UserDefaults(suiteName: Self.appGroupId) else {
            print("[ShareExtension] Failed to access App Group defaults")
            return
        }

        defaults.set(content, forKey: Self.pendingContentKey)
        defaults.set(Date(), forKey: Self.pendingTimestampKey)
        defaults.synchronize()

        print("[ShareExtension] Stored content: \(content.prefix(50))...")
    }
}
