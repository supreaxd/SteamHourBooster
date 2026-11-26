# 🚀 SupBooster v1.3.0 (Major Update — Liquid Glass, Prism UI, Trade Consolidate & Pricing Fix)

> **Languages / Diller / Языки:** **English (Main)** · [🇹🇷 Türkçe](#-türkçe-sürüm-notları-v130) · [🇷🇺 Русский](#-примечания-к-выпуску-v130)

Welcome to **v1.3.0 Major Update** of **SupBooster** (`SupBoost.exe`)! This major release transforms the application with the Prism UI dark glass layout, an ultra-transparent **Liquid Glass** engine with specular edge refraction, working **Trade Consolidate** with mobile confirmation alerts, **Account Balance** with Desktop CSV export, fixed **Market Pricing** strategies, and full localization.

---

### ✨ 1. Prism UI & Liquid Glass Theme Engine
- **New `Liquid Glass` Mode (`ThemeMode::LiquidGlass`):** Ultra-transparent frosted background with `window-vibrancy` Acrylic/Blur and crisp specular edge-refraction borders (`14px` rounded glass cards), alongside enhanced `Transparent (Acrylic)`, `Dark`, `Light`, and `System`.
- **Fixed Transparency & Color Pickers:** Fixed the alpha slider snapping back to `255` on Surface Tint; added a dedicated **Glass Opacity** slider (`10..255`) and pure RGB pickers for Text and Accent colors.
- **Light Theme Contrast & Hover Fix:** Forced high-contrast black text across all widgets in Light mode and added clean gray hover fills so hovered sidebar tabs never disappear.
- **Cleaner Sidebar & Wider Window:** Widened default window to `1240x760`, removed redundant `Tools -> Accounts` and `● Boosting` label, added collapsible `▾ ACCOUNTS` with direct click-to-edit on any account (and Dashboard cards), pinned `Changelogs` + `Settings (⚙)` cleanly at the bottom without overlap or clipping, and removed the non-functional top-right search bar.

### ⚖️ 2. Trade Consolidate Fix, Item Selection & Account Balance
- **Steam Trade API Preflight & Session Fix:** Fixed `Invalid JSON response from Steam Trade API` by extracting `sessionid` cookies from `settoken`, running a preflight `GET` on the trade offer page with `X-Requested-With: XMLHttpRequest`, and surfacing exact Steam error messages if a session expired or accounts aren't friends.
- **Per-Item Trade Selection + Mobile Confirmation Box:** Added checkboxes for every tradable item, `Select All` / `Deselect All` controls, clean localized `Consolidate` / `Topla` button naming, and a prominent **Steam Guard Mobile Confirmation** info banner when sending offers.
- **Live Account Balance & Desktop CSV Export:** Displays live cached inventory value (`$X.XX`), card count, sold net/gross earnings, and boosted hours per account with `Refresh All Values` and direct `SupBooster_Balance.csv` export to Desktop.

### 🛒 3. Market Pricing Strategies & Steam CM Ping Monitor
- **4 Real Pricing Strategies:** `Undercut (-$0.01)`, `Instant Sell`, `Market / Median Price`, and `Custom Offset` now compute real prices across item rows, bottom bulk bar, confirmation modal, and Steam `/market/sellitem/` API payloads.
- **Live Steam CM Ping & Quality:** Background TCP latency monitor in the bottom status bar (`Good` / `Fair` / `Poor` / `No Connection`).

### 🛠️ 4. Reliability, Tray, UI Scale, Logs & Telegram
- **System Tray Event Handlers:** Fixed tray right-click menu buttons (`Open SupBooster`, `Start All`, `Stop All`, `Exit`) to respond immediately even when minimized.
- **Smooth UI Scale Slider & Misc Settings:** Fixed UI zoom oscillation while dragging by committing scale on release; renamed `Security` to `Misc` with silent background DPAPI encryption.
- **Dedicated Logs Tab, Instant Auto-Save & Telegram:** Top-to-bottom auto-scrolling Logs tab, immediate `config.json` saving, and Telegram `/lang` + `/addaccount <user> | <pass>`.

---

# 🇹🇷 Türkçe Sürüm Notları (v1.3.0)

- **Prism UI & Yeni `Liquid Glass (Tam Şeffaf)` Teması:** Kenarları ışık kıran buzlu cam çerçeveleri, artırılmış `Acrylic` şeffaflık, serbest bırakılmış şeffaflık (alfa) seçici ve bağımsız **Cam Opaklığı** kaydırıcısı eklendi. Aydınlık temada tüm yazılar her durumda siyah olacak şekilde düzeltildi; sekme üzerine gelindiğinde gri vurgu eklenerek görünmez olma sorunu giderildi.
- **Sol Menü, Dashboard & Ayarlar Düzeni:** Genişletilebilir `ACCOUNTS` listesi, listeden veya Dashboard'dan doğrudan hesap ayarlarına geçiş, alt bölümde kesilmeyen `ACTIVE BOOST / DROPS` kartı ve en altta `Changelogs` + `⚙ Ayarlar` ikonu. `Security` sekmesi `Misc` olarak sadeleştirildi (DPAPI arka planda sessizce aktif).
- **Takas (`Topla`) Düzeltmesi & Ürün Seçimi:** Steam Trade API oturum/preflight doğrulaması düzeltildi ve gerçek Steam hata mesajları gösterildi. Takas yapılacak ürünleri tek tek seçme / `Tümünü Seç`, parantezsiz `Topla` düğmesi, telefonunuza mobil onay bildirimi gittiğini gösteren bilgilendirme kutusu ve Masaüstüne doğrudan `SupBooster_Balance.csv` dışa aktarma eklendi.
- **Pazar Stratejileri, Tray & Ölçek Slider:** `Undercut`, `Instant Sell`, `Pazar Fiyatı` ve `Özel Ofset` gerçek fiyatlarla çalışır hale getirildi; System Tray sağ tık butonları ve arayüz ölçeği kaydırıcısı düzeltildi; Telegram `/lang` ve `/addaccount <kullanıcı> | <şifre>` komutları eklendi.

---

# 🇷🇺 Примечания к выпуску (v1.3.0)

- **Prism UI и новая тема `Liquid Glass`:** Усиленная прозрачность, преломляющие края стекла, исправленный ползунок альфа-канала и ползунок прозрачности стекла, а также чёрный текст и серый hover в Светлой теме.
- **Исправление обменов и Баланс:** Исправлена ошибка `Invalid JSON` при отправке трейдов (preflight-запрос и `sessionid`), добавлен выбор предметов (`Выбрать все`), баннер уведомления о подтверждении в Steam Mobile, живая стоимость инвентаря и экспорт `SupBooster_Balance.csv` на Рабочий стол.
- **Стратегии цен, Трей и Масштаб:** Реальный расчёт для 4 стратегий продажи, исправленные кнопки трея, плавный ползунок масштаба интерфейса, отдельная вкладка логов и команды Telegram `/lang`, `/addaccount`.
