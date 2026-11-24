# ⚡ SupBooster (SteamHourBooster)

A lightweight, feature-rich multi-account Steam hour booster and trading card idle tool with a modern desktop GUI built in **Rust** using **egui**, **window-vibrancy**, and **steam-vent**.

---

## ✨ Features

- **Transparent Glass & Theme Engine**: 4 distinct theme modes (Dark, Light, System, Transparent Glass / Blur) with Windows 11 Acrylic/Mica hardware vibrancy and customizable RGB tint, text, and accent color pickers.
- **Active Drops & Auto-Switch**: Scans Steam badge data to identify games with remaining card drops (`remaining_cards > 0`), shows remaining card drop badges, and automatically swaps to the next eligible game once a game's drops hit 0.
- **Inventory Inspection & Bulk Selling**: Game-grouped collapsing headers, Steam CDN item thumbnails, live Steam Community Market prices, gross & net fee calculation (15% Steam/game cut), floating sticky action bar, Quick Sell / Undercut strategies, and 2FA mobile confirmations.
- **Built with 100% Rust**: Engineered in pure Rust for blazing-fast performance, memory safety, and ultra-low CPU/RAM footprint (~50 MB RAM).
- **Multi-Account Boosting**: Boost hours simultaneously across multiple Steam accounts.
- **Multi-Game Boosting**: Select and boost up to 32 games per account at the same time.
- **QR Code & Token Login**: Quick and secure QR code authentication, stored session refresh tokens (no need to re-enter credentials), and Shared Secret (auto-2FA TOTP) support.
- **Custom Game Titles**: Display custom non-Steam game titles or dynamic rotating titles with live variables (`{hours}`, `{time}`, etc.).
- **AFK Auto-Responder**: Automatically replies to incoming Steam friend messages while boosting, with customizable cooldowns per contact to prevent spam.
- **Trading Card Drop Tracker**: Monitors Steam trading card drops in real time and reports drop events along with estimated Steam Community Market prices directly to Telegram.
- **Free Game Hunter (License Claimer)**: Detects and automatically claims free-to-keep promotional packages and weekend giveaways directly to your Steam accounts.
- **Per-Account SOCKS5 / HTTP Proxy**: Route individual Steam accounts through dedicated proxies to isolate IP addresses.
- **Security & DPAPI Encryption**: Passwords and session tokens are encrypted at rest using Windows Data Protection API (DPAPI).
- **Auto-Pause on Real Game**: Automatically suspends boosting when you launch a real game, and resumes when you finish playing.
- **Scheduler & Auto-Start**: Configure scheduled boosting hours and optional Windows startup with system tray minimization.
- **Developer Debug Packet Inspector**: Live interactive console monitoring underlying Steam CM protocol packets and session events.

---

## 🚀 Getting Started

### Download Pre-built Release
Check the [Releases](https://github.com/supreaxd/SteamHourBooster/releases) section for the latest `SupBoost.exe`.

### Building from Source

#### Prerequisites
- [Rust](https://rustup.rs/) (stable channel, 2021 edition)
- Windows 10 / 11

#### Build Command
```powershell
# Clone the repository
git clone https://github.com/supreaxd/SteamHourBooster.git
cd SteamHourBooster

# Build in release mode
cargo build --release
```

The optimized executable will be located at:
`target/release/SupBoost.exe`

---

## 🔒 Security & Privacy

All credentials, passwords, and tokens are stored locally on your machine and encrypted using Windows DPAPI. Your credentials are never uploaded to any third-party servers; all Steam network traffic communicates directly with official Valve Steam CM servers (or via your configured proxies).

---

## ⚠️ Disclaimer

This tool is designed for educational and personal automation purposes. Use at your own risk. The developers are not affiliated with Valve Corporation or Steam.

---

## 📄 License

MIT License. See [LICENSE](LICENSE) for details.
