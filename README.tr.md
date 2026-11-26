# ⚡ SupBooster v1.3.0 (Türkçe)

> **Diller:** [🇬🇧 English (Main)](README.md) · **🇹🇷 Türkçe** · [🇷🇺 Русский](README.ru.md)

**SupBooster**, **Rust** (`eframe` / `egui`, `window-vibrancy`, `steam-vent`) ile geliştirilmiş, modern **Prism UI & Liquid Glass** arayüzüne sahip hafif ve gelişmiş bir Steam çoklu hesap saat kasma, kart düşürme, envanter satış ve takas birleştirme uygulamasıdır.

## ✨ Özellikler (v1.3.0 Major Update)
- **Liquid Glass & Şeffaf Tema Motoru:** Koyu, Aydınlık, Sistem, Şeffaf (Acrylic) ve ışık kıran cam kenarlıklı **Liquid Glass (Tam Şeffaf)** temaları; serbest alfa şeffaflık seçici, cam opaklık kaydırıcısı ve yüksek kontrastlı Aydınlık tema.
- **Sol Kenar Çubuğu & Sistem Tepsisi:** Genişletilebilir `HESAPLAR` listesi, doğrudan hesap ayarlarına geçiş, alt kısma hizalı `Changelogs` ve çalışan Sistem Tepsisi (`Open`, `Start All`, `Stop All`, `Exit`).
- **Active Drops & Sınırsız Paralel Kasma:** Kalan kart hakkı `> 0` olan oyunları tarar, paralel olarak kasar ve kart hakkı `0` olunca sıradaki oyuna otomatik geçer. Alt durum çubuğunda anlık **Steam CM Ping & Bağlantı Kalitesi** gösterilir.
- **Envanter & 4 Fiyat Stratejisi:** `Undercut (-$0.01)`, `En Düşük Alış Siparişi`, `Pazar / Medyan Fiyat` ve `Özel Ofset` stratejileriyle gerçek fiyat hesaplamalı toplu satış ve onay penceresi.
- **Takas (Topla) & Hesap Bilançosu:** Alt hesaplardaki kartları/öğeleri seçerek ana hesaba tek tıkla takas teklifi gönderme, Steam Mobil Onay bilgilendirme kutusu, canlı hesap değerleri ve Masaüstüne `SupBooster_Balance.csv` dışa aktarma.
- **Telegram Bot & Canlı Loglar:** `/lang`, `/addaccount <kullanıcı adı> | <şifre>`, QR görsel gönderimi, kart drop bildirimleri ve ayrı Canlı Loglar sekmesi.
- **Arka Plan Güvenliği:** Tüm şifreler ve oturum tokenları Windows DPAPI ile yerel olarak şifrelenir.
