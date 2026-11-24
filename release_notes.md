# 🚀 SupBooster v1.0.0 (Official Major Release)

Welcome to the **v1.0.0** milestone of **SupBooster** (`SupBoost.exe`)! This release brings significant architectural upgrades, full Steam Community Market integration, mobile authenticator alert handling, live Steam avatar integration with animated GIF support, store search autocomplete, and extensive UI/UX polish.

---

### 🛒 1. Authentic Steam Community Market Bulk Selling & 2FA Flow
- **Real Session Generation:** Authenticates real Steam Web Sessions directly from saved QR refresh tokens using Valve's official `/jwt/finalizelogin` and `/login/settoken` protocol.
- **Real Market Listing:** Dispatches actual `POST /market/sellitem/` requests with session cookies, asset IDs, and exact net pricing calculations.
- **Steam Guard Mobile Confirmation Alert:** If Steam requires mobile confirmation, SupBooster displays a prominent, unmistakable alert informing you that the confirmation request was dispatched to your phone's Steam Mobile Authenticator.
- **Auto-Cleanup:** Sold items are automatically removed from active selections and updated immediately.

---

### 🎨 2. Enhanced Themes & Contrast Guarantee
- **Light Theme Contrast Fix:** Selecting **Light Mode** automatically sets default text to high-contrast dark/black (`[25, 30, 36]`) with light panel surfaces, completely resolving any invisible text issues. Users can still customize colors freely.
- **Clean Modal Presentation:** Completely removed the heavy black screen dimming overlay. Confirmation modals and changelogs now blend seamlessly over your workspace.
- **Hardware Acrylic & Blur Vibrancy:** Windows 10 & 11 `window-vibrancy` support with customizable RGB tint, text, and accent color pickers.

---

### 👤 3. Steam Profile Pictures (PFP) & Animated GIFs
- **Live Steam Avatars in Top Bar:** All currently boosting accounts display their actual Steam profile pictures directly in the top status bar.
- **Full GIF Support:** Powered by `egui_extras` image loaders, animated Steam avatars and frames animate smoothly in real time.
- **Boosting Indicators:** Accounts feature glowing status indicators and real-time game counts.

---

### 🔍 4. Real-Time Store Search Autocomplete
- **Type-Ahead Discovery:** Typing in the "Search Steam Store" input (e.g. *counter*, *rust*, *witcher*) automatically queries Steam and displays matches with capsule thumbnails, title, and AppID without needing to click search.
- **One-Click Add:** Quickly add discovered games directly to your account's boosting library.

---

### ⚡ 5. Control Polish & Controls
- **Orange "Restart All" Button:** Added a prominent `🔄 Restart All` button alongside `⏹ Stop All` in the top header, allowing you to restart active boost sessions with a single click.
- **Single Navigation Tab Icons:** Fixed duplicated emoji icons across sidebar tabs (Dashboard, Active Drops, Inventory, Accounts, Telegram, Settings).
- **Responsive Vertical Layouts:** All game chips now stack cleanly in vertical cards with responsive scrolling, preventing any horizontal overflow.
- **Network Resilience:** Automatic failover to direct connections if a configured SOCKS5 proxy is unreachable, plus dynamic 75-item inventory chunking to eliminate Steam HTTP 429 rate limit errors.

---

### 📦 Binary Assets
- `SupBoost.exe`
- `SupBoost.v1.0.0.exe`
