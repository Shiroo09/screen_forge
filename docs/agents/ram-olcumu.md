# RAM ölçümü: C# orijinal vs Rust portu

Ölçüm: 2026-11-05, Windows 11 Pro (26200), aynı makine, aynı koşullar.
Yöntem: her iki uygulama da tepsi modunda başlatıldı, 20 sn oturması beklendi,
ardından 5 sn arayla 4 örnek alındı. `WorkingSet64` PowerShell ile okundu.

## Sonuç

| Uygulama | Ortalama çalışma seti | Örnekler | İş parçacığı |
| --- | --- | --- | --- |
| C# / WPF (`ScreenForge.exe`, net9.0-windows) | **349,8 MB** | 351,9 · 349,7 · 348,9 · 348,9 | 128 |
| Rust (`screenforge-app.exe`, release) | **12,4 MB** | 12,4 · 12,4 · 12,4 · 12,4 | 9 |

**Oran: 28× daha az bellek.** Fark tek ölçümde değil, dört örnekte de sabit.

## Neden bu kadar fark var

- C# tarafı WPF'i ve .NET çalışma zamanını yükler: 128 iş parçacığı, GPU/Composition
  katmanı ve JIT edilmiş derlemeler. Tepside hiçbir şey yapmasa bile bu taban kalır.
- Rust tarafı native: 9 iş parçacığı, çalışma zamanı yok, tepside yalnız kısayol
  mesaj döngüsü ve tepsi simgesi ayakta.

## Bu ölçümün kapsamadığı

- **Yakalama/overlay anı ölçülmedi.** Overlay açıkken tam ekran görüntüsü
  (3383×2160 ≈ 29 MB ham BGRA) bellekte tutulur; yani tepe değer buradan yüksek olur.
  Tepsi ölçümü boştaki tabanı gösterir.
- GIF kaydı ve video kodlama sırasındaki tepe bellek ölçülmedi.
- C# tarafında ilk açılışta JIT nedeniyle sayı bir miktar daha yüksek olabilir;
  20 sn bekleme bunu büyük ölçüde dengeler.

Tekrar üretmek için: `docs/agents/ram-olcumu.md` içindeki PowerShell bloğu
(iki uygulamayı sırayla başlatıp 4 örnek alır).
