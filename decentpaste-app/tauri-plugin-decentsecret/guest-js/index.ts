/**
 * Tauri plugin for secure secret storage across all platforms.
 *
 * Provides a unified API for storing secrets using:
 * - Android: AndroidKeyStore with BiometricPrompt (TEE/StrongBox)
 * - iOS: Keychain with Secure Enclave
 * - macOS: Keychain Access
 * - Windows: Credential Manager
 * - Linux: Secret Service API (GNOME Keyring, KWallet)
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * The method used for secure secret storage.
 */
export type SecretStorageMethod =
  | 'androidBiometric'
  | 'iOSBiometric'
  | 'macOSKeychain'
  | 'windowsCredentialManager'
  | 'linuxSecretService';

/**
 * Status of secure secret storage availability.
 */
export interface SecretStorageStatus {
  /** Whether secure storage is available and can be used. */
  available: boolean;
  /** The method that will be used (if available). */
  method: SecretStorageMethod | null;
  /** Why secure storage is unavailable (if not available). */
  unavailableReason: string | null;
}

/**
 * Response from retrieving a secret.
 */
interface RetrieveSecretResponse {
  secret: number[];
}

/**
 * Check what secure storage capabilities are available on this platform.
 *
 * @returns Status including availability, method, and reason if unavailable.
 *
 * @example
 * ```typescript
 * const status = await checkAvailability();
 * if (status.available) {
 *   console.log(`Using ${status.method} for secure storage`);
 * } else {
 *   console.log(`Falling back to PIN: ${status.unavailableReason}`);
 * }
 * ```
 */
export async function checkAvailability(): Promise<SecretStorageStatus> {
  return await invoke<SecretStorageStatus>('plugin:decentsecret|check_availability');
}

/**
 * Store a secret in platform secure storage.
 *
 * - **Android**: Shows BiometricPrompt, encrypts with TEE key
 * - **iOS**: Stores in Keychain with Secure Enclave protection
 * - **Desktop**: Stores in OS keyring (no prompt, session-based)
 *
 * @param secret - The secret bytes to store (typically a 32-byte vault key)
 * @throws Error if storage fails or user cancels authentication
 *
 * @example
 * ```typescript
 * // Store a 32-byte vault key
 * const vaultKey = new Uint8Array(32);
 * crypto.getRandomValues(vaultKey);
 * await storeSecret(Array.from(vaultKey));
 * ```
 */
export async function storeSecret(secret: number[]): Promise<void> {
  await invoke('plugin:decentsecret|store_secret', {
    request: { secret },
  });
}

/**
 * Retrieve the secret from platform secure storage.
 *
 * - **Android**: Shows BiometricPrompt, decrypts with TEE key
 * - **iOS**: Shows Face ID/Touch ID, retrieves from Secure Enclave
 * - **Desktop**: Retrieves from OS keyring (no prompt, session-based)
 *
 * @returns The stored secret bytes
 * @throws Error if secret not found, authentication fails, or biometrics changed
 *
 * @example
 * ```typescript
 * try {
 *   const secret = await retrieveSecret();
 *   const vaultKey = new Uint8Array(secret);
 *   // Use vaultKey to decrypt vault...
 * } catch (error) {
 *   if (error.message.includes('BIOMETRIC_CHANGED')) {
 *     // User's biometrics changed, vault is inaccessible
 *     showResetVaultDialog();
 *   }
 * }
 * ```
 */
export async function retrieveSecret(): Promise<number[]> {
  const response = await invoke<RetrieveSecretResponse>('plugin:decentsecret|retrieve_secret');
  return response.secret;
}

/**
 * Delete the secret from platform secure storage.
 *
 * Used during vault reset or when the user wants to switch auth methods.
 * This operation is idempotent - it won't fail if the secret doesn't exist.
 *
 * @example
 * ```typescript
 * // During vault reset
 * await deleteSecret();
 * await destroyVault();
 * ```
 */
export async function deleteSecret(): Promise<void> {
  await invoke('plugin:decentsecret|delete_secret');
}
