---
title: "Offline Clipboard Sync: A Hash-First Protocol for P2P Message Delivery"
date: 2026-01-14
description: "How we implemented offline message delivery for DecentPaste using a hash-first sync protocol that scales from text to files while maintaining E2E encryption."
---

DecentPaste uses libp2p's gossipsub for clipboard synchronization - a fire-and-forget broadcast model. When a user copies text, it's encrypted for each paired peer and published to the network. Online peers receive it immediately, but offline peers never do.

This became a real issue:

- Mobile devices background apps, killing network connections
- Desktop users close the app and miss clipboard updates
- Users expect "eventual consistency" like Apple's Universal Clipboard

The expectation is simple: **if I was offline and my paired devices had clipboard changes, I should receive them when I reconnect.**

## Design Requirements

I needed a solution that:
- Delivers the latest message from all paired peers on reconnection
- Handles multiple senders (peer A and B both send while C is offline)
- Preserves chronological order
- Deduplicates content (same content shouldn't be applied twice)
- Uses minimal memory and network bandwidth
- Future-proofs for file transfers (10MB+ payloads)
- Maintains security - only paired peers participate

## Architectural Decisions

### 1. Always Buffer for All Paired Peers

The first decision was deceptively simple: **buffer messages for all paired peers, regardless of their online status.**

Why not track who's online and only buffer for offline peers?

Because of a race condition:

```
1. PeerA copies text
2. PeerB is in `ready_peers` (appears online)
3. PeerB publishes message
4. PeerB's app closes mid-transmission
5. PeerB never receives the message
```

By always buffering, we don't need to track online/offline state. The buffer catches messages that were "missed" - whether due to being offline, mid-disconnect, or network issues.

**Trade-off:** We might duplicate messages (peer receives live broadcast AND sync). This is solved with hash-based deduplication.

**Memory cost:** ~1KB per peer (1 message × ~10 peers = ~10KB). Negligible.

```rust
// lib.rs lines 456-491
for peer in paired_peers.iter() {
    let msg = ClipboardMessage { /* ... */ };

    // 1. Send via gossipsub (fire-and-forget)
    network_cmd_tx.send(NetworkCommand::BroadcastClipboard {
        message: msg.clone(),
    }).await;

    // 2. ALWAYS buffer for this peer
    // This handles the race condition where peer goes offline
    // mid-transmission. Sync ensures eventual delivery.
    state.store_buffered_message(&peer.peer_id, msg).await;
}
```

### 2. Hash-First Sync Protocol

This was the most critical architectural decision. Instead of sending all buffered messages immediately, I implemented a two-phase protocol:

```
1. Requester sends SyncRequest
2. Responder sends HashListResponse [hash1, hash2, hash3]
3. Requester compares against local history
4. Requester sends ContentRequest for missing hashes only
5. Responder sends ContentResponse with full message
```

Why this complexity? **Future-proofing for file transfers.**

Current clipboard messages are ~1KB text - trivial to send in bulk. But when we add file support, messages will be 10MB+. Sending files we already have would be wasteful.

The hash-first protocol:
- Avoids sending duplicate content
- Scales to large payloads (files)
- Hash lists are tiny (bytes)

```rust
// protocol.rs lines 18-35
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    Request { peer_id: String },
    HashListResponse { hashes: Vec<MessageHash> },
    ContentRequest { hash: String },
    ContentResponse { message: ClipboardMessage },
}
```

### 3. Per-Recipient Buffering

I needed to decide: buffer by sender (who sent the message) or recipient (who missed it)?

**Per-recipient** won:

```rust
// Key = peer_id (who missed the message)
// Value = messages WE sent that THEY missed
message_buffers: HashMap<String, Vec<ClipboardMessage>>
```

When PeerA requests sync from PeerB:
- PeerB looks up `buffer["peer_a_id"]`
- Returns only messages PeerB sent that PeerA missed

The alternative (per-sender buffering) would require filtering logic: "return all messages from all senders, except ones originally from PeerA (we don't want to echo their own message back to them)."

Per-recipient is semantically correct and simpler.

### 4. TTL with Lazy Cleanup

Messages expire after 5 minutes (`SYNC_TTL_SECONDS`). I chose lazy cleanup - filter expired messages only when the buffer is accessed (during sync), not via a periodic cleanup task.

Why no periodic task? **Simplicity.** The memory is negligible (~10KB). Messages are consumed on sync. Expired messages filtered on next access. No need for background tasks.

```rust
// state.rs lines 203-220
pub async fn get_buffer_for_peer(&self, peer_id: &str) -> Vec<ClipboardMessage> {
    let buffers = self.message_buffers.read().await;
    let now = Utc::now();
    let ttl = Duration::seconds(SYNC_TTL_SECONDS);

    buffers.get(peer_id)
        .map(|msgs| {
            msgs.iter()
                .filter(|msg| now.signed_duration_since(msg.timestamp) < ttl)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}
```

### 5. Content Hash for Deduplication

I use `content_hash` (not message UUID) for deduplication:

- UUID identifies a single broadcast event - same content, different IDs
- Hash identifies the clipboard content - same content, same hash

This ensures the same clipboard content is never applied twice, even if broadcast multiple times or received via both live broadcast and sync.

```rust
// lib.rs lines 1315-1324
let already_has = {
    let history = state.clipboard_history.read().await;
    history.iter().any(|entry| entry.content_hash == hash)
};

if !already_has {
    // Apply the message
} else {
    debug!("Synced message already in history (deduplicated)");
}
```

## Implementation Details

### Protocol Layer

The sync protocol integrates cleanly with the existing `ProtocolMessage` enum:

```rust
// protocol.rs line 14
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolMessage {
    Pairing(PairingMessage),
    Clipboard(ClipboardMessage),
    Sync(SyncMessage),  // New sync protocol
    // ...
}
```

### State Management

Message buffers are stored as `Arc<RwLock<HashMap<String, Vec<ClipboardMessage>>>>`:

```rust
// state.rs lines 97-105
/// Per-recipient buffering: we store messages WE sent that THEY missed.
/// Key = peer_id of the recipient (who missed the message)
/// Value = messages we sent that they should receive on reconnection
pub message_buffers: Arc<RwLock<HashMap<String, Vec<ClipboardMessage>>>>,
```

Helper methods manage buffer operations:
- `store_buffered_message()` - Add to buffer, truncate to max size
- `get_buffer_for_peer()` - Get buffer with TTL filtering
- `find_message_for_peer_by_hash()` - Find message by hash for a specific peer
- `remove_buffered_message_for_peer()` - Remove after delivery

### Bi-Directional Sync

Sync is triggered automatically on both sides when a peer becomes ready (`PeerReady` event):

```rust
// lib.rs lines 719-735
if state.is_peer_paired(peer_id).await {
    debug!("Peer {} is paired and ready, requesting sync", peer_id);
    network_cmd_tx.send(NetworkCommand::RequestSync {
        peer_id: peer_id.clone(),
    }).await;
}
```

This creates bi-directional sync:
- PeerA requests sync from PeerB
- PeerB requests sync from PeerA
- Both sides exchange missed messages

## Security Model

All sync operations require both peers to be paired:

```rust
// Every sync handler checks this
if !state.is_peer_paired(&peer_id).await {
    warn!("Ignoring ... from unpaired peer: {}", peer_id);
    continue;
}
```

Why this matters:
- Unpaired peers can't decrypt messages anyway (no shared secret)
- Prevents information leakage (buffer shouldn't be exposed to untrusted peers)
- Limits attack surface to known, authenticated devices

Each message is pre-encrypted per-recipient, so the buffer stores encrypted messages ready for delivery without re-encryption.

## Edge Cases

### Race Condition on Disconnect

Scenario: Peer disconnects mid-transmission
- **Solution:** Always buffer for all peers
- **Trade-off:** Accept small message loss window (seconds to minutes)
- **Alternative rejected:** Heartbeat/ping-pong (complex, still has race conditions)

### Buffer Overflow with Rapid Copying

Scenario: User copies 3 messages while peer is offline. Which one do they receive?
- **Solution:** Buffer size = 1, only latest message synced
- **Rationale:** User expectation is "I copied 'Final Version', why is my clipboard showing 'Draft 1'?"
- **Future:** User-configurable buffer size (1, 5, 10)

### Expired Message in Buffer

Scenario: Peer offline for 10+ minutes, message expired (TTL = 5 min)
- **Solution:** Filter expired messages during sync
- **Behavior:** Peer receives empty response, no messages delivered

### Sync During Active Copying

Scenario: PeerA copies "Message 1", then PeerB comes online requesting sync, then PeerA copies "Message 2" before sync completes
- **Solution:** Both messages delivered, sorted by timestamp
- **Deduplication:** Hash-based prevents duplicates

## Lessons Learned

### Simplicity Wins

I considered tracking online/offline state, but it added complexity and still had race conditions. Always buffering + deduplication was simpler and more reliable.

### Future-Proofing Matters

The hash-first protocol seems overkill for 1KB text messages. But for file transfers (10MB+), it's essential. The extra round-trip is negligible now, pays dividends later.

### Security by Default

I verified paired status at every sync operation. Could I have skipped some checks? Yes. But security bugs are hard to find and catastrophic when missed.

### Hash-First as a Pattern

This protocol could be reused for other sync operations (settings sync, device state sync). The pattern is general: exchange metadata, request only what's missing.

## Trade-offs

| Decision | Trade-off | Mitigation |
|----------|-----------|------------|
| Always buffer | Duplicate delivery possible | Hash-based deduplication |
| Hash-first protocol | Extra round-trip | Overhead negligible for text |
| Per-recipient buffering | More memory (vs shared buffer) | Negligible (< 10KB) |
| TTL = 5 min | Misses very old messages | Acceptable for clipboard use |
| Buffer size = 1 | Only latest synced | Future: user-configurable |

## Conclusion

The clipboard history sync implementation adds offline message delivery to DecentPaste's P2P clipboard sharing. The design prioritizes simplicity, reliability, and future-proofing:

- **Always buffer** for all paired peers - handles race conditions
- **Hash-first protocol** - scales to file transfers
- **Per-recipient buffering** - semantically correct
- **TTL with lazy cleanup** - no periodic tasks
- **Security by default** - paired peer verification at every step

The result is a robust sync protocol that handles offline scenarios gracefully while maintaining end-to-end encryption and minimal resource usage.

## Future Work

- User-configurable buffer size (1, 5, 10 messages)
- File transfer support using the hash-first protocol
- Sync progress indicator in UI
- Extended TTL option for users who leave devices offline longer

---

*DecentPaste is a cross-platform clipboard sharing app built with Tauri, Rust, and libp2p. It uses E2E encryption (X25519 + AES-256-GCM) and runs entirely on your local network. [View on GitHub](https://github.com/decentpaste/decentpaste).*
