# ⚡ SupBooster v0.4.0 (Major Release)

The application has been officially rebranded to **SupBooster** (`SupBoost.exe`), featuring a completely redesigned modern desktop interface built from the ground up in Rust. This release brings substantial performance gains, a hardware-accelerated transparent visual aesthetic, automated card drop farming, Steam Community Market integration, and extensive UI polish.

---

### 🎨 1. Theme Engine & Transparent Glass (Acrylic & Blur)
- **4 Distinct Modes:** Dark, Light, System Default, and **Transparent Glass (Hardware Blur & Acrylic)**.
- **Windows 11 & 10 Hardware Vibrancy:** Native `window-vibrancy` integration with mica and acrylic effects, paired with graceful fallback themes on older hardware.
- **Interactive Color Palette:** Full RGB tint, text, and accent customization with instant presets and single-click default restore.
- **High-Contrast Badges & Readability:** Completely revamped status indicators (Boosting/Online/Offline) with high-contrast borders and readable foreground typography, eliminating washed-out or neon green artifacts.
- **No Overflow Vertical Layout:** Game lists on both Dashboard and Accounts views now stack cleanly in vertical cards with responsive scrolling, preventing horizontal overflow or truncated cards regardless of window size.

---

### 🎴 2. Active Drops & Sequential Auto-Switch
- **Automatic Badge Scanning:** Automatically scans the user's Steam badge library to detect all owned games with remaining card drops (`remaining_cards > 0`).
- **Active Drops Tab:** Displays all drop-eligible games with badges indicating cards remaining, plus one-click `+ Add to Boost List` and `Add All Active Drops` buttons.
- **Auto-Switch Mechanism:** The booster constantly monitors drop status; as soon as a game's remaining drops reach 0, it removes that game from the active session and automatically queues up the next available drop game.
- **Network Resilience:** Added automatic direct connection fallback if an account's configured SOCKS5/HTTP proxy is unreachable, ensuring drop scanning never fails silently.

---

### 💰 3. Steam Inventory & Bulk Market Selling
- **Game-Grouped Inventory:** Items and trading cards are grouped neatly under collapsing game headers with item counts.
- **CDN Cached Thumbnails:** Fast asynchronous loading and caching of high-resolution Steam CDN item graphics.
- **Live Market Pricing & Fee Calculation:** Real-time pricing with clear separation between buyer price and net seller payout (factoring in the 15% Steam cut).
- **Selling Strategies:**
  - `Quick Sell (Lowest Buy Order)`
  - `Undercut (0.01 Below Lowest Listing)`
- **Robust Steam Rate Limiting:** Dynamic chunking (`count=75`) and browser-compliant referer headers to prevent Steam HTTP 429 rate limit errors.
- **Bulk Confirmation Modal:** Detailed breakdown showing total items, estimated gross/net earnings, and 2FA mobile confirmation support (`identity_secret`).

---

### 📱 4. Dedicated Telegram Notifications
- **Card Drops Exclusively:** Stripped away bloat and unnecessary background alerts. Telegram bot notifications now focus strictly on reporting newly acquired Steam Trading Cards along with their real-time estimated market value.
- **Integrated Control Panel:** Direct configuration of Bot Token, Chat ID, instant connection diagnostics, and notification toggles directly in the Settings menu.

---

### 🚀 5. Core Performance & System Polish
- **100% Rust:** Memory safety, blazing fast startup, and lightweight footprint (~50 MB RAM).
- **Silent Update Notification:** Subtle update badge and clean English Changelogs dialog with data quota warnings before downloading.
- **English as Default:** All menus, dashboard cards, modal dialogs, empty states, and system notifications now feature clean, idiomatic English terminology.
- **Standardized Executable:** Distributed as `SupBoost.exe` and `SupBoost.v0.4.0.exe`.
