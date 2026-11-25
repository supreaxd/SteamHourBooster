# 🚀 SupBooster v1.2.2 (Telegram Bot Overhaul)

Welcome to **v1.2.2** of **SupBooster** (`SupBoost.exe`)! This release completely rewrites the Telegram Bot command system with 15 trilingual commands, inline action buttons, and QR image delivery.

---

### 🤖 1. Complete Telegram Bot Rewrite (EN / TR / RU)
- **All 15 Commands Implemented:** `/start`, `/help`, `/status`, `/start_boost`, `/stop_boost`, `/restart`, `/drops`, `/games`, `/add_game`, `/remove_game`, `/auto_drops`, `/accounts`, `/switch`, `/drop_notify`, and `/qr`.
- **Trilingual Responses:** Every command reply displays English, Turkish, and Russian together in a single clean message.
- **Interactive Inline Buttons:** `/status` includes per-account `Start` / `Stop` buttons and `/drops` includes one-tap `Boost <Game>` buttons with full `callback_query` support.
- **QR Code Image Delivery:** `/qr` renders and sends the Steam QR login image directly in chat via `sendPhoto`.
- **Silent Chat ID Authorization:** Unauthorized Chat IDs are silently ignored without leaking responses.

---

# 🚀 SupBooster v1.2.1 (Maintenance & Usability Release)

Welcome to **v1.2.1** of **SupBooster** (`SupBoost.exe`)! This release addresses library browsing, batch boosting operations, and account lifecycle management.

---

### 🎮 1. Dedicated Library Scrolling & "＋ Add All to Boost"
- **Smooth & Persistent Scrolling:** Fixed the issue where adding games to boosting restricted vertical scrolling in the Account Owned Games (Library) view. The library now features a spacious scroll container with persistent scrollbars (`ScrollBarVisibility::AlwaysVisible`) and isolated scroll state (`id_salt`), allowing effortless scrolling through your entire library.
- **One-Click "＋ Add All to Boost":** Added a dedicated `＋ Add All to Boost` button next to `🔄 Refresh Games` in the library header. One click instantly queues all library games (or filtered search results) into your boosting list.
- **Boosting Games "✕ Clear All":** Added a `✕ Clear All` button in the Boosting Games section to quickly clear all queued games.

---

### 🗑️ 2. Account Deletion & Config Synchronization Fixed
- **Complete Deletion Lifecycle:** Fixed the bug where deleting an account (`🗑`) failed to remove it from the application and `config.json`.
- **Immediate Disk Persistence:** Deleting or saving an account immediately syncs to `config.json` on disk and updates runtime instances.
- **Clean Session Termination:** Deleting an active account automatically terminates its Steam CM connection and cleans up background tasks.

---

### ⏱️ 3. Dynamic Title Tags in Logs & UI
- **Live Tag Rendering:** Fixed logging so dynamic tags like `(hours)`, `(games)`, `(time)` display their actual computed values (e.g., `17.4h`, `21 games`) in logs on startup, presence hot-reloads, and 10-minute auto-refresh ticks.

---

# 🚀 SupBooster v1.2.0 (Official Release)

Welcome to **v1.2.0** of **SupBooster** (`SupBoost.exe`)! This release brings major fixes to the Steam Inventory & Bulk Market Seller engine and the Account Owned Games scanner, along with complete UI polish.

---

### 🛒 1. Real-Time Steam Market Price Engine (0.20 Fixed)
- **Live Price Querying:** Fixed the issue where all inventory items (CS2 skins, stickers, etc.) defaulted to a flat `$0.20` price. The engine now queries live Steam Community Market prices (`/market/priceoverview`) in the background.
- **Rarity & Type Fallback Estimation:** If an item's live price is still being fetched or temporarily rate-limited, items receive realistic baseline values based on skin rarity (Covert, Classified, Restricted, Mil-Spec), stickers (Glitter, Holo, Foil), cases, and foil cards instead of a fixed $0.20.
- **Accurate Bulk Market Listing:** Market sale transactions now list items at their true Steam Community Market value (or -$0.01 for Undercut strategy), preventing valuable items from being underpriced.

---

### 🎮 2. Account Owned Games Scanner Fixed
- **Multi-Source Library Discovery:** Fixed the issue where clicking "Refresh Games" showed 0 games found. The scanner now scrapes paginated Steam Community badges (`/badges/?p=1..4`), extracts games from inventory items, and checks active Steam CM sessions.
- **Instant Background UI Update:** Account Owned Games section now automatically updates and repaints immediately when the refresh task completes in the background.
- **One-Click Boost Addition:** Any owned game can be added directly to the boosting list with one click (`＋ Add to Boost`).

---

### 🔄 3. Infinite Reconnection Loop Resolved
- **Eliminated Free License Spam:** Removed invalid package license requests (`CMsgClientRequestFreeLicense`) on paid games (e.g. AppID 730 / CS2) that caused Steam CM WebSocket drops right after inventory checks.
- **Smart Presence Updates:** Replaced rapid 60-second presence packet resends with intelligent 10-minute title refreshes and state-change triggers.
- **Baseline Card Drop Fix:** Prevented false card drop alerts on startup when comparing against empty initial baseline.

---

### 🎨 4. Clean Single Tab Icons & 100% English UI
- **Single Tab Icons:** Removed duplicate icons on sidebar tabs.
- **Unified English Interface:** All badges, modals, confirmations, and settings are now standardized to clean English.

---

### 📦 Binary Assets
- `SupBoost.exe`
- `SupBoost.v1.2.0.exe`
- `SteamBooster.exe`
