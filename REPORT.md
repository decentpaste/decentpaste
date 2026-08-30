# End-to-end test run on a disposable desktop

DecentPaste is a peer-to-peer app. Its interesting behaviour only exists *between* two
running instances, which makes it awkward to test: unit tests can't tell you whether a
clipboard entry actually crossed the wire, and one machine can't easily run two peers.

This is the report of a full end-to-end run performed inside a throwaway
[uremot](https://uremot.com) desktop instance — clone, build, run two paired peers, break
things on purpose, fix three of the results, and verify the fixes in the same environment.

![Two DecentPaste peers on one disposable desktop](img/01-hero.png)

*Left: the host driving the test. Middle and right: two independent DecentPaste instances.
Note the badges — `Local` means the entry originated on that device, a device-name badge
means it arrived from the peer. That is the only honest proof a message crossed the network,
because both instances share one X11 clipboard.*

---

## Environment

| | |
|---|---|
| Sandbox | UremotOS container, Debian 13 |
| Toolchain | rustc/cargo 1.98, Node 22.23, Yarn 1.22, webkit2gtk-4.1 2.52 |
| App | DecentPaste 0.8.1 — Tauri v2, libp2p 0.56 (mDNS + gossipsub + request-response) |
| Codebase under test | 7,016 lines of Rust across 28 files, plus the TypeScript frontend |

### Running two peers on one host

Three things have to be isolated, and none of them are obvious from the symptoms:

1. **Vault + identity** — `HOME=/path/to/peerB`. Each vault holds its own libp2p keypair.
2. **DBus session** — `dbus-run-session --`. `tauri-plugin-single-instance` claims a
   session-bus name from the app identifier; a private bus sidesteps it with no code change.
3. **libp2p port** — `network/swarm.rs` hardcodes `/ip4/0.0.0.0/tcp/31773` (deliberately, so
   cached peer addresses survive restarts). Both instances otherwise fight over one socket,
   and the symptom — `Dial error: Unexpected peer ID` — looks like a crypto bug.

The port override used for this run was **test scaffolding only** and is not part of the
accompanying code changes.

---

## Findings

13 issues, each reproduced before it was written down.

![Findings dashboard](img/03-dash.png)

### Critical

**1. Clipboard content over ~18KB never syncs, and the UI reports success.**
Both `clipboard/monitor.rs` and `commands.rs` advertise a 1MB limit. The real ceiling was
**18,218 bytes OK / 18,250 bytes fail**, found by binary search. gossipsub's default
`max_transmit_size` is 64KiB, and serde_json serialises `encrypted_content: Vec<u8>` as a
JSON array of decimal numbers — roughly 3.6 wire bytes per payload byte.

The silence is the real defect: `broadcast_count` counts successful *channel sends*, not
successful *publishes*, so the entry still enters local history and `clipboard-sent` still
fires.

![The bug, with the original logs](img/02-bug.png)

**2. Sync can stop permanently between two connected peers.** *(open — not fixed here)*
After a burst of large clipboard items, gossipsub delivery dies in both directions and never
recovers. Everything else looks healthy: TCP alive (yamux ping/pong), `identify` still
exchanging device names, `publish()` returning `Ok`. The receiver logs nothing.

```
A: Broadcast clipboard message: 250a358e…      <- app thinks it sent
   libp2p: Published message message_id=…      <- libp2p thinks it sent
B: (nothing)                                   <- never arrives
   HEARTBEAT: Mesh low. Topic contains: 0 needs: 6
   Peer couldn't consume messages: FailedMessages { publish: 1, timeout: 1 }
A: WARN Peer 12D3KooW… is slow
```

Reproduced deterministically 3×, intermittently later. Survives 10+ minutes and dozens of
probes; only an app restart recovers it. `gossipsub::Event::SlowPeer` is handled in
`swarm.rs` with a bare `warn!` and nothing else.

A candidate fix (re-add the explicit peer and force a topic re-subscribe on `SlowPeer`) was
built and tested — **it did not restore delivery**, so it is deliberately not proposed here.
Recovery likely needs a connection-level reset.

**3. Locking the vault doesn't stop syncing, and loses everything received while locked.**
`lock_vault` cleared the vault key and nothing else. Verified live with both vaults locked:
peer B broadcast, peer A received and decrypted behind its lock screen, and both logged
`Cannot flush clipboard history: vault not open`. After a lock/restart cycle, ~12 minutes of
history was gone.

### Medium

4. **No listener-failure handling.** `swarm.rs` handles `NewListenAddr` but has no
   `ListenerClosed`/`ListenerError` arm — they fall into `_ => {}`. Combined with the
   hardcoded port, a bind failure means the app runs with no networking and says nothing.
5. **`SERVICES_STARTED` never resets.** After `reset_vault` + re-onboarding in the same
   process, `start_network_services` returns early, leaving networking on the *old* libp2p
   keypair. It also means a dead network stack can never be restarted in-process.
6. **Echo-guard ordering race.** The receive path wrote the system clipboard *before*
   registering the hash with the monitor; the 500ms poll can land in between.
7. **`PeerLost` fires for still-connected peers.** `mdns::Event::Expired` emits it with no
   connection check, and the mDNS TTL is 60s, so a working paired device can vanish from the
   UI. Relatedly, `Discovered` dials unconditionally on every rediscovery.

### Security hardening

8. **The pairing PIN provides no MITM protection.** `generate_pin()` is
   `rng.random_range(0..1_000_000)`, transmitted inside `PairingChallenge` and never bound to
   the ECDH keys. A relaying attacker forwards the PIN unchanged while substituting its own
   X25519 public keys, and both users see matching digits. Proper numeric comparison derives
   the digits from a hash of *both* public keys. `PairingResponse.pin_hash` exists in the
   protocol and is never written or read anywhere.
9. **Raw ECDH output used directly as the AES-256 key** — `derive_shared_secret` returns
   `diffie_hellman().as_bytes()` with no HKDF.
10. **No unlock rate limiting**, and the frontend accepts 4-digit PINs. Argon2id is the only
    brake.

### Minor

11. `ClipboardEntry::preview` byte-slices a `String` — panics on a multibyte boundary.
    Currently unreachable (no callers), but a landmine.
12. `truncate()` in `utils/dom.ts` slices at 120 UTF-16 units, splitting emoji surrogate pairs.
13. `SYNC_MAX_BUFFER_SIZE = 1` means offline sync recovers only the single most recent item,
    despite the hash-list protocol implying more.

### Tested clean

UTF-8/emoji/ZWJ/multiline round-trips byte-exact (SHA-256 verified); both sync directions;
echo prevention under normal timing; consecutive-duplicate dedup; oversized payloads rejected
without corrupting state.

---

## Fixes in this branch

![Before and after](img/04-fix.png)

Three of the findings are fixed here — #1 (with the error surfacing), #3 (which resolves #5),
and #6. Each was verified in the same sandbox that found it.

![Live re-test](img/05-verify.png)

Measured sync latency after the fix: **8.3ms min, 10.1ms median**, 115.8ms for a 500KB
payload.

## Reproducing

```bash
# two peers on one Linux host
setsid env HOME=~/peerB DECENTPASTE_P2P_PORT=31774 \
  dbus-run-session -- ./target/debug/decentpaste-app > peerB.log 2>&1 &
```

Then drive the X11 clipboard and diff both logs — `Broadcast clipboard message` on the
sender, `Received clipboard message` on the receiver. The UI alone cannot prove sync happened,
because both instances observe the same clipboard.
