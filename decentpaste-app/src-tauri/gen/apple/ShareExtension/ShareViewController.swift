import UIKit
import UniformTypeIdentifiers
import os.log

// MARK: - Constants

/// App Group identifier - MUST match DecentsharePlugin.swift and Xcode configuration
private let appGroupIdentifier = "group.com.decentpaste.application"

/// UserDefaults key - MUST match DecentsharePlugin.swift
private let pendingShareKey = "pendingShareContent"

// MARK: - ShareViewController

/// Share extension view controller that receives shared content from the iOS share sheet.
///
/// ## Overview
/// This extension appears when users share text from any iOS app. It:
/// 1. Extracts the shared text from the extension context
/// 2. Saves it to App Groups UserDefaults (shared with main app)
/// 3. Shows a confirmation card with instructions
/// 4. Waits for user to tap "Done" to dismiss
///
/// ## Data Flow
/// ```
/// User shares text → iOS share sheet → ShareViewController
///                                            ↓
///                               Save to App Groups UserDefaults
///                                            ↓
///                               Show confirmation "Content Saved!"
///                                            ↓
///                               User taps "Done" to dismiss
///                                            ↓
///                               User opens DecentPaste app manually
///                                            ↓
///                               Main app reads via getPendingShare()
/// ```
///
/// ## Why Manual App Opening?
/// iOS share extensions run in a sandboxed process and cannot reliably open
/// the containing app. The shared content is saved to App Groups, and the
/// main app will detect and process it when opened via visibility change.
///
/// ## Memory Constraints
/// iOS share extensions have a ~120MB memory limit. This implementation is
/// lightweight and should complete well within the 30-second timeout.
class ShareViewController: UIViewController {

    private let logger = Logger(subsystem: "com.decentpaste.application.ShareExtension", category: "ShareViewController")

    /// Track if content was saved successfully (for background tap dismissal)
    private var contentSaved = false

    // MARK: - UI Elements

    /// Main card container with white/dark background
    private lazy var cardContainer: UIView = {
        let view = UIView()
        view.backgroundColor = UIColor.systemBackground
        view.layer.cornerRadius = 20
        view.layer.shadowColor = UIColor.black.cgColor
        view.layer.shadowOffset = CGSize(width: 0, height: 4)
        view.layer.shadowRadius = 12
        view.layer.shadowOpacity = 0.15
        view.translatesAutoresizingMaskIntoConstraints = false
        return view
    }()

    /// Success checkmark icon
    private lazy var checkmarkImageView: UIImageView = {
        let config = UIImage.SymbolConfiguration(pointSize: 56, weight: .medium)
        let image = UIImage(systemName: "checkmark.circle.fill", withConfiguration: config)
        let imageView = UIImageView(image: image)
        imageView.tintColor = UIColor.systemGreen
        imageView.translatesAutoresizingMaskIntoConstraints = false
        return imageView
    }()

    /// Title label "Content Saved!"
    private lazy var titleLabel: UILabel = {
        let label = UILabel()
        label.text = "Content Saved!"
        label.textColor = .label
        label.textAlignment = .center
        label.font = .systemFont(ofSize: 22, weight: .bold)
        label.translatesAutoresizingMaskIntoConstraints = false
        return label
    }()

    /// Instruction label explaining next steps
    private lazy var instructionLabel: UILabel = {
        let label = UILabel()
        label.text = "Open DecentPaste to sync\nwith your devices."
        label.textColor = .secondaryLabel
        label.textAlignment = .center
        label.font = .systemFont(ofSize: 15, weight: .regular)
        label.numberOfLines = 0
        label.translatesAutoresizingMaskIntoConstraints = false
        return label
    }()

