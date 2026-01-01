package com.decentpaste.plugins.decentshare

import android.app.Activity
import android.content.Intent
import android.util.Log
import android.webkit.WebView
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
 * 3. We extract the shared text and store it for the frontend to retrieve
 * 4. The frontend handles vault unlock (if needed) and sharing to peers
 *
 * Note: We use a command-based approach (getPendingShare) rather than events
 * to avoid race conditions where both the event and command could fire.
 */
@TauriPlugin
class DecentsharePlugin(private val activity: Activity) : Plugin(activity) {

    /**
     * Stores the pending shared content until it's retrieved by the frontend.
     * This handles the case where the intent arrives before the webview is ready.
     *
     * Thread-safety: Uses @Volatile for visibility across threads (onNewIntent runs
     * on the activity thread, commands run on Tauri's IPC thread).
     */
    @Volatile
    private var pendingShareContent: String? = null

    /**
     * Called when the plugin is loaded into the web view.
     * Check for initial launch intent (cold start via share sheet).
     *
     * IMPORTANT: This handles the case where the app was fully killed and launched
     * via a share intent. In this case, onNewIntent() is NOT called - the intent
     * is available via activity.intent instead.
     */
    override fun load(webView: WebView) {
        super.load(webView)

        Log.d(TAG, "load() called, checking initial intent")

        // Check if app was launched via share intent (cold start)
        handleShareIntent(activity.intent)
    }

    /**
     * Called when the activity receives a new intent while already running (warm start).
     * This handles the case where the app is in the background and receives a share intent.
     */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)

        Log.d(TAG, "onNewIntent called with action: ${intent.action}, type: ${intent.type}")

        handleShareIntent(intent)
    }

    /**
     * Shared helper to process share intents from both cold start and warm start.
     * Thread-safe: Only writes to @Volatile pendingShareContent.
     *
     * @param intent The intent to check for shared content
     */
    private fun handleShareIntent(intent: Intent?) {
        if (intent == null) {
            Log.d(TAG, "handleShareIntent: intent is null")
            return
        }

        // Check if this is a SEND intent with plain text
        if (intent.action != Intent.ACTION_SEND || intent.type != "text/plain") {
            Log.d(TAG, "handleShareIntent: Not a text share intent (action=${intent.action}, type=${intent.type})")
            return
        }

        val sharedText = intent.getStringExtra(Intent.EXTRA_TEXT)

        if (sharedText.isNullOrEmpty()) {
            Log.w(TAG, "Received SEND intent but EXTRA_TEXT was null or empty")
            return
        }

        Log.i(TAG, "Received shared text (${sharedText.length} chars)")

        // Store for later retrieval via getPendingShare command
        // We don't emit an event here to avoid race conditions where both
        // the event listener and getPendingShare could process the same content
        pendingShareContent = sharedText
        Log.d(TAG, "Stored pending share content for retrieval")
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
