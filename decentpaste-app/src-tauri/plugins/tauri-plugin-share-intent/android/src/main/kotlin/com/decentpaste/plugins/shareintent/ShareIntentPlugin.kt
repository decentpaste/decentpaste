package com.decentpaste.plugins.shareintent

import android.app.Activity
import android.content.Intent
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

    // Track if we've already emitted for this content (prevent duplicates)
    private var lastEmittedContent: String? = null

    /**
     * Called when the plugin is loaded and WebView is ready.
     * Check for share intent that launched the app (cold start).
     */
    override fun load(webView: WebView) {
        super.load(webView)
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
                // Store for retrieval
                pendingContent = sharedText
                Log.i(TAG, "Stored pending content (${sharedText.length} chars)")

                // Only emit if this is new content (prevent duplicate events)
                if (sharedText != lastEmittedContent) {
                    lastEmittedContent = sharedText

                    // Emit event to frontend
                    val payload = JSObject()
                    payload.put("content", sharedText)
                    payload.put("source", "android")
                    Log.i(TAG, "Emitting share-intent-received event")
                    trigger("share-intent-received", payload)
                } else {
                    Log.d(TAG, "Skipping duplicate content emission")
                }
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
        lastEmittedContent = null
        invoke.resolve(JSObject())
    }
}