    /// Done button to dismiss the extension
    private lazy var doneButton: UIButton = {
        let button = UIButton(type: .system)
        button.setTitle("Done", for: .normal)
        button.titleLabel?.font = .systemFont(ofSize: 17, weight: .semibold)
        button.backgroundColor = UIColor.systemBlue
        button.setTitleColor(.white, for: .normal)
        button.layer.cornerRadius = 12
        button.translatesAutoresizingMaskIntoConstraints = false
        button.addTarget(self, action: #selector(doneButtonTapped), for: .touchUpInside)
        return button
    }()

    // MARK: - Lifecycle

    override func viewDidLoad() {
        super.viewDidLoad()

        // Semi-transparent background with tap-to-dismiss
        view.backgroundColor = UIColor.black.withAlphaComponent(0.5)

        let tapGesture = UITapGestureRecognizer(target: self, action: #selector(backgroundTapped))
        tapGesture.delegate = self
        view.addGestureRecognizer(tapGesture)

        // Setup UI
        setupUI()

        // Process shared content
        handleSharedContent()
    }

    // MARK: - UI Setup

    private func setupUI() {
        // Add card container
        view.addSubview(cardContainer)

        // Add elements to card
        cardContainer.addSubview(checkmarkImageView)
        cardContainer.addSubview(titleLabel)
        cardContainer.addSubview(instructionLabel)
        cardContainer.addSubview(doneButton)

        // Card container constraints (centered, fixed width)
        NSLayoutConstraint.activate([
            cardContainer.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            cardContainer.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            cardContainer.widthAnchor.constraint(equalToConstant: 280)
        ])

        // Checkmark constraints
        NSLayoutConstraint.activate([
            checkmarkImageView.topAnchor.constraint(equalTo: cardContainer.topAnchor, constant: 32),
            checkmarkImageView.centerXAnchor.constraint(equalTo: cardContainer.centerXAnchor),
            checkmarkImageView.widthAnchor.constraint(equalToConstant: 56),
            checkmarkImageView.heightAnchor.constraint(equalToConstant: 56)
        ])

        // Title label constraints
        NSLayoutConstraint.activate([
            titleLabel.topAnchor.constraint(equalTo: checkmarkImageView.bottomAnchor, constant: 16),
            titleLabel.leadingAnchor.constraint(equalTo: cardContainer.leadingAnchor, constant: 24),
            titleLabel.trailingAnchor.constraint(equalTo: cardContainer.trailingAnchor, constant: -24)
        ])

        // Instruction label constraints
        NSLayoutConstraint.activate([
            instructionLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 8),
            instructionLabel.leadingAnchor.constraint(equalTo: cardContainer.leadingAnchor, constant: 24),
            instructionLabel.trailingAnchor.constraint(equalTo: cardContainer.trailingAnchor, constant: -24)
        ])

        // Done button constraints
        NSLayoutConstraint.activate([
            doneButton.topAnchor.constraint(equalTo: instructionLabel.bottomAnchor, constant: 24),
            doneButton.leadingAnchor.constraint(equalTo: cardContainer.leadingAnchor, constant: 20),
            doneButton.trailingAnchor.constraint(equalTo: cardContainer.trailingAnchor, constant: -20),
            doneButton.heightAnchor.constraint(equalToConstant: 50),
            doneButton.bottomAnchor.constraint(equalTo: cardContainer.bottomAnchor, constant: -24)
        ])

        // Initially hidden for animation
        cardContainer.alpha = 0
        cardContainer.transform = CGAffineTransform(translationX: 0, y: 30)
    }

    private func showSuccessUI() {
        // Animate card appearance with spring
        UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0.5) {
            self.cardContainer.alpha = 1
            self.cardContainer.transform = .identity
        }
    }

    private func showErrorUI(message: String) {
        // Update UI for error state
        let config = UIImage.SymbolConfiguration(pointSize: 56, weight: .medium)
        checkmarkImageView.image = UIImage(systemName: "xmark.circle.fill", withConfiguration: config)
        checkmarkImageView.tintColor = .systemRed
        titleLabel.text = "Something went wrong"
        instructionLabel.text = message

        // Animate card appearance
        UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0.5) {
            self.cardContainer.alpha = 1
            self.cardContainer.transform = .identity
        }
    }

    // MARK: - Actions

    @objc private func doneButtonTapped() {
        dismissExtension(success: contentSaved)
    }

    @objc private func backgroundTapped() {
        // Only dismiss via background tap if content was saved
        if contentSaved {
            dismissExtension(success: true)
        }
    }

    // MARK: - Share Handling

    private func handleSharedContent() {
        guard let extensionItem = extensionContext?.inputItems.first as? NSExtensionItem,
              let attachments = extensionItem.attachments else {
            logger.error("No extension items or attachments found")
            showErrorUI(message: "No content found to share.")
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
        showErrorUI(message: "Only text content is supported.")
    }

    private func loadText(from itemProvider: NSItemProvider, typeIdentifier: String) {
        itemProvider.loadItem(forTypeIdentifier: typeIdentifier, options: nil) { [weak self] item, error in
            guard let self = self else { return }

            if let error = error {
                self.logger.error("Failed to load item: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self.showErrorUI(message: "Failed to load shared content.")
                }
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
                DispatchQueue.main.async {
                    self.showErrorUI(message: "The shared content is empty.")
                }
                return
            }

            self.logger.info("Received shared text (\(text.count) characters)")

            // Save to App Groups
            self.savePendingShare(text)
            self.contentSaved = true

            // Show success UI
            DispatchQueue.main.async {
                self.showSuccessUI()
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

    // MARK: - Extension Lifecycle

    private func dismissExtension(success: Bool) {
        // Animate out
        UIView.animate(withDuration: 0.25, animations: {
            self.cardContainer.alpha = 0
            self.cardContainer.transform = CGAffineTransform(translationX: 0, y: 30)
            self.view.backgroundColor = UIColor.black.withAlphaComponent(0)
        }) { _ in
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

// MARK: - UIGestureRecognizerDelegate

extension ShareViewController: UIGestureRecognizerDelegate {
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        // Only handle taps outside the card
        let location = touch.location(in: view)
        return !cardContainer.frame.contains(location)
    }
}
