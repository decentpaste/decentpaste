import SwiftRs
import Tauri
import UIKit
import LocalAuthentication
import Security

// MARK: - Argument Types

class StoreSecretArgs: Decodable {
    let secret: [Int]
}

// MARK: - Constants

private let kServiceName = "com.decentpaste.vault"
private let kAccountName = "vault-key"

// MARK: - Plugin Implementation

class DecentsecretPlugin: Plugin {

    // MARK: - Check Availability

    @objc public func checkAvailability(_ invoke: Invoke) throws {
        let context = LAContext()
        var error: NSError?

        // Check if biometric authentication is available
        if context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error) {
            // Check the biometry type
            let biometryType = context.biometryType

            invoke.resolve([
                "available": true,
                "method": "iOSBiometric",
                "unavailableReason": NSNull()
            ])
        } else {
            var reason = "Biometric authentication not available"

            if let err = error {
                switch err.code {
                case LAError.biometryNotAvailable.rawValue:
                    reason = "NOT_AVAILABLE: No biometric hardware on this device"
                case LAError.biometryNotEnrolled.rawValue:
                    reason = "NO_BIOMETRICS: No biometrics enrolled on this device"
                case LAError.biometryLockout.rawValue:
                    reason = "ACCESS_DENIED: Biometry is locked out due to too many failed attempts"
                case LAError.passcodeNotSet.rawValue:
                    reason = "NOT_AVAILABLE: Device passcode not set"
                default:
                    reason = "NOT_AVAILABLE: \(err.localizedDescription)"
                }
            }

            invoke.resolve([
                "available": false,
                "method": NSNull(),
                "unavailableReason": reason
            ])
        }
    }

    // MARK: - Store Secret

    @objc public func storeSecret(_ invoke: Invoke) throws {
        do {
            let args = try invoke.parseArgs(StoreSecretArgs.self)
            let secretData = Data(args.secret.map { UInt8($0 & 0xFF) })

            // Delete any existing item first
            deleteFromKeychain()

            // Create access control with biometric protection
            var accessControlError: Unmanaged<CFError>?
            guard let accessControl = SecAccessControlCreateWithFlags(
                kCFAllocatorDefault,
                kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
                [.biometryCurrentSet], // CRITICAL: Invalidate on biometric change
                &accessControlError
            ) else {
                let errorMsg = accessControlError?.takeRetainedValue().localizedDescription ?? "Unknown error"
                invoke.reject("NOT_AVAILABLE: Failed to create access control: \(errorMsg)")
                return
            }

            // Build the keychain query
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: kServiceName,
                kSecAttrAccount as String: kAccountName,
                kSecValueData as String: secretData,
                kSecAttrAccessControl as String: accessControl,
                kSecUseAuthenticationContext as String: LAContext()
            ]

            // Add to keychain
            let status = SecItemAdd(query as CFDictionary, nil)

            if status == errSecSuccess {
                invoke.resolve([:])
            } else {
                let errorMsg = securityErrorMessage(status)
                invoke.reject("Failed to store secret: \(errorMsg)")
            }

        } catch {
            invoke.reject("Failed to parse arguments: \(error.localizedDescription)")
        }
    }

    // MARK: - Retrieve Secret

    @objc public func retrieveSecret(_ invoke: Invoke) throws {
        let context = LAContext()
        context.localizedReason = "Access your vault"

        // Build the keychain query
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: kServiceName,
            kSecAttrAccount as String: kAccountName,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecUseAuthenticationContext as String: context,
            kSecUseAuthenticationUI as String: kSecUseAuthenticationUIAllow
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        switch status {
        case errSecSuccess:
            if let data = result as? Data {
                let secretArray = data.map { Int($0) }
                invoke.resolve(["secret": secretArray])
            } else {
                invoke.reject("NOT_FOUND: Secret data is corrupted")
            }

        case errSecItemNotFound:
            invoke.reject("NOT_FOUND: No secret stored")

        case errSecUserCanceled:
            invoke.reject("USER_CANCELLED: User cancelled authentication")

        case errSecAuthFailed:
            invoke.reject("AUTH_FAILED: Authentication failed")

        case -25293: // errSecInvalidKeychain - often indicates biometric change
            invoke.reject("BIOMETRIC_CHANGED: Key invalidated due to biometric enrollment change")

        default:
            // Check if it's a biometric invalidation error
            let errorMsg = securityErrorMessage(status)
            if errorMsg.contains("invalidat") || errorMsg.contains("LAError") {
                invoke.reject("BIOMETRIC_CHANGED: \(errorMsg)")
            } else {
                invoke.reject("Failed to retrieve secret: \(errorMsg)")
            }
        }
    }

    // MARK: - Delete Secret

    @objc public func deleteSecret(_ invoke: Invoke) throws {
        deleteFromKeychain()
        invoke.resolve([:])
    }

    // MARK: - Private Helpers

    private func deleteFromKeychain() {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: kServiceName,
            kSecAttrAccount as String: kAccountName
        ]
        SecItemDelete(query as CFDictionary)
    }

    private func securityErrorMessage(_ status: OSStatus) -> String {
        if let errorMessage = SecCopyErrorMessageString(status, nil) as String? {
            return errorMessage
        }
        return "Security error: \(status)"
    }
}

// MARK: - Plugin Initialization

@_cdecl("init_plugin_decentsecret")
func initPlugin() -> Plugin {
    return DecentsecretPlugin()
}
