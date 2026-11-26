# SteamBooster — Proje Bağlamı

## Hedef
Lenovo Legion 5 için system tray uygulaması.
İki mod: MAKSIMUM PERFORMANS ve MİNİMUM PERFORMANS.
Her modda ağ kartları (Ethernet + Wi-Fi) daima açık kalır.

---

## Donanım
| Bileşen | Detay |
|---|---|
| Model | Lenovo Legion 5 15IRX10 (83LY) |
| CPU | Intel i7-13650HX — 14 çekirdek / 20 thread |
| GPU | NVIDIA GeForce RTX 5060 Laptop GPU |
| iGPU | Intel UHD Graphics |
| RAM | 32 GB |
| OS | Windows 11 |
| Yazılım | Lenovo Legion Space 2.1.11.31 kurulu |

---

## Güç Planı GUIDleri
| Plan | GUID |
|---|---|
| Nihai Performans | `4cb7f453-bb24-4e9b-8a13-cd44df77eca1` |
| Yüksek Performans | `605b5320-7995-4598-a649-6a62703775b6` |
| Dengeli | `381b4222-f694-41f0-9685-ff5bb260df2e` |
| Gece Sessiz Mod | `2f0fb5d6-79c1-440e-8cdc-8f5565a2ae6f` |

---

## Lenovo WMI API
```
Namespace : root\wmi
Class     : LENOVO_GAMEZONE_DATA
```

### Fan Modu — SetSmartFanMode(value)
| Değer | Mod |
|---|---|
| 1 | Sessiz (Quiet) — fan neredeyse duruyor |
| 2 | Dengeli (Balance) |
| 3 | Performans (Performance) — fan turbo |

### Fan Turbo Boost — SetFanCooling(value)
| Değer | Durum |
|---|---|
| 0 | Kapalı |
| 1 | Açık (ek soğutma) |

### Diğer Çağrılar
```powershell
$gz = Get-WmiObject -Namespace root\wmi -Class LENOVO_GAMEZONE_DATA
$gz.GetSmartFanMode()   # mevcut fan modunu okur
$gz.GetThermalMode()    # thermal modu okur (1=Quiet şu an)
```

---

## Powercfg Subgroup GUIDleri

### Kablosuz Bağdaştırıcı (Wi-Fi daima açık)
```
Subgroup : 19cbb8fa-5279-450e-9fac-8a3d5fedd0c1
Setting  : 12bbebe6-58d6-4636-95bb-3217ef867c1a
Value    : 0  →  Maksimum Performans (uyumaz)
```

### PCI Express Link State (ağ kartı kapanmasın)
```
Subgroup : 501a4d13-42af-4429-9fd1-a8218c268e20
Setting  : ee12f906-d277-404b-b6da-e5fa1a576df5
Value    : 0  →  Kapalı
```

### CPU (SUB_PROCESSOR)
```
PROCTHROTTLEMIN  : 893dee8e-2bef-41e0-89c6-b55d0929964c
PROCTHROTTLEMAX  : bc5038f7-23e0-4960-96da-33abaf5935ec
PERFBOOSTMODE    : be337238-0d82-4146-a960-4f3749d470c7  (0=Kapalı, 2=Agresif)
CPMINCORES       : 0cc5b647-c1df-4637-891a-dec35c318583
```

---

## Registry Ayarları

### NVIDIA PowerMizer
```
HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e96c-e325-11ce-bfc1-08002be10318}\000X
  PerfLevelSrc      = 0x2222
  PowerMizerEnable  = 1
  PowerMizerLevel   = 1  (Max Perf) | 3  (Enerji Tasarrufu)
  PowerMizerLevelAC = 1  (Max Perf) | 3  (Enerji Tasarrufu)
```

### Power Throttling (CPU kısıtlama)
```
HKLM:\SYSTEM\CurrentControlSet\Control\Power\PowerThrottling
  PowerThrottlingOff = 1  (Kapalı / Max Perf)
                     = 0  (Açık / Sessiz Mod)
```

### MMCSS Oyun Önceliği (Max Perf için)
```
HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile
  SystemResponsiveness   = 0       (Max) | 20 (Normal)
  NetworkThrottlingIndex = 0xFFFF  (Max) | 10 (Normal)

HKLM:\...\Tasks\Games
  GPU Priority        = 8
  Priority            = 6
  Scheduling Category = "High"
  SFIO Priority       = "High"
```

### Hardware-Accelerated GPU Scheduling
```
HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers
  HwSchMode = 2  (Zaten açık, dokunma)
```

### Xbox Game DVR (Max Perf'de kapat)
```
HKCU:\System\GameConfigStore
  GameDVR_Enabled       = 0
  GameDVR_FSEBehaviorMode = 2
```

### Ağ Kartı Güç Yönetimi (Registry)
```
HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}\000X
  PnPCapabilities = 24  (Windows aygıtı kapatamaz)
Etkili kart: Realtek PCIe GbE (Ethernet), Intel Wi-Fi 6 AX203
```

---

## MAKSIMUM PERFORMANS Ayarları
1. Güç planı: Nihai Performans (`4cb7f453`)
2. CPU: Min=%100, Max=%100, Boost=Agresif(2)
3. Çekirdek park: %100 aktif
4. Fan: SetSmartFanMode(3) + SetFanCooling(1)
5. NVIDIA: PowerMizerLevel=1 (Maks)
6. PowerThrottling: Kapalı
7. MMCSS: SystemResponsiveness=0, GPU Priority=8
8. GameDVR: Kapalı
9. Ağ: Her zaman açık

## MİNİMUM PERFORMANS Ayarları
1. Güç planı: Gece Sessiz Mod (`2f0fb5d6`)
2. CPU: Min=%5, Max=%60, Boost=Kapalı(0)
3. Fan: SetSmartFanMode(1) + SetFanCooling(0)
4. NVIDIA: PowerMizerLevel=3 (Tasarruf)
5. PowerThrottling: Açık
6. MMCSS: SystemResponsiveness=20
7. Ağ: Her zaman açık
