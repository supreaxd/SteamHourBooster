# ⚡ Steam Booster v0.4 (SupBooster Major Update)

Uygulama adı **SupBooster** olarak güncellenmiş ve masaüstü arayüzü modern standartlarda baştan sona yeniden tasarlanmıştır. Bu sürümde performans, görsel şeffaflık, Steam pazarı ve kart kasma otomasyonuna odaklanan köklü yenilikler yer almaktadır.

---

### 🎨 1. Gelişmiş Tema Sistemi & Liquid Glass (Blurlu Şeffaflık)
- **4 Tema Modu:** Koyu (Dark), Aydınlık (Light), Sistem Tercihi (System) ve **Şeffaf Pencere (Liquid Glass / Blur)**.
- **Windows 11 & 10 Acrylic/Mica Desteği:** `window-vibrancy` entegrasyonu ile donanım hızlandırmalı bulanıklaştırılmış cam estetiği. Donanım desteği olmayan ortamlarda otomatik yarı saydam Fallback temaya geçiş.
- **Renk Özelleştirme:** Ayarlar sekmesinden **Pencere Yüzey Rengi (Tint)**, **Yazı Rengi** ve **Vurgu (Accent) Rengi** için interaktif renk paletleri (Color Picker) ve tek tıkla varsayılanlara sıfırlama.
- **Yüksek Kontrast & Okunabilirlik:** Şeffaf modda cam çerçeveler, kartlar ve modallar yarı saydam dolgu (`alpha: 180-220`) ve `1.0px` ince konturla korunarak her masaüstü arka planında keskin okunabilirlik sağlar.

---

### 🎴 2. Active Drops & Otomatik Sıralı Geçiş (Auto-Switch)
- **Steam Rozet Taraması:** Hesaptaki tüm oyunları otomatik tarar ve kalan kart hakkı bulunan oyunları (`remaining_cards > 0`) belirler.
- **Active Drops Sekmesi:** Kalan düşürülebilir kart hakkı bulunan oyunlar `X Kart Kaldı` rozeti ile listelenir, tek tıkla `+ Listeye Ekle` ve `Tüm Aktif Dropları Ekle` butonları ile boosting listesine aktarılabilir.
- **Auto-Switch Mekanizması:** Bir oyunun kalan kart hakkı 0'a indiğinde, bot o oyunu boost listesinden otomatik olarak çıkarıp sıradaki kart hakkı olan oyunu listeye ekleyerek kesintisiz kart kasımı sağlar.

---

### 💰 3. Envanter Yönetimi & Toplu Satış (Bulk Sell)
- **Oyun Bazlı Gruplandırma:** Koleksiyon kartları ve eşyalar oyun adına göre ayrılmış `CollapsingHeader` bloklarında listelenir.
- **Steam CDN İkonları:** Tüm kart ve eşyaların orijinal simgeleri Steam CDN üzerinden dinamik olarak yüklenir ve önbelleğe alınır.
- **Pazar Fiyatı & Komisyon Ayrımı:** Her eşyanın altında **Satılan Fiyat (Pazar Fiyatı)** ve Steam komisyonu (%15) düşüldükten sonra **Kullanıcının Alacağı Net Ücret** şeffaf bir şekilde gösterilir.
- **Hızlı Satış Stratejileri:**
  - `En Düşük Alıştan Hızlı Sat (Quick Sell)`
  - `Mevcut En Düşük Listelemenin 0.01 Altında Listele (Undercut)`
- **Toplu Satış Onay Modalı:** Çoklu dilde `"{count} tane öğe değeri {total} satmak istediğinize emin misiniz?"` onay penceresi, brüt/komisyon/net döküm tablosu ve canlı ilerleme çubuğu.
- **Steam 2FA Mobil Onay:** Hesabınızda `identity_secret` kayıtlıysa toplu satış sonrası oluşan Steam Guard mobil onayları tek tıkla onaylanabilir.

---

### 📱 4. Sadeleştirilmiş Telegram Entegrasyonu
- **Sadece Kart Bildirimleri:** Web arayüzü, sunucu ve gereksiz bildirimler (ücretsiz oyun ekleme, oyun duraklatma vb.) tamamen kaldırıldı. Telegram botu artık **yalnızca Steam Koleksiyon Kartı düşüşlerini** ve pazar değerini bildirir.
- Arayüzden token, chat ID yönetimi, anlık bağlantı testi ve kart drop bildirim anahtarı kontrol edilebilir.

---

### 🚀 5. Arayüz ve Güncelleme Deneyimi
- **Yeni Düzen & Boyut:** Minimum pencere boyutu `960x620` olarak ayarlanmış, 6 sekmeli (~200px) dikey menü ve üst durum çubuğu ile ferah çalışma alanı sağlanmıştır.
- **Sessiz Güncelleme Bildirimi & Changelogs:** Güncelleme olduğunda büyük afişler yerine sol menü altında zarif bir güncelleme rozeti ve tıklanabilir **`Changelogs`** bağlantısı görünür.
- **Veri Kotası Güvenlik Uyarısı:** İndirme başlatılmadan önce mobil veri / internet kotası kullanımı ve olası operatör ücretlendirmesi uyarısı veren onay penceresi eklendi.
- **Paket ve Dosya Adı:** GitHub üzerindeki ana yürütülebilir dosya adı **`SupBoost.exe`** olarak güncellendi.
