package com.decentpaste.application

import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Build
import android.os.Bundle
import android.os.IBinder
import android.util.Log
import androidx.activity.enableEdgeToEdge

/**
 * Main activity for DecentPaste.
 *
 * This activity manages the foreground service lifecycle and provides
 * a bridge between Tauri/Rust and Android-specific functionality.
 */
class MainActivity : TauriActivity() {

    companion object {
        private const val TAG = "MainActivity"

        /**
         * Tracks whether the app is in foreground.
         * Used by ClipboardSyncService to decide whether to copy directly
         * or show a notification.
         */
        @Volatile
        var isInForeground: Boolean = false
            private set

        /**
         * Reference to the service for clipboard operations.
         * Accessed from Tauri plugin via JNI.
         */
        @Volatile
        var clipboardService: ClipboardSyncService? = null
            private set
    }

    private var serviceBound = false

    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            Log.d(TAG, "Service connected")
            val binder = service as ClipboardSyncService.LocalBinder
            clipboardService = binder.getService()
            serviceBound = true
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            Log.d(TAG, "Service disconnected")
            clipboardService = null
            serviceBound = false
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        Log.d(TAG, "MainActivity onCreate")

        // Start and bind to foreground service
        startClipboardSyncService()

        // Handle clipboard content from notification tap
        handleClipboardIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleClipboardIntent(intent)
    }

    private fun handleClipboardIntent(intent: Intent) {
        val clipboardContent = intent.getStringExtra(ClipboardSyncService.EXTRA_CLIPBOARD_CONTENT)
        if (clipboardContent != null) {
            Log.d(TAG, "Handling clipboard content from notification")
            copyToClipboard(clipboardContent)
        }
    }

    private fun copyToClipboard(content: String) {
        val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = ClipData.newPlainText("DecentPaste", content)
        clipboard.setPrimaryClip(clip)
        Log.d(TAG, "Copied ${content.length} chars to clipboard")
    }

    override fun onStart() {
        super.onStart()
        Log.d(TAG, "MainActivity onStart")
        isInForeground = true

        // Bind to service if not already bound
        if (!serviceBound) {
            Intent(this, ClipboardSyncService::class.java).also { intent ->
                bindService(intent, serviceConnection, Context.BIND_AUTO_CREATE)
            }
        }

        // If there's pending clipboard content, copy it now that we're in foreground
        ClipboardSyncService.pendingClipboardContent?.let { content ->
            copyToClipboard(content)
            ClipboardSyncService.pendingClipboardContent = null
        }
    }

    override fun onResume() {
        super.onResume()
        Log.d(TAG, "MainActivity onResume")
        isInForeground = true
    }

    override fun onPause() {
        super.onPause()
        Log.d(TAG, "MainActivity onPause")
        // Don't set isInForeground = false here, wait for onStop
    }

    override fun onStop() {
        super.onStop()
        Log.d(TAG, "MainActivity onStop")
        isInForeground = false
    }

    override fun onDestroy() {
        Log.d(TAG, "MainActivity onDestroy")
        // Unbind from service but don't stop it (it should keep running)
        if (serviceBound) {
            unbindService(serviceConnection)
            serviceBound = false
        }
        super.onDestroy()
    }

    private fun startClipboardSyncService() {
        Log.d(TAG, "Starting ClipboardSyncService")
        val serviceIntent = Intent(this, ClipboardSyncService::class.java)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(serviceIntent)
        } else {
            startService(serviceIntent)
        }

        // Bind to service for direct communication
        bindService(serviceIntent, serviceConnection, Context.BIND_AUTO_CREATE)
    }

    /**
     * Called from Tauri/Rust when clipboard is received from another device.
     * This method is invoked via JNI from the Rust side.
     *
     * @param content The clipboard content to copy
     * @param fromDevice The device name that sent the clipboard
     * @return true if copied directly (foreground), false if notification shown (background)
     */
    fun onClipboardReceived(content: String, fromDevice: String): Boolean {
        Log.d(TAG, "onClipboardReceived from $fromDevice (${content.length} chars)")

        return if (isInForeground) {
            // App is in foreground, copy directly
            copyToClipboard(content)
            true
        } else {
            // App is in background, show notification
            clipboardService?.showClipboardReceivedNotification(content, fromDevice)
            false
        }
    }

    /**
     * Stop the background service.
     * Called from settings if user wants to disable background sync.
     */
    fun stopClipboardSyncService() {
        Log.d(TAG, "Stopping ClipboardSyncService")
        if (serviceBound) {
            unbindService(serviceConnection)
            serviceBound = false
        }
        stopService(Intent(this, ClipboardSyncService::class.java))
    }
}
