# 🚀 SupBooster v1.4.0 (Live Behind-Window Liquid Glass, Steam In-Game & AFK Fix, Trade Reliability)

> **Languages / Diller / Языки:** **English (Main)** · [🇹🇷 Türkçe](#-türkçe-sürüm-notları-v140) · [🇷🇺 Русский](#-примечания-к-выпуску-v140)

Welcome to **v1.4.0** of **SupBooster** (`SupBoost.exe`)! This release introduces a live behind-window **Liquid Glass & Blur** compositor with symmetric 4-edge optical refraction, native borderless window resizing, immediate Steam **In-Game (orange)** & **Waiting** state handling, an **AFK Auto-Responder** duplicate-message fix, and deeper **Steam Trade Consolidate** reliability fixes.

---

### ✨ 1. Live Behind-Window Liquid Glass, Blur Slider & Borderless UX
- **Live Behind-Window Blur Compositor (`backdrop.rs`):** Replaced static wallpaper-only blur with a live Windows `PrintWindow` + wallpaper compositor (with safe single-monitor Magnification fallback) so Liquid Glass and Blur modes smoothly frost actual windows behind SupBooster without multi-monitor crashes or scroll flicker.
- **Refined `Blur` Mode & 0.0–10.0 Slider:** Renamed `Transparent` to `Blur` (preserving backward config compatibility) and added a fine-grained **Blur Intensity** slider (`0.0 = Clear`, `10.0 = Heavy`, default `6.0`) with a gentle progressive curve.
- **Symmetric 4-Side Optical Glass Bending:** Added a 26px curved lens UV refraction mesh across top, bottom, left, and right edges paired with a 5-ring translucent perimeter meniscus shimmer, themed TopBar/StatusBar chrome, and +5px frame padding so borders never crowd text or icons.
- **Native 8-Way Borderless Resizing:** Added Windows `WM_NCHITTEST` subclassing so all 4 edges and corners resize smoothly and support Aero Snap while leaving TopBar buttons untouched.

### 🎮 2. Steam Session, In-Game Status & AFK Auto-Responder Fix
- **Immediate In-Game Detection (Orange Status):** Subscribes to `CMsgClientPlayingSessionState` before sending `GamesPlayed`; if you are already playing a game on Steam, boosting pauses automatically with an orange In-Game indicator and resumes cleanly when you exit.
- **Resilient Waiting / Reconnecting Fallback:** Transient Steam server discovery timeouts or connection drops now enter a `Waiting / Reconnecting` backoff state instead of stalling in an error state.
- **AFK Duplicate Reply Fix:** Fixed the AFK Auto-Responder sending the same auto-reply twice (modern `CFriendMessages.SendMessage` is used first, falling back to legacy `CMsgClientFriendMsg` only if the modern RPC fails).

### ⚖️ 3. Steam Trade Consolidate & Inventory Reliability
- **Exact `(classid, instanceid)` Matching:** Fixed `parse_inventory_json` matching only `classid`, preventing non-tradable item variants from being flagged as tradable and causing `(11)` errors.
- **Expanded Inventory Fetch (`count=500`) & Clean Session Handshake:** Increased inventory fetch size from 75 to 500 items per context, disabled `reqwest` internal `cookie_store` conflicts in favor of clean single-header `Cookie` & `sessionid` negotiation, trimmed `#` fragments from Trade URLs, and removed sent assets from cache and selection automatically.
- **Clear `(15)` / `(11)` Steam Diagnostics:** Surfaces actionable hints when Steam returns `AccessDenied (15)` (friendship/Trade URL token/pending limit) or `InvalidState (11)` (trade-lock/already offered item).

---

# 🇹🇷 Türkçe Sürüm Notları (v1.4.0)

- **Arkadaki Pencereleri Canlı Blurlayan Liquid Glass & Blur Motoru:** Sadece masaüstü duvar kağıdını değil, uygulamanın arkasındaki aktif pencereleri canlı olarak yakalayıp blurlayan yeni kompozitör (`PrintWindow` + güvenli tek monitör desteği) eklendi; çoklu monitör geçiş çökmesi ve kaydırma titremesi giderildi.
- **Yeni `Blur` Modu, Detaylı 0.0–10.0 Slider & 4 Kenarda Eşit Bükülme:** `Transparent` modu `Blur` olarak adlandırıldı ve varsayılan `6.0` seviyesinde yumuşak artışlı `0.0–10.0` bulanıklık kaydırıcısı eklendi. Pencerenin tüm 4 kenarında (üst, alt, sol, sağ) eşit 26px optik cam bükülmesi, temayla uyumlu üst/alt çubuk, genişletilmiş çerçeve boşlukları ve 8 yönlü kenar/köşe boyutlandırma (Aero Snap dahil) eklendi.
- **Oyunda (Turuncu) İkonu, Waiting Modu & AFK Çift Mesaj Düzeltmesi:** Hesapta oyun açıkken kasma otomatik duraklatılarak turuncu ikonla gösterilir; Steam sunucularına bağlanılamadığında hata yerine `Waiting / Reconnecting` bekleme moduna geçilir ve AFK otomatik yanıtlayıcının aynı mesajı 2 kez göndermesi düzeltildi.
- **Steam Takas & Envanter Güvenilirliği:** Envanter ayrıştırmada `(classid, instanceid)` eşleştirmesi düzeltildi (takas kapalı öğelerin yanlışlıkla seçilmesi engellendi), envanter çekme limiti `500`'e çıkarıldı, çerez/`sessionid` çakışması giderildi, gönderilen öğeler önbellekten temizlendi ve `(15)` / `(11)` hataları için açıklayıcı bilgiler eklendi.

---

# 🇷🇺 Примечания к выпуску (v1.4.0)

- **Живое размытие окон за приложением (Liquid Glass & Blur):** Новый композитор (`PrintWindow` с безопасным fallback) размывает реальные окна позади SupBooster вместо одних обоев рабочего стола, устраняя вылеты при переносе между мониторами и мерцание при прокрутке.
- **Режим `Blur`, плавный ползунок 0.0–10.0 и симметричное преломление:** Режим `Transparent` переименован в `Blur` (по умолчанию `6.0`), добавлено симметричное 26px преломление стекла по всем 4 сторонам окна, единый стиль верхней/нижней панели, расширенные отступы рамок и изменение размера окна за все 4 края и угла (`WM_NCHITTEST`).
- **Оранжевый индикатор «В игре», режим Waiting и фикс двойного ответа AFK:** Мгновенная проверка `CMsgClientPlayingSessionState` ставит буст на паузу (оранжевый значок), временные сбои подключения переводятся в ожидание с повтором, а автоответчик AFK больше не дублирует сообщения.
- **Надёжность обменов Steam:** Исправлено сопоставление `(classid, instanceid)` в инвентаре, увеличен лимит загрузки до `500`, устранены конфликты `Cookie`/`sessionid`, очищаются отправленные предметы из кэша и добавлены подсказки для ошибок Steam `(15)` и `(11)`.
