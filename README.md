# ⚡ SupBooster v1.4.0 (SteamHourBooster)

> **Languages / Diller / Языки:** **English (Main)** · [🇹🇷 Türkçe](README.tr.md) · [🇷🇺 Русский](README.ru.md)

A lightweight, feature-rich multi-account Steam hour booster, trading card idle manager, inventory seller, and trade consolidator with a modern **Prism UI & Live Liquid Glass** desktop interface built in **100% Rust** (`eframe` / `egui`, `window-vibrancy`, `steam-vent`).

---

## ✨ Key Features (v1.4.0)

- **Live Behind-Window Liquid Glass & Blur Engine**: 5 theme modes (**Dark**, **Light**, **System**, **Blur**, and **Liquid Glass** with live `PrintWindow` behind-window blurring), smooth 0.0–10.0 Blur Intensity slider (default 6.0), symmetric 4-edge optical glass refraction, native 8-way borderless resizing, customizable Glass Opacity, and RGB color pickers.
- **Multi-Account, In-Game Detection & Waiting Fallback**: Expandable sidebar `ACCOUNTS` list, unlimited parallel game boosting, automatic **In-Game (orange)** pause/resume via `CMsgClientPlayingSessionState`, resilient `Waiting / Reconnecting` backoff on server drops, AFK auto-responder (duplicate-free), and live **Steam CM Ping & Connection Quality** monitor.
- **Active Drops & Auto-Switch**: Scans Steam badge data to list games with remaining trading card drops (`remaining_cards > 0`), allows one-click queueing, and automatically switches to the next eligible game when drops hit `0`.
- **Inventory & Bulk Market Seller (4 Pricing Strategies)**: Groups cards & items by game with live Steam Community Market valuations and 15% fee breakdown. Supports **Undercut (-$0.01)**, **Instant Sell**, **Market / Median Price**, and **Custom Offset**, plus mobile confirmation prompts.
- **Trade Consolidate & Account Balance**: Send Steam Trade Offers from selected alt accounts to your main account with item selection checkboxes, Select All / Deselect All, preflight session validation, and Mobile Confirmation notification banner. Track per-account inventory value, cards earned, net/gross revenue, boosted hours, and export directly to `SupBooster_Balance.csv` on Desktop.
- **Telegram Bot Control (`/lang`, `/addaccount`, 15+ Commands)**: Remotely check `/status`, manage boosts, inspect `/drops`, switch bot language (`/lang en|tr|ru`), add accounts (`/addaccount <user> | <pass>`), and receive QR login images and card drop alerts.
- **Dedicated Live Logs & Instant Settings Save**: Top-to-bottom live logs with auto-scroll in their own sidebar tab, and immediate `config.json` persistence on every setting change.
- **Background Security & System Tray**: Credentials and refresh tokens are encrypted locally with Windows DPAPI in the background, paired with responsive System Tray controls (`Open`, `Start All`, `Stop All`, `Exit`).

---

## 🚀 Getting Started

### Download Pre-built Release
Download the latest `SupBoost.exe` from the [Releases](https://github.com/supreaxd/SteamHourBooster/releases) page.

### Building from Source
```powershell
git clone https://github.com/supreaxd/SteamHourBooster.git
cd SteamHourBooster
cargo build --release
```
Executable output: `target/release/SupBoost.exe`

---

## 🔒 Privacy & Disclaimer
All credentials and session tokens are stored locally and encrypted with Windows DPAPI. Network traffic communicates directly with official Steam servers (or your configured per-account SOCKS5/HTTP proxy). Not affiliated with Valve Corporation or Steam.

---

# 🇹🇷 Türkçe Özet

**SupBooster v1.4.0**, **Rust** ile geliştirilmiş modern **Prism UI & Canlı Liquid Glass** arayüzlü çoklu hesap Steam saat kasma, kart düşürme, toplu pazar satış ve takas birleştirme aracıdır.

- **Canlı Liquid Glass & Blur Motoru:** Arkadaki pencereleri canlı blurlayan `Liquid Glass` ve `Blur` modları (`0.0–10.0` arası hassas slider), tüm 4 kenarda eşit optik cam bükülmesi, 8 yönlü kenarsız pencere boyutlandırma ve yüksek kontrastlı Aydınlık/Koyu tema.
- **Oyunda Algılama, Waiting Modu & AFK Düzeltmesi:** Hesapta oyun açıkken turuncu ikonla otomatik duraklatma, bağlantı kesintilerinde `Waiting` bekleme modu ve çiftsiz AFK otomatik yanıt.
- **Takas & Bilanço (`Topla` / `Hesap Bilançosu`):** `(classid, instanceid)` düzeltmeli envanter, tek tek ürün seçip (`Tümünü Seç` destekli) ana hesaba aktarma, telefon mobil onay bilgilendirme kutusu, canlı envanter değeri ve Masaüstüne doğrudan `CSV` dışa aktarma.
- **Pazar Fiyat Stratejileri:** `Undercut (-$0.01)`, `Instant Sell`, `Pazar Fiyatı` ve `Özel Ofset` ile gerçek fiyat hesaplamalı toplu satış.
- **Telegram Botu & Canlı Loglar:** `/lang` ile tek dil seçimi, `/addaccount <kullanıcı> | <şifre>`, inline butonlar ve en alta akan ayrı Loglar sekmesi.

---

# 🇷🇺 Краткое описание (Русский)

**SupBooster v1.4.0** — быстрое десктоп-приложение на **Rust** с интерфейсом **Prism UI & Liquid Glass** для одновременного буста часов Steam, фарма коллекционных карточек, массовой продажи на Торговой площадке и сбора предметов на основной аккаунт.

- **Живой Liquid Glass и Blur:** Размытие реальных окон позади приложения (`Blur` / `Liquid Glass`, ползунок `0.0–10.0`), симметричное 26px преломление на всех 4 сторонах и изменение размера окна за края.
- **Статус «В игре», режим Waiting и AFK:** Оранжевый индикатор при запущенной игре на аккаунте, автоповтор при сбое сети и корректные одиночные ответы AFK.
- **Обмен и Баланс:** Точное сопоставление `(classid, instanceid)`, сбор выбранных предметов с твинков на основу (`Выбрать все`), уведомление о подтверждении в Steam Mobile, оценка инвентаря и экспорт `CSV` на Рабочий стол.
- **4 стратегии цены:** `Undercut (-$0.01)`, `Instant Sell`, `Рыночная цена` и `Свой сдвиг`.
- **Telegram-бот и Логи:** Команды `/lang`, `/addaccount <логин> | <пароль>`, inline-кнопки и отдельная вкладка логов с автопрокруткой вниз.
