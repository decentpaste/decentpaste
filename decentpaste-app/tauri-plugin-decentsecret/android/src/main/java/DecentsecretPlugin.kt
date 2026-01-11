package com.decentpaste.plugins.decentsecret

import android.app.Activity
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONArray
import org.json.JSONObject
import java.security.KeyStore
import java.util.concurrent.ConcurrentHashMap
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

private const val TAG = "DecentsecretPlugin"
private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
private const val KEY_ALIAS = "com.decentpaste.vault.key"
private const val SHARED_PREFS_NAME = "decentsecret_prefs"
private const val PREF_ENCRYPTED_SECRET = "encrypted_secret"
private const val PREF_IV = "encryption_iv"
private const val GCM_TAG_LENGTH = 128

@InvokeArg
class StoreSecretArgs {
    var secret: List<Int>? = null
}

@TauriPlugin
class DecentsecretPlugin(private val activity: Activity) : Plugin(activity) {

    // Thread-safe map for pending biometric operations
    private val pendingInvokes = ConcurrentHashMap<String, Invoke>()

    @Command
    fun checkAvailability(invoke: Invoke) {
        try {
            val biometricManager = BiometricManager.from(activity)
            val result = biometricManager.canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_STRONG)

            val ret = JSObject()
            when (result) {
                BiometricManager.BIOMETRIC_SUCCESS -> {
                    ret.put("available", true)
                    ret.put("method", "androidBiometric")
                    ret.put("unavailableReason", JSONObject.NULL)
                }
                BiometricManager.BIOMETRIC_ERROR_NO_HARDWARE -> {
                    ret.put("available", false)
                    ret.put("method", JSONObject.NULL)
                    ret.put("unavailableReason", "No biometric hardware available")
                }
                BiometricManager.BIOMETRIC_ERROR_HW_UNAVAILABLE -> {
                    ret.put("available", false)
                    ret.put("method", JSONObject.NULL)
                    ret.put("unavailableReason", "Biometric hardware temporarily unavailable")
                }
                BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED -> {
                    ret.put("available", false)
                    ret.put("method", JSONObject.NULL)
                    ret.put("unavailableReason", "NO_BIOMETRICS: No biometrics enrolled on this device")
                }
                BiometricManager.BIOMETRIC_ERROR_SECURITY_UPDATE_REQUIRED -> {
                    ret.put("available", false)
                    ret.put("method", JSONObject.NULL)
                    ret.put("unavailableReason", "Security update required for biometric authentication")
                }
                else -> {
                    ret.put("available", false)
                    ret.put("method", JSONObject.NULL)
                    ret.put("unavailableReason", "Unknown biometric status: $result")
                }
            }
            invoke.resolve(ret)
        } catch (e: Exception) {
            Log.e(TAG, "Error checking biometric availability", e)
            invoke.reject("Failed to check biometric availability: ${e.message}")
        }
    }

    @Command
    fun storeSecret(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(StoreSecretArgs::class.java)
            val secretList = args.secret ?: run {
                invoke.reject("NOT_AVAILABLE: No secret provided")
                return
            }

            // Convert List<Int> to ByteArray
            val secretBytes = secretList.map { it.toByte() }.toByteArray()

            // Create or get the biometric-protected key
            val secretKey = getOrCreateSecretKey()
            if (secretKey == null) {
                invoke.reject("NOT_AVAILABLE: Failed to create encryption key")
                return
            }

            // Store invoke for callback
            val invokeId = System.currentTimeMillis().toString()
            pendingInvokes[invokeId] = invoke

            // Initialize cipher for encryption
            val cipher = getCipher()
            cipher.init(Cipher.ENCRYPT_MODE, secretKey)

            // Show biometric prompt
            showBiometricPrompt(
                invokeId = invokeId,
                cipher = cipher,
                operation = BiometricOperation.STORE,
                secretBytes = secretBytes
            )
        } catch (e: KeyPermanentlyInvalidatedException) {
            Log.w(TAG, "Key invalidated due to biometric enrollment change")
            invoke.reject("BIOMETRIC_CHANGED: Key invalidated due to biometric enrollment change")
        } catch (e: Exception) {
            Log.e(TAG, "Error storing secret", e)
            invoke.reject("Failed to store secret: ${e.message}")
        }
    }

    @Command
    fun retrieveSecret(invoke: Invoke) {
        try {
            // Check if we have stored data
            val prefs = activity.getSharedPreferences(SHARED_PREFS_NAME, Activity.MODE_PRIVATE)
            val encryptedBase64 = prefs.getString(PREF_ENCRYPTED_SECRET, null)
            val ivBase64 = prefs.getString(PREF_IV, null)

            if (encryptedBase64 == null || ivBase64 == null) {
                invoke.reject("NOT_FOUND: No secret stored")
                return
            }

            // Get the existing key
            val secretKey = getExistingSecretKey()
            if (secretKey == null) {
                invoke.reject("NOT_FOUND: Encryption key not found")
                return
            }

            // Store invoke for callback
            val invokeId = System.currentTimeMillis().toString()
            pendingInvokes[invokeId] = invoke

            // Initialize cipher for decryption
            val iv = Base64.decode(ivBase64, Base64.NO_WRAP)
            val cipher = getCipher()
            cipher.init(Cipher.DECRYPT_MODE, secretKey, GCMParameterSpec(GCM_TAG_LENGTH, iv))

            // Show biometric prompt
            showBiometricPrompt(
                invokeId = invokeId,
                cipher = cipher,
                operation = BiometricOperation.RETRIEVE,
                encryptedData = Base64.decode(encryptedBase64, Base64.NO_WRAP)
            )
        } catch (e: KeyPermanentlyInvalidatedException) {
            Log.w(TAG, "Key invalidated due to biometric enrollment change")
            invoke.reject("BIOMETRIC_CHANGED: Key invalidated due to biometric enrollment change")
        } catch (e: Exception) {
            Log.e(TAG, "Error retrieving secret", e)
            invoke.reject("Failed to retrieve secret: ${e.message}")
        }
    }

    @Command
    fun deleteSecret(invoke: Invoke) {
        try {
            // Delete the key from KeyStore
            val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
            keyStore.load(null)
            if (keyStore.containsAlias(KEY_ALIAS)) {
                keyStore.deleteEntry(KEY_ALIAS)
                Log.i(TAG, "Deleted encryption key from KeyStore")
            }

            // Clear stored encrypted data
            val prefs = activity.getSharedPreferences(SHARED_PREFS_NAME, Activity.MODE_PRIVATE)
            prefs.edit()
                .remove(PREF_ENCRYPTED_SECRET)
                .remove(PREF_IV)
                .apply()

            Log.i(TAG, "Secret deleted successfully")
            invoke.resolve(JSObject())
        } catch (e: Exception) {
            Log.e(TAG, "Error deleting secret", e)
            invoke.reject("Failed to delete secret: ${e.message}")
        }
    }

    private fun getOrCreateSecretKey(): SecretKey? {
        val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
        keyStore.load(null)

        // Check if key already exists
        if (keyStore.containsAlias(KEY_ALIAS)) {
            return keyStore.getKey(KEY_ALIAS, null) as? SecretKey
        }

        // Generate a new biometric-protected key
        val keyGenerator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)

        val builder = KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setUserAuthenticationRequired(true)
            // CRITICAL: Invalidate key when biometric enrollment changes
            .setInvalidatedByBiometricEnrollment(true)

        // Set authentication parameters based on API level
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Android 11+: Use the new API with timeout=0 (every use requires auth)
            builder.setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
        } else {
            // Android 10 and below: Use legacy API
            @Suppress("DEPRECATION")
            builder.setUserAuthenticationValidityDurationSeconds(-1)
        }

        keyGenerator.init(builder.build())
        return keyGenerator.generateKey()
    }

    private fun getExistingSecretKey(): SecretKey? {
        val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
        keyStore.load(null)
        return if (keyStore.containsAlias(KEY_ALIAS)) {
            keyStore.getKey(KEY_ALIAS, null) as? SecretKey
        } else {
            null
        }
    }

    private fun getCipher(): Cipher {
        return Cipher.getInstance("AES/GCM/NoPadding")
    }

    private fun showBiometricPrompt(
        invokeId: String,
        cipher: Cipher,
        operation: BiometricOperation,
        secretBytes: ByteArray? = null,
        encryptedData: ByteArray? = null
    ) {
        val fragmentActivity = activity as? FragmentActivity
        if (fragmentActivity == null) {
            pendingInvokes.remove(invokeId)?.reject("NOT_AVAILABLE: Activity is not a FragmentActivity")
            return
        }

        val executor = ContextCompat.getMainExecutor(activity)

        val callback = object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                val invoke = pendingInvokes.remove(invokeId) ?: return

                try {
                    val authenticatedCipher = result.cryptoObject?.cipher
                    if (authenticatedCipher == null) {
                        invoke.reject("AUTH_FAILED: No cipher after authentication")
                        return
                    }

                    when (operation) {
                        BiometricOperation.STORE -> {
                            // Encrypt the secret
                            val encrypted = authenticatedCipher.doFinal(secretBytes)
                            val iv = authenticatedCipher.iv

                            // Store encrypted data and IV
                            val prefs = activity.getSharedPreferences(SHARED_PREFS_NAME, Activity.MODE_PRIVATE)
                            prefs.edit()
                                .putString(PREF_ENCRYPTED_SECRET, Base64.encodeToString(encrypted, Base64.NO_WRAP))
                                .putString(PREF_IV, Base64.encodeToString(iv, Base64.NO_WRAP))
                                .apply()

                            Log.i(TAG, "Secret stored successfully")
                            invoke.resolve(JSObject())
                        }
                        BiometricOperation.RETRIEVE -> {
                            // Decrypt the secret
                            val decrypted = authenticatedCipher.doFinal(encryptedData)

                            val ret = JSObject()
                            // Convert ByteArray to JSONArray for proper JSON serialization
                            // Using JSONArray ensures it serializes as [1,2,3] not "[1, 2, 3]"
                            val secretArray = JSONArray()
                            decrypted.forEach { secretArray.put(it.toInt() and 0xFF) }
                            ret.put("secret", secretArray)

                            Log.i(TAG, "Secret retrieved successfully")
                            invoke.resolve(ret)
                        }
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "Error in biometric callback", e)
                    invoke.reject("AUTH_FAILED: ${e.message}")
                }
            }

            override fun onAuthenticationFailed() {
                // Called when biometric didn't match - user can retry
                Log.w(TAG, "Biometric authentication failed (didn't match)")
            }

            override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                val invoke = pendingInvokes.remove(invokeId) ?: return

                val errorMsg = when (errorCode) {
                    BiometricPrompt.ERROR_USER_CANCELED,
                    BiometricPrompt.ERROR_NEGATIVE_BUTTON -> "USER_CANCELLED: $errString"
                    BiometricPrompt.ERROR_LOCKOUT,
                    BiometricPrompt.ERROR_LOCKOUT_PERMANENT -> "ACCESS_DENIED: $errString"
                    BiometricPrompt.ERROR_NO_BIOMETRICS -> "NO_BIOMETRICS: $errString"
                    else -> "AUTH_FAILED: $errString"
                }
                invoke.reject(errorMsg)
            }
        }

        val promptInfo = BiometricPrompt.PromptInfo.Builder()
            .setTitle("Authenticate")
            .setSubtitle("Verify your identity to access the vault")
            .setNegativeButtonText("Cancel")
            .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG)
            .build()

        // Must run on main thread
        activity.runOnUiThread {
            val biometricPrompt = BiometricPrompt(fragmentActivity, executor, callback)
            biometricPrompt.authenticate(promptInfo, BiometricPrompt.CryptoObject(cipher))
        }
    }

    private enum class BiometricOperation {
        STORE,
        RETRIEVE
    }
}
