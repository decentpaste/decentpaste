---
title: How to Implement iOS Share Extension in Tauri v2
date: 2026-01-07
description: A practical guide to adding iOS Share Extension support to your Tauri v2 app, including the gotchas I learned the hard way.
---

I recently added iOS Share Extension support to my Tauri v2 app and wanted to share what I learned. If you're trying to add "Share to..." functionality for iOS, this guide should save you some time.

**Fun fact**: I've probably worked with every programming language on the planet - except Swift. So this was my initiation ceremony.

I used AI assistants throughout this project. Early on, they led me down dead ends because I didn't understand iOS limitations. But once I took time to learn how iOS Share Extensions actually work and came back with a concrete plan, AI became an excellent implementation partner - helping me write Swift code, create automation scripts, and handle edge cases efficiently.

---

## Why I Needed This

I'm building [DecentPaste](https://github.com/decentpaste/decentpaste) - a P2P clipboard sharing app. Think Apple's Universal Clipboard, but for everyone. When someone shares text to my app, I need to:

1. Receive the shared content
2. Reconnect to paired devices (connections drop when backgrounded on mobile)
3. Encrypt and broadcast to peers

So the app *has* to open after sharing. That's where the fun begins...

---

## The Plot Twist: iOS vs Android

On Android, the app just... receives the intent. Done. One Kotlin file.

iOS? Hold my beer.

| Aspect       | Android       | iOS                              |
|--------------|---------------|----------------------------------|
| Process      | Same process  | Separate process (extension!)    |
| Data Passing | Intent extras | App Groups (shared UserDefaults) |
| App Opening  | Automatic     | Manual (sandbox restriction)     |
| Memory Limit | Normal        | ~120MB for extension             |

**The kicker**: iOS Share Extensions **cannot open the containing app**. The sandbox blocks it. All those Stack Overflow answers about URL schemes and responder chains? They don't work reliably. Trust me, I tried for days.

---

## The Solution

```
User shares text → ShareExtension (separate process)
        ↓
Save to App Groups UserDefaults
        ↓
Show "Content Saved!" with Done button
        ↓
User opens app manually
        ↓
App reads from App Groups → reconnects → sends to peers
```

Not as slick as Android's automatic flow, but it works reliably.

---

## The Key Parts

### 1. Extension saves to App Groups

```swift
// ShareViewController.swift - the essential bit
private func savePendingShare(_ content: String) {
    let defaults = UserDefaults(suiteName: "group.com.yourapp.identifier")
    defaults?.set(content, forKey: "pendingShareContent")
    defaults?.synchronize()
}
```

### 2. Main app reads from App Groups

```swift
// DecentsharePlugin.swift - atomic get-and-clear
@objc public func getPendingShare(_ invoke: Invoke) {
    let defaults = UserDefaults(suiteName: "group.com.yourapp.identifier")
    let content = defaults?.string(forKey: "pendingShareContent")

    if content != nil {
        defaults?.removeObject(forKey: "pendingShareContent")
    }

    invoke.resolve(["content": content, "hasPending": content != nil])
}
```

### 3. Frontend checks on visibility change

```typescript
document.addEventListener('visibilitychange', async () => {
    if (document.visibilityState === 'visible') {
        const result = await getPendingShare();
        if (result.hasPending) {
            await handleSharedContent(result.content);
        }
    }
});
```

### 4. Automation script (the real MVP)

Since `gen/apple/` gets nuked every time you run `tauri ios init`, you need a script that rebuilds the ShareExtension target. Ours copies Swift files, creates entitlements, and runs xcodegen.

```bash
yarn tauri ios init
./tauri-plugin-decentshare/scripts/setup-ios-share-extension.sh
open src-tauri/gen/apple/yourapp.xcodeproj
```

[Full implementation code](https://github.com/decentpaste/decentpaste/tree/dev/decentpaste-app/tauri-plugin-decentshare)

---

## Quick Checklist

- [ ] Create App Group in Apple Developer Portal
- [ ] Add App Groups capability to **both** targets in Xcode
- [ ] Use same App Group ID everywhere
- [ ] Test on real device (Simulator is unreliable for extensions)
- [ ] Install xcodegen: `brew install xcodegen`

---

## Don't Do This

❌ **Deep links from extension** - Sandbox blocks it, don't waste time

❌ **Edit `gen/apple/` directly** - It gets regenerated

❌ **Auto-dismiss the extension** - Users need to know what happened

❌ **Skip real device testing** - Simulator lies

---

## Lessons Learned

I'm an Android user developing for iOS - I had no idea about sandbox limitations. When I asked AI why my URL scheme wasn't working, it kept suggesting fixes that assumed the approach was valid: "add a delay", "try the responder chain", "check your Info.plist".

The problem wasn't my code - it was the fundamental approach. AI tools work within your assumptions rather than challenge them. If you don't know what you don't know, AI can lead you deeper into a dead end.

**My advice**: When stuck on an unfamiliar platform, step back and question the entire approach. Read platform docs directly. Test on real devices early. This is a good reminder of this AI journey we are on.

---

## Links

- [Full plugin implementation](https://github.com/decentpaste/decentpaste/tree/dev/decentpaste-app/tauri-plugin-decentshare)
- [Setup script](https://github.com/decentpaste/decentpaste/blob/dev/decentpaste-app/tauri-plugin-decentshare/scripts/setup-ios-share-extension.sh)
- [DecentPaste](https://github.com/decentpaste/decentpaste)
- [The website](https://decentpaste.com/)

Happy to learn more if you have better ideas!
