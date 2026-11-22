# ⚡ SteamBooster

A lightweight, feature-rich multi-account Steam hour booster with a modern desktop GUI built in **Rust** using **egui** and **steam-vent**.

---

## ✨ Features

- **Built with 100% Rust**: Engineered in pure Rust for blazing-fast performance, memory safety, and ultra-low CPU/RAM footprint (~50 MB RAM).
- **Multi-Account Boosting**: Boost hours simultaneously across multiple Steam accounts.
- **Multi-Game Boosting**: Select and boost up to 32 games per account at the same time.
- **QR Code & Token Login**: Quick and secure QR code authentication, stored session refresh tokens (no need to re-enter credentials), and Shared Secret (auto-2FA TOTP) support.
- **Custom Game Titles**: Display custom non-Steam game titles or dynamic rotating titles with live variables (`{hours}`, `{time}`, etc.).
- **AFK Auto-Responder**: Automatically replies to incoming Steam friend messages while boosting, with customizable cooldowns per contact to prevent spam.
- **Trading Card Drop Tracker**: Monitors Steam trading card drops in real time and reports drop events along with estimated Steam Community Market prices.
- **Free Game Hunter (License Claimer)**: Detects and automatically claims free-to-keep promotional packages and weekend giveaways directly to your Steam accounts.
- **Inventory Tracker**: Tracks item counts and changes across your boosted accounts.
- **Telegram Bot Integration**: Full remote control of boosting, status checks, and instant notifications via Telegram.
- **Per-Account SOCKS5 / HTTP Proxy**: Route individual Steam accounts through dedicated proxies to isolate IP addresses.
- **Security & DPAPI Encryption**: Passwords and session tokens are encrypted at rest using Windows Data Protection API (DPAPI).
- **Auto-Pause on Real Game**: Automatically suspends boosting when you launch a real game, and resumes when you finish playing.
- **Scheduler & Auto-Start**: Configure scheduled boosting hours and optional Windows startup with system tray minimization.
- **Developer Debug Packet Inspector**: Live interactive console monitoring underlying Steam CM protocol packets and session events.

---

## 🚀 Getting Started

### Download Pre-built Release
Check the [Releases](https://github.com) section for the latest `SteamBooster.exe`.

### Building from Source

#### Prerequisites
- [Rust](https://rustup.rs/) (stable channel, 2021 edition)
- Windows 10 / 11

#### Build Command
```powershell
# Clone the repository
git clone https://github.com/<your-username>/SteamBooster.git
cd SteamBooster

# Build in release mode
cargo build --release
```

The optimized executable will be located at:
`target/release/SteamBooster.exe`

---

## 🔒 Security & Privacy

All credentials, passwords, and tokens are stored locally on your machine and encrypted using Windows DPAPI. Your credentials are never uploaded to any third-party servers; all Steam network traffic communicates directly with official Valve Steam CM servers (or via your configured proxies).

---

## ⚠️ Disclaimer

This tool is designed for educational and personal automation purposes. Use at your own risk. The developers are not affiliated with Valve Corporation or Steam.

---

## 📄 License

MIT License. See [LICENSE](LICENSE) for details.
