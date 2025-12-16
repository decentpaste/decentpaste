package com.decentpaste.application

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Binder
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import androidx.core.app.NotificationCompat

/**
 * Foreground service that keeps the app alive in background for clipboard sync.
 *
 * Android kills background apps aggressively. This service runs with a persistent
 * notification to prevent the OS from killing our network connections.
 *
 * IMPORTANT: This service does NOT monitor clipboard in background (Android 10+ blocks this).
 * It only keeps the network alive so we can RECEIVE clipboard from other devices.
 * When clipboard arrives, we show a notification - user taps it to copy to clipboard.
 */
class ClipboardSyncService : Service() {

    companion object {
        private const val TAG = "ClipboardSyncService"
        private const val CHANNEL_ID = "clipboard_sync_channel"
        private const val CLIPBOARD_CHANNEL_ID = "clipboard_notification_channel"
        private const val NOTIFICATION_ID = 1
        private const val CLIPBOARD_NOTIFICATION_ID = 2
        private const val WAKE_LOCK_TAG = "DecentPaste::ClipboardSync"

        // Action for clipboard notification tap
        const val ACTION_COPY_CLIPBOARD = "com.decentpaste.application.COPY_CLIPBOARD"
        const val EXTRA_CLIPBOARD_CONTENT = "clipboard_content"

        // Pending clipboard content (to be copied when user taps notification)
        @Volatile
        var pendingClipboardContent: String? = null
    }

    private var wakeLock: PowerManager.WakeLock? = null
    private val binder = LocalBinder()

    inner class LocalBinder : Binder() {
        fun getService(): ClipboardSyncService = this@ClipboardSyncService
    }

    override fun onBind(intent: Intent?): IBinder = binder

    override fun onCreate() {
        super.onCreate()
        Log.d(TAG, "ClipboardSyncService created")
        createNotificationChannels()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d(TAG, "ClipboardSyncService onStartCommand, action: ${intent?.action}")

        // Handle clipboard copy action from notification
        if (intent?.action == ACTION_COPY_CLIPBOARD) {
            val content = intent.getStringExtra(EXTRA_CLIPBOARD_CONTENT)
                ?: pendingClipboardContent
            if (content != null) {
                copyToClipboard(content)
                cancelClipboardNotification()
            }
            return START_STICKY
        }

        // Start as foreground service
        startForeground(NOTIFICATION_ID, createForegroundNotification())
        acquireWakeLock()

        // START_STICKY: Restart service if killed by system
        return START_STICKY
    }

    override fun onDestroy() {
        Log.d(TAG, "ClipboardSyncService destroyed")
        releaseWakeLock()
        super.onDestroy()
    }

    private fun createNotificationChannels() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val notificationManager = getSystemService(NotificationManager::class.java)

            // Channel for foreground service (silent, low importance)
            val syncChannel = NotificationChannel(
                CHANNEL_ID,
                "Clipboard Sync",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Keeps DecentPaste running for clipboard sync"
                setShowBadge(false)
            }
            notificationManager.createNotificationChannel(syncChannel)

            // Channel for clipboard received notifications (audible, high importance)
            val clipboardChannel = NotificationChannel(
                CLIPBOARD_CHANNEL_ID,
                "Clipboard Received",
                NotificationManager.IMPORTANCE_HIGH
            ).apply {
                description = "Notifications when clipboard content is received from other devices"
            }
            notificationManager.createNotificationChannel(clipboardChannel)
        }
    }

    private fun createForegroundNotification(): Notification {
        val intent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
        val pendingIntent = PendingIntent.getActivity(
            this, 0, intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("DecentPaste")
            .setContentText("Syncing clipboard in background")
            .setSmallIcon(android.R.drawable.ic_menu_share)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .build()
    }

    private fun acquireWakeLock() {
        if (wakeLock == null) {
            val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
            wakeLock = powerManager.newWakeLock(
                PowerManager.PARTIAL_WAKE_LOCK,
                WAKE_LOCK_TAG
            ).apply {
                acquire(10 * 60 * 1000L) // 10 minutes timeout, will be re-acquired
            }
            Log.d(TAG, "Wake lock acquired")
        }
    }

    private fun releaseWakeLock() {
        wakeLock?.let {
            if (it.isHeld) {
                it.release()
                Log.d(TAG, "Wake lock released")
            }
        }
        wakeLock = null
    }

    /**
     * Called from Tauri/Rust when clipboard content is received from another device.
     * Since we can't write to clipboard in background (Android 10+), we show a notification.
     * User taps the notification to copy the content.
     */
    fun showClipboardReceivedNotification(content: String, fromDevice: String) {
        Log.d(TAG, "Showing clipboard notification from: $fromDevice")

        // Store content for later copy
        pendingClipboardContent = content

        // Create intent for when user taps notification
        val copyIntent = Intent(this, ClipboardSyncService::class.java).apply {
            action = ACTION_COPY_CLIPBOARD
            putExtra(EXTRA_CLIPBOARD_CONTENT, content)
        }
        val copyPendingIntent = PendingIntent.getService(
            this, 0, copyIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Also open app intent
        val openAppIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
            putExtra(EXTRA_CLIPBOARD_CONTENT, content)
        }
        val openAppPendingIntent = PendingIntent.getActivity(
            this, 1, openAppIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Truncate content for notification display
        val displayContent = if (content.length > 100) {
            content.take(100) + "..."
        } else {
            content
        }

        val notification = NotificationCompat.Builder(this, CLIPBOARD_CHANNEL_ID)
            .setContentTitle("Clipboard from $fromDevice")
            .setContentText(displayContent)
            .setSmallIcon(android.R.drawable.ic_menu_agenda)
            .setContentIntent(openAppPendingIntent)
            .setAutoCancel(true)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_MESSAGE)
            .addAction(
                android.R.drawable.ic_menu_save,
                "Copy",
                copyPendingIntent
            )
            .setStyle(NotificationCompat.BigTextStyle().bigText(displayContent))
            .build()

        val notificationManager = getSystemService(NotificationManager::class.java)
        notificationManager.notify(CLIPBOARD_NOTIFICATION_ID, notification)
    }

    private fun copyToClipboard(content: String) {
        Log.d(TAG, "Copying content to clipboard (${content.length} chars)")
        val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = ClipData.newPlainText("DecentPaste", content)
        clipboard.setPrimaryClip(clip)
    }

    private fun cancelClipboardNotification() {
        val notificationManager = getSystemService(NotificationManager::class.java)
        notificationManager.cancel(CLIPBOARD_NOTIFICATION_ID)
        pendingClipboardContent = null
    }

    /**
     * Try to copy to clipboard if app is in foreground.
     * Returns true if successful, false if in background (need to show notification instead).
     */
    fun tryCopyToClipboardOrNotify(content: String, fromDevice: String): Boolean {
        // Check if app is in foreground by checking if MainActivity is resumed
        return if (MainActivity.isInForeground) {
            copyToClipboard(content)
            true
        } else {
            showClipboardReceivedNotification(content, fromDevice)
            false
        }
    }
}
