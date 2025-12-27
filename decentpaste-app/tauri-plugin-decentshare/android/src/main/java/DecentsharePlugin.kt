package com.decentpaste.plugins.decentshare

import android.app.Activity
import android.content.Intent
import android.util.Log
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke

private const val TAG = "DecentsharePlugin"

/**
 * Argument class for the getPendingShare command.
 * Currently takes no arguments but needed for the invoke pattern.
 */
@InvokeArg
class GetPendingShareArgs

/**
 * Tauri plugin that handles Android share intents.
 *
 * When a user shares text from another app to DecentPaste:
 * 1. Android routes the SEND intent to our activity-alias (defined in AndroidManifest.xml)
 * 2. The activity-alias targets MainActivity, which triggers onNewIntent()
 * 3. We extract the shared text and emit a "share-received" event to the frontend
 * 4. The frontend handles vault unlock (if needed) and sharing to peers
 */
@TauriPlugin
class DecentsharePlugin(private val activity: Activity) : Plugin(activity) {

    /**
     * Stores the pending shared content until it's retrieved by the frontend.
     * This handles the case where the intent arrives before the webview is ready.
     */
    private var pendingShareContent: String? = null

    /**
     * Called when the activity receives a new intent (e.g., from share sheet).
     * This is the main entry point for handling shared content from other apps.
     */
    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)

        Log.d(TAG, "onNewIntent called with action: ${intent?.action}, type: ${intent?.type}")

        if (intent == null) return

        // Check if this is a SEND intent with plain text
        if (intent.action == Intent.ACTION_SEND && intent.type == "text/plain") {
            val sharedText = intent.getStringExtra(Intent.EXTRA_TEXT)

            if (sharedText.isNullOrEmpty()) {
                Log.w(TAG, "Received SEND intent but EXTRA_TEXT was null or empty")
                return
            }

            Log.i(TAG, "Received shared text (${sharedText.length} chars)")

            // Store for later retrieval and emit event
            pendingShareContent = sharedText

            // Emit event to frontend
            val payload = JSObject().apply {
                put("content", sharedText)
                put("timestamp", System.currentTimeMillis())
            }

            trigger("share-received", payload)
            Log.d(TAG, "Emitted share-received event")
        }
    }

    /**
     * Command to retrieve pending shared content.
     * Called by the frontend after initialization to check if there's pending content.
     * This handles the race condition where the intent arrives before the frontend is ready.
     */
    @Command
    fun getPendingShare(invoke: Invoke) {
        val content = pendingShareContent
        pendingShareContent = null  // Clear after retrieval

        val result = JSObject()
        if (content != null) {
            result.put("content", content)
            result.put("hasPending", true)
            Log.d(TAG, "Returning pending share content (${content.length} chars)")
        } else {
            result.put("content", null)
            result.put("hasPending", false)
        }

        invoke.resolve(result)
    }

    /**
     * Command to clear any pending shared content.
     * Called by the frontend after successfully processing the shared content.
     */
    @Command
    fun clearPendingShare(invoke: Invoke) {
        pendingShareContent = null
        Log.d(TAG, "Cleared pending share content")
        invoke.resolve(JSObject())
    }
}
