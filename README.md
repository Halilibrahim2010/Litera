# Litera 🚀

Linux (Wayland) için tasarlanmış, **GTK4** ve **Rust** ile geliştirilmiş, dosya yöneticisi ile sanal terminali (VTE) çift yönlü ve gerçek zamanlı senkronize eden modern, ultra hafif hibrit masaüstü uygulaması.

---

## ✨ Özellikler

### 1. Çift Yönlü ve Gerçek Zamanlı Senkronizasyon (Bi-Directional Sync)
* **Terminal ➔ Dosya Yöneticisi:**
  * Terminalde `cd` çalıştırıldığında veya aktif çalışma dizini (CWD) değiştiğinde, **OSC 7 escape sequence** (`PROMPT_COMMAND`) ve `/proc/<pid>/cwd` mekanizması üzerinden sinyal yakalanır ve dosya yöneticisi paneli anında o dizine geçer.
* **Dosya Yöneticisi ➔ Terminal:**
  * Görsel panelden bir klasöre girildiğinde veya üst dizine çıkıldığında, terminal otomatik olarak o dizine `cd <escaped_path>` komutunu besler.
* **Canlı Dosya Sistemi İzleyici (Inotify):**
  * Terminalden veya harici programlardan yapılan `mkdir`, `touch`, `rm`, `mv` gibi dosya işlemlerinde `notify` (Linux inotify) arka planda tetiklenir ve dosya listesi gecikmesiz yenilenir.

### 2. Dinamik Görünüm Modları
* **◫ Hibrit Mod (F1):** Sol tarafta dosya yöneticisi, sağ tarafta terminal (ayarlanabilir bölünmüş ekran ayırıcısı).
* **⬛ Saf Terminal Modu (F2):** Dosya yöneticisi tamamen gizlenir, tam alan terminale tahsis edilir.
* **📁 Saf Dosya Yöneticisi Modu (F3):** Terminal gizlenir, görsel dosya gezgini tam ekran olur.

### 3. Etkileşim ve Verimlilik Detayları
* **Sürükle & Bırak (Drag & Drop):** Dosya yöneticisindeki herhangi bir dosya terminale sürüklendiğinde, dosyanın kabuk kaçış karakterli (shell-escaped) tam yolu terminal imlecine yapıştırılır.
* **Sağ Tık Menüsü (Context Menu):**
  * *Klasöre Gir / Dosyayı Aç (Varsayılan Sistem Uygulaması)*
  * *Terminalde Bu Konuma Geç (cd)*
  * *Terminalde Bilgilerini Görüntüle (`ls -la` / `file`)*
* **Hızlı Kısayollar:**
  * `F1`: Hibrit Mod
  * `F2`: Saf Terminal Modu
  * `F3`: Saf Dosya Yöneticisi Modu
  * `F5`: Listeyi Yenile
  * Üst Bar Butonları: Gizli Dosyaları Göster/Gizle, Üst Dizine Çık, Görünüm Değiştiriciler.

---

## 🎨 Tasarım ve Tema Kuralları
Litera, özel tasarlanmış **Glassmorphism** koyu mor/obsidyen paletiyle gelir:

| Değişken | Değer | Açıklama |
|---|---|---|
| `--bg-primary` | `#120f1d` | Arka plan ana tonu |
| `--bg-secondary` | `#181525` | İkincil kartlar ve menüler |
| `--bg-tertiary` | `#211d33` | Panel ve buton arka planı |
| `--accent` | `#7a66ff` | Canlı vurgu & odak rengi |
| `--accent-hover` | `#8f7dff` | Buton hover rengi |
| `--accent-secondary` | `#2c2840` | Kenarlık (Border) çizgileri |
| `--text-primary` | `#f0eef7` | Metin ve terminal ön planı |
| `--card-glass` | `rgba(122, 102, 255, 0.18)` | Cam efektli pencere |
| `--radius` | `1rem` (16px) | Köşe yuvarlama |

---

## 🛠️ Derleme ve Çalıştırma

### Bağımlılıklar (Ubuntu / Pop!_OS / Debian)
- GTK4 geliştirme kütüphaneleri (`libgtk-4-dev`)
- VTE GTK4 kütüphanesi (`libvte-2.91-gtk4-dev`)
- Rust toolchain (`cargo`, `rustc`)

### Geliştirme Modunda Çalıştırma:
```bash
cargo run
```

### Optimize Edilmiş Release İkili Dosyası:
```bash
cargo run --release
```
Derlenmiş ikili dosya: `target/release/litera` (yalnızca ~1.5 MB boyuta sahiptir).
