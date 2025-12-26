package com.decentpaste.plugins.shareintent

import android.app.Activity
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONObject

@TauriPlugin
class ShareIntentPlugin(private val activity: Activity) : Plugin(activity) {

    companion object {
        private const val TAG = "ShareIntentPlugin"
    }

    // Store pending content for retrieval by Rust/JS
    private var pendingContent: String? = null

    // Track the last processed intent to prevent duplicate processing on config changes
    // We use a hash of content + timestamp to allow same content to be shared multiple times
    private var lastProcessedIntentHash: Int = 0

    // Store webView reference for event emission
    private var webViewRef: WebView? = null

    /**
     * Called when the plugin is loaded and WebView is ready.
     * Check for share intent that launched the app (cold start).
     */
    override fun load(webView: WebView) {
        super.load(webView)
        webViewRef = webView
        Log.i(TAG, "ShareIntentPlugin loaded")
        Log.i(TAG, "Checking activity intent: action=${activity.intent?.action}, type=${activity.intent?.type}")
        // Handle intent that started the activity
        handleIntent(activity.intent)
    }

    /**
     * Called when the activity receives a new intent while running (warm start).
     * This happens when user shares to the app while it's in background.
     */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        Log.i(TAG, "onNewIntent received: action=${intent.action}, type=${intent.type}")
        handleIntent(intent)
    }

    /**
     * Emit an event to the frontend via JavaScript evaluation.
     * Uses window.__TAURI__.event.emit for Tauri v2 compatibility.
     */
    private fun emitEvent(eventName: String, content: String, source: String) {
        val webView = webViewRef ?: run {
            Log.e(TAG, "WebView not available for event emission")
            return
        }

        // Escape content for JavaScript string
        val escapedContent = content
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\t", "\\t")

        // JavaScript to emit the event using Tauri's event system
        val js = """
            (function() {
                if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.emit) {
                    window.__TAURI__.event.emit('$eventName', { content: "$escapedContent", source: "$source" });
                    console.log('[ShareIntentPlugin] Event emitted via __TAURI__.event.emit');
                } else if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
                    // Fallback for older Tauri versions
                    window.__TAURI_INTERNALS__.invoke('plugin:event|emit', { event: '$eventName', payload: { content: "$escapedContent", source: "$source" } });
                    console.log('[ShareIntentPlugin] Event emitted via __TAURI_INTERNALS__.invoke');
                } else {
                    // Last resort: dispatch a custom DOM event
                    window.dispatchEvent(new CustomEvent('$eventName', { detail: { content: "$escapedContent", source: "$source" } }));
                    console.log('[ShareIntentPlugin] Event emitted via dispatchEvent');
                }
            })();
        """.trimIndent()

        // Run on UI thread
        Handler(Looper.getMainLooper()).post {
            webView.evaluateJavascript(js) { result ->
                Log.d(TAG, "JavaScript evaluation result: $result")
            }
        }
    }

    /**
     * Process an intent and extract shared text if present.
     */
    private fun handleIntent(intent: Intent?) {
        if (intent == null) {
            Log.d(TAG, "handleIntent: intent is null")
            return
        }

        Log.d(TAG, "handleIntent: action=${intent.action}, type=${intent.type}")

        // Check if this is a share intent
        if (intent.action == Intent.ACTION_SEND && intent.type == "text/plain") {
            val sharedText = intent.getStringExtra(Intent.EXTRA_TEXT)
            Log.i(TAG, "Share intent detected! Text: ${sharedText?.take(50)}...")

            if (!sharedText.isNullOrEmpty()) {
                // Use Intent's identity hash to detect if this is the same intent being reprocessed
                // (e.g., on configuration changes). This allows the same text to be shared multiple times.
                val intentHash = System.identityHashCode(intent)

                if (intentHash == lastProcessedIntentHash) {
                    Log.d(TAG, "Skipping already-processed intent (same object)")
                    return
                }
                lastProcessedIntentHash = intentHash

                // Store for retrieval via command
                pendingContent = sharedText
                Log.i(TAG, "Stored pending content (${sharedText.length} chars)")

                Log.i(TAG, "Emitting share-intent-received event via JavaScript")
                // Use a slight delay to ensure WebView is fully ready
                Handler(Looper.getMainLooper()).postDelayed({
                    emitEvent("share-intent-received", sharedText, "android")
                }, 500)
            }
        } else {
            Log.d(TAG, "Not a share intent, skipping")
        }

        // Clear the intent action to prevent re-processing on config changes
        intent.action = null
    }

    /**
     * Command to get pending share content.
     * Called from Rust/JS to retrieve and consume the pending content.
     */
    @Command
    fun getPendingContent(invoke: Invoke) {
        val result = JSObject()

        if (pendingContent != null) {
            result.put("content", pendingContent)
            result.put("source", "android")
            pendingContent = null  // Consume the content
        } else {
            result.put("content", JSONObject.NULL)
            result.put("source", JSONObject.NULL)
        }

        invoke.resolve(result)
    }

    /**
     * Command to check if there's pending content without consuming it.
     */
    @Command
    fun hasPendingContent(invoke: Invoke) {
        val result = JSObject()
        result.put("hasPending", pendingContent != null)
        invoke.resolve(result)
    }

    /**
     * Command to clear pending content without processing.
     */
    @Command
    fun clearPendingContent(invoke: Invoke) {
        pendingContent = null
        lastProcessedIntentHash = 0
        invoke.resolve(JSObject())
    }
}
