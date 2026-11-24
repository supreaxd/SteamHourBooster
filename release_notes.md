# 🚀 SupBooster v1.1.0 (Official Release)

Welcome to **v1.1.0** of **SupBooster** (`SupBoost.exe`)! This release introduces dynamic non-Steam game title auto-replacements (`(hours)`, `(games)`, `(time)`, `(date)`), periodic 10-minute title auto-refresh, instant live hot-reload on account edits, permanent Windows DPAPI hardware encryption, instant Steam QR code login generation, and GitHub Atom feed & redirect fallback for update checking.

---

### 🎮 1. Dynamic Steam Non-Steam Game Titles & Auto-Refresh
- **Dynamic Variable Tags:** Use `(hours)` or `{hours}` to display real-time boosted hours (e.g. `14.2h`), `(games)` or `{games}` to display the number of active games (e.g. `14`), and `(time)` for current time (HH:MM).
- **10-Minute Auto-Refresh:** Non-Steam game presence automatically re-evaluates and syncs to Steam every 10 minutes so hours and clock times stay accurate in friends lists.
- **Instant Hot-Reload:** Editing an account's custom status/title or game list instantly updates the running boosting session without having to restart boosting.

---

### 📱 2. Steam Mobile QR Code Login Fixed
- **Instant QR Generation:** Resolved the issue where QR codes did not load or spin indefinitely. QR authentication sessions now initialize immediately upon opening the modal.
- **Live Polling & Token Storing:** Seamless scanning with Steam Mobile App and automatic DPAPI credential encryption.

---

### 🔒 3. Permanent Windows DPAPI Hardware Security
- **Enforced Security:** Windows DPAPI hardware encryption is now permanently enabled across all accounts and tokens without requiring manual checkboxes.

---

### 🚀 4. Seamless Update System & Sidebar Changelogs
- **Prominent Update Indicator:** When a new version is published, an orange `🚀 Changelogs (v1.1.0)` pill appears directly ABOVE the sidebar's `Changelogs` link.
- **Unmetered Atom & Redirect Fallback:** Update checks utilize GitHub's Atom feed and redirect mechanisms to bypass strict unauthenticated GitHub REST API rate limits (HTTP 429/403).

---

### 🎨 5. Controls & Theme Polish
- **Orange Restart All:** Easily restart all accounts with the dedicated top-header button.
- **Light Theme Contrast Guarantee:** Default dark text ensures perfect visibility when switching to Light theme.
- **Clean Modals:** Removed screen dimming for modern translucent window overlay.

---

### 📦 Binary Assets
- `SupBoost.exe`
- `SupBoost.v1.1.0.exe`
- `SteamBooster.exe`
