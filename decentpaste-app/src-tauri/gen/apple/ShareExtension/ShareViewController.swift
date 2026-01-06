import UIKit
import UniformTypeIdentifiers
import os.log

// MARK: - Constants

/// App Group identifier - MUST match DecentsharePlugin.swift and Xcode configuration
private let appGroupIdentifier = "group.com.decentpaste.application"

/// UserDefaults key - MUST match DecentsharePlugin.swift
private let pendingShareKey = "pendingShareContent"

/// URL scheme for opening the main DecentPaste app
private let appURLScheme = "decentpaste"

// MARK: - ShareViewController

/// Share extension view controller that receives shared content from the iOS share sheet.
///
/// ## Overview
/// This extension appears when users share text from any iOS app. It:
/// 1. Extracts the shared text from the extension context
/// 2. Saves it to App Groups UserDefaults (shared with main app)
/// 3. Shows a brief toast notification
/// 4. Attempts to open the main DecentPaste app via URL scheme
/// 5. Closes the extension
///
/// ## Data Flow
/// ```
/// User shares text → iOS share sheet → ShareViewController
///                                            ↓
///                               Save to App Groups UserDefaults
///                                            ↓
///                               Show toast "Saved! Opening DecentPaste..."
///                                            ↓
///                               Open decentpaste://share URL
///                                            ↓
///                               Main app reads via getPendingShare()
/// ```
///
/// ## Memory Constraints
/// iOS share extensions have a ~120MB memory limit. This implementation is
/// lightweight and should complete well within the 30-second timeout.
class ShareViewController: UIViewController {

    private let logger = Logger(subsystem: "com.decentpaste.application.ShareExtension", category: "ShareViewController")

    // MARK: - UI Elements

    /// Toast label shown during share processing
    private lazy var toastContainer: UIView = {
        let view = UIView()
        view.backgroundColor = UIColor.black.withAlphaComponent(0.85)
        view.layer.cornerRadius = 16
        view.layer.masksToBounds = true
        view.translatesAutoresizingMaskIntoConstraints = false
        return view
    }()

    private lazy var toastLabel: UILabel = {
        let label = UILabel()
        label.text = "Saved! Opening DecentPaste..."
        label.textColor = .white
        label.textAlignment = .center
        label.font = .systemFont(ofSize: 16, weight: .medium)
        label.numberOfLines = 0
        label.translatesAutoresizingMaskIntoConstraints = false
        return label
    }()

    private lazy var checkmarkImageView: UIImageView = {
        let config = UIImage.SymbolConfiguration(pointSize: 32, weight: .medium)
        let image = UIImage(systemName: "checkmark.circle.fill", withConfiguration: config)
        let imageView = UIImageView(image: image)
        imageView.tintColor = UIColor.systemGreen
        imageView.translatesAutoresizingMaskIntoConstraints = false
        return imageView
    }()

    // MARK: - Lifecycle

    override func viewDidLoad() {
        super.viewDidLoad()

        // Semi-transparent background
        view.backgroundColor = UIColor.black.withAlphaComponent(0.4)

        // Setup UI
        setupToast()

        // Process shared content
        handleSharedContent()
    }

    // MARK: - UI Setup

    private func setupToast() {
        // Add container
        view.addSubview(toastContainer)

        // Add checkmark and label to container
        toastContainer.addSubview(checkmarkImageView)
        toastContainer.addSubview(toastLabel)

        // Container constraints (centered)
        NSLayoutConstraint.activate([
            toastContainer.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            toastContainer.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            toastContainer.widthAnchor.constraint(lessThanOrEqualTo: view.widthAnchor, multiplier: 0.8),
            toastContainer.widthAnchor.constraint(greaterThanOrEqualToConstant: 200)
        ])

        // Checkmark constraints
        NSLayoutConstraint.activate([
            checkmarkImageView.topAnchor.constraint(equalTo: toastContainer.topAnchor, constant: 20),
            checkmarkImageView.centerXAnchor.constraint(equalTo: toastContainer.centerXAnchor)
        ])

        // Label constraints
        NSLayoutConstraint.activate([
            toastLabel.topAnchor.constraint(equalTo: checkmarkImageView.bottomAnchor, constant: 12),
            toastLabel.leadingAnchor.constraint(equalTo: toastContainer.leadingAnchor, constant: 24),
            toastLabel.trailingAnchor.constraint(equalTo: toastContainer.trailingAnchor, constant: -24),
            toastLabel.bottomAnchor.constraint(equalTo: toastContainer.bottomAnchor, constant: -20)
        ])

        // Initially hidden for animation
        toastContainer.alpha = 0
        toastContainer.transform = CGAffineTransform(scaleX: 0.8, y: 0.8)
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
            // Try plain text first (most common)
            if itemProvider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                loadText(from: itemProvider, typeIdentifier: UTType.plainText.identifier)
                return
            }

            // Fall back to URL (shared links from Safari, etc.)
            if itemProvider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                loadText(from: itemProvider, typeIdentifier: UTType.url.identifier)
                return
            }
        }

        logger.warning("No text or URL attachment found in shared content")
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

            // Extract text from item (can be String, URL, or Data)
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

            self.logger.info("Received shared text (\(text.count) characters)")

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
            logger.error("Failed to access App Group UserDefaults - verify App Group configuration")
            return
        }

        sharedDefaults.set(content, forKey: pendingShareKey)
        sharedDefaults.synchronize() // Force immediate write to disk
        logger.info("Saved pending share to App Group (\(content.count) chars)")
    }

    // MARK: - App Opening

    private func showToastAndOpenApp() {
        // Animate toast appearance
        UIView.animate(withDuration: 0.3, delay: 0, usingSpringWithDamping: 0.7, initialSpringVelocity: 0.5) {
            self.toastContainer.alpha = 1
            self.toastContainer.transform = .identity
        }

        // Attempt to open main app after showing toast
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
        // Even if URL open fails, the data is saved and will be picked up
        // when the user manually opens the app
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
                let error = NSError(
                    domain: "com.decentpaste.ShareExtension",
                    code: 1,
                    userInfo: [NSLocalizedDescriptionKey: "Failed to process shared content"]
                )
                self.extensionContext?.cancelRequest(withError: error)
            }
        }
    }
}
