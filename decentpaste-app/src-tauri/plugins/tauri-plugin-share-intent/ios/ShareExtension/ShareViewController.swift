import UIKit
import Social
import MobileCoreServices
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

            // Store in shared container
            self.storeSharedContent(fullText)

            // Open main app (optional - may not work in all cases)
            self.openMainApp()

            // Complete the extension
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

    // MARK: - Open Main App

    private func openMainApp() {
        // Try to open main app via URL scheme
        // Note: This may not work reliably from extensions
        guard let url = URL(string: "decentpaste://share") else { return }

        // Use the responder chain to find the application
        var responder: UIResponder? = self
        while responder != nil {
            if let application = responder as? UIApplication {
                application.open(url, options: [:], completionHandler: nil)
                break
            }
            responder = responder?.next
        }
    }
}
