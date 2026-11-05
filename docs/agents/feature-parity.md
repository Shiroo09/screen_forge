# C#/WPF → Rust özellik paritesi denetimi

Salt okunur denetim. Kaynak: eski C#/WPF uygulaması (`ScreenForge/`, `ScreenForge/obj/**` hariç) referans alındı; Rust portu `rust/crates/` (12 crate).

**Yöntem notu.** Denetim sırasında çalışma ağacı değişti (`rust/crates/sf-capture/src/editor_window.rs`, `overlay_chrome.rs`, `sf-draw/src/model.rs` commit edilmemiş durumda). Bu belgedeki Rust satır numaraları **denetim sonundaki güncel ağaca** göredir ve commit `2841e59`'dan farklı olabilir. Özellikle `draw_option_bar` / `draw_action_bar` artık `editor_window.rs`'ten çağrılıyor; `draw_size_badge` / `draw_toast` hâlâ yalnız test tarafından çağrılıyor.

Sınıflandırma:
- **PORTED** — Rust'ta uygulamadan erişilebilen çalışır karşılık var.
- **PARTIAL** — alt küme, VEYA var ama uygulamadan hiç çağrılmıyor, VEYA çizilen UI'nin işleyicisi yok, VEYA stub. Hangisi olduğu "eksik" kolonunda yazılı.
- **MISSING** — Rust karşılığı yok.
- **belirsiz** — doğrulanamadı.

---

## 1. Özellik tablosu

### 1.1 Yakalama ve overlay (C# `CaptureOverlayWindow`, `ScreenCapture`)

| Özellik | C# kaynak | Rust durum | Rust kaynak | Eksik olan |
|---|---|---|---|---|
| Bölge seçimi (sürükle) | `Windows/CaptureOverlayWindow.xaml.cs:657,811` | PORTED | `sf-capture/src/editor_window.rs:247,266`, `sf-capture/src/selection.rs:43,62,83` | — |
| Tam ekran / monitör yakalama | `Capture/ScreenCapture.cs:73` | PORTED | `sf-capture/src/screen.rs:75`, `sf-app/src/panels/capture.rs:202` | Yalnız GDI; C#'taki DXGI hızlı yolu (`ScreenCapture.cs:60,89`) yok. İşlevsel fark yok |
| Pencere algılama + hover vurgusu (pencereye tıkla-seç) | `Windows/CaptureOverlayWindow.xaml.cs:396` (`BuildWindowList`), `:454` (`UpdateWindowHover`) | MISSING | — | `WindowFromPoint`/`EnumWindows` Rust'ta hiç yok |
| Serbest mod (serbest tuval) | `CaptureOverlayWindow.xaml.cs:26,574` (`CaptureMode.Free`) | PORTED | `sf-app/src/panels/collage.rs:30,80`, `app.rs:60,64` | Kolaj tuvali olarak var; ekran üstü serbest form seçim olarak yok |
| Çok monitör: tüm sanal masaüstü overlay | `Capture/ScreenCapture.cs:45`, `Windows/ChromeScale.cs:35` | PARTIAL | `sf-capture/src/screen.rs:63`, `editor_window.rs:328` (`cursor_monitor`) | Rust overlay yalnız imlecin bulunduğu monitörü kaplar; C# tüm sanal masaüstünü kaplar |
| Krom için DPI ölçekleme | `ChromeScale.cs:35`, `CaptureOverlayWindow.xaml.cs:3448` | PARTIAL | `editor_window.rs:163-171` | Rust ölçeği yalnız pencere DC DPI'ından türetir; monitör başına etkin DPI oranı yok |
| Beyaz çarpı imleç | `Windows/WhiteCrossCursor.cs:11` | PARTIAL | `editor_window.rs:103` | Stok `IDC_CROSS`; çizilen beyaz çarpı yok |
| Anlık W×H boyut rozeti | `CaptureOverlayWindow.xaml.cs:1187` | PARTIAL | `overlay_chrome.rs:887` (`draw_size_badge`) | **Hiç çağrılmıyor** — tek referans `sf-capture/tests/chrome_render.rs:166`. Overlay boyut göstermiyor |
| Esc ile iptal | `CaptureOverlayWindow.xaml.cs:328` | PORTED | `editor_window.rs:305`, `overlay_window.rs:197` | — |
| Sağ tık ile bitir/iptal | `CaptureOverlayWindow.xaml.cs` (sağ tuş işleyicisi yok — belirsiz) | PORTED | `editor_window.rs:297-301` | — |
| Enter ile onayla | `CaptureOverlayWindow.xaml.cs` (satır belirsiz) | PORTED | `editor_window.rs:314` | — |
| Seçimi yeniden boyutlandırma (tutamaçlar) | `CaptureOverlayWindow.xaml.cs:944` (`ResizeSelection`) | MISSING | — | Kenar isabet testi / tutamaç sürükleme yok |
| Yeniden seçim | `CaptureOverlayWindow.xaml.cs:~1005` | MISSING | — | Kapatmadan yeniden seçilemez |
| Seçimi sürükleyerek taşıma | `CaptureOverlayWindow.xaml.cs:963,1010` | MISSING | — | — |
| Yakala → düzenle | `CaptureOverlayWindow.xaml.cs:1228` (`EnterEditPhase`) | PORTED | `editor_window.rs:95` | — |
| Yakala → panoya kopyala | `CaptureOverlayWindow.xaml.cs:4330` (`DoCopy`) | PORTED | `sf-capture/src/clipboard.rs:8`, `sf-app/src/tray.rs:138` | — |
| Yakala → dosyaya kaydet | `CaptureOverlayWindow.xaml.cs:4343` (`DoSave`) | PORTED | `sf-app/src/tray.rs:148`, `panels/capture.rs:231` | — |
| Yakala → buluta yükle | `CaptureOverlayWindow.xaml.cs:4356` (`DoUpload`), `App.xaml.cs:367` | PARTIAL | `sf-app/src/panels/upload.rs:41` | Yalnız panel butonu; tam ekran+yükle hotkey'i kayıtlı değil, tepside öğe yok |
| Yakala → çevir (Lens) | `CaptureOverlayWindow.xaml.cs:2261` (`DoTranslateAsync`) | PARTIAL | `sf-app/src/panels/translate.rs:28` | Rust çeviri salt metin paneli; yakalanan görüntüyü/bölgeyi hiç almıyor, OCR/Lens yok |
| Yakala → bölgeyi GIF/video kaydet | `CaptureOverlayWindow.xaml.cs:2567` (`StartRecording`) | PARTIAL | `sf-app/src/panels/record.rs:29` | Kayıt tüm ekranı alır; overlay bölgesinden başlatılmaz |
| Overlay'e dosya bırak → yükle | `Windows/FileDropUploadWindow.cs:17,146,183` | MISSING | — | `rust/crates` içinde `WM_DROPFILES`/`AllowDrop` yok |
| Kayıt biçimi seçici popup (GIF/video) | `Windows/RecorderPickerPopup.cs:12,249,263` | MISSING | — | Popup yok; panel yalnız biçim/süre/FPS sunuyor |
| Ters arama seçici popup | `Windows/ReverseSearchPickerPopup.cs:10,62` | MISSING | — | Özellik uygulamada tamamen yok |
| Görsel çeviri seçici popup | `Windows/VisualTranslatePickerPopup.cs:11,107` | MISSING | — | — |
| Kayıt geri sayımı (3-2-1 + ses metreleri) | `Windows/RecordingCountdown.cs:16` | MISSING | — | `show_countdown` ayarı (`sf-settings/src/lib.rs:291`) hiç okunmuyor |
| Mod çubuğu (Bölgesel / Tam ekran / Serbest) | `CaptureOverlayWindow.xaml.cs:569,577` | PARTIAL | `overlay_chrome.rs:202`, çağrı `editor_window.rs:577` | Yalnız 2 çip; "Serbest" yok; "Bölgesel" çipi no-op (`editor_window.rs:373`) |
| Eylem çubuğu (Kaydet/Kopyala/Yükle/Çevir/İptal) | `CaptureOverlayWindow.xaml.cs:1812` | PARTIAL | çizim `overlay_chrome.rs:745`, isabet `editor_window.rs:380` | Çipler overlay'i kapatıyor ama seçilen eylem **tüketici tarafından okunmuyor**: `EditResult.action` alanını kimse okumuyor (grep: `\.action` için sf-app/sf-cli'de çağrı yok); "Çevir" çipi çeviri yapmıyor |
| Araç şeridi (10 araç) | `CaptureOverlayWindow.xaml.cs:1506` (`BuildToolbar`) | PORTED | `overlay_chrome.rs:348`, `editor_window.rs:593` | Sürüklenebilir grip / sticky-tool göstergesi / tooltip yok |
| Seçenek çubuğu (kalınlık/dolgu çipleri) | `CaptureOverlayWindow.xaml.cs:1572` (`BuildOptionBar`) | PARTIAL | `overlay_chrome.rs:635`, `editor_window.rs:608` | Dolgu çipi no-op (`editor_window.rs:379`) |
| Toast / meşgul banner'ı | `CaptureOverlayWindow.xaml.cs:3943` (`ShowBusyToast`) | PARTIAL | `overlay_chrome.rs:935` (`draw_toast`) | **Hiç çağrılmıyor** — tek referans `tests/chrome_render.rs:179` |
| İpucu (hint) kutusu | `CaptureOverlayWindow.xaml.cs:3329` | PORTED | `overlay_chrome.rs:309`, `editor_window.rs:578` | — |
| Seçim dışı karartma | `CaptureOverlayWindow.xaml:43-46` | PORTED | `editor_window.rs:464` (`dim_outside`) | — |
| Klavye/tooltip ipuçları | `CaptureOverlayWindow.xaml.cs` araç çubuğu | MISSING | — | — |

### 1.2 Global kısayollar

| Özellik | C# kaynak | Rust durum | Rust kaynak | Eksik olan |
|---|---|---|---|---|
| Bölge yakalama kısayolu | `App.xaml.cs:164` | PORTED | `sf-app/src/tray.rs:24-29`, `sf-capture/src/hotkey.rs:52` | — |
| Tam ekran kısayolu | `App.xaml.cs:165` | MISSING | alan: `sf-settings/src/lib.rs:631` | Kaydedilmiyor; ayar panelinde "aktif" gibi gösteriliyor |
| Tam ekran yükleme kısayolu | `App.xaml.cs:166` | MISSING | alan: `sf-settings/src/lib.rs:632` | Kaydedilmiyor |
| Kolaj kısayolu | `App.xaml.cs:167` | MISSING | alan: `sf-settings/src/lib.rs:633` | Kaydedilmiyor |
| Hızlı çeviri kısayolu | `App.xaml.cs:168` | MISSING | alan: `sf-settings/src/lib.rs:634` | Kaydedilmiyor |
| Sunum kısayolları (kalem/lazer/dikdörtgen/elips/ok/çizgi/spot/renk) | `App.xaml.cs:198-225` | MISSING | alanlar: `sf-settings/src/lib.rs:427-443` | Hiç kaydedilmiyor; sunum penceresi sadece 1/2/3 tuşlarını dinliyor |
| Ayrılmış kısayol filtresi/uyarısı | `Hotkeys/WindowsReservedHotkeys.cs:58`, `SettingsWindow.xaml.cs:838` | PARTIAL | `sf-capture/src/reserved_hotkeys.rs:8` | Var ama **uygulamadan hiç çağrılmıyor** (yalnız test) |
| Düşük seviyeli klavye hook'u (Fn / PrtSc özel) | `Hotkeys/HotkeyService.cs:204,230` | MISSING | — | Rust yalnız `RegisterHotKey` kullanıyor; `WH_KEYBOARD_LL` yok |
| Kısayol düzenleme (tıkla-kaydet) | `SettingsWindow.xaml.cs:70-150` | MISSING | `sf-app/src/panels/settings.rs:114` | Salt okunur ("Kısayollar şimdilik salt okunur.") |

### 1.3 Açıklama / düzenleme (statik ekran görüntüsü)

| Özellik | C# kaynak | Rust durum | Rust kaynak | Eksik olan |
|---|---|---|---|---|
| Dikdörtgen aracı | `Editor/InteractiveCanvas.cs:1057`, `ShapeItems.cs:10` | PARTIAL | `sf-draw/src/shapes.rs:64`, `model.rs` | Köşe yarıçapı (`RectItem.CornerRadius`) ve dolgu yok |
| Elips aracı | `InteractiveCanvas.cs:1060`, `ShapeItems.cs:49` | PARTIAL | `sf-draw/src/shapes.rs:100` | Dolgu yok |
| Ok aracı | `InteractiveCanvas.cs:1066`, `ShapeItems.cs:142` | PARTIAL | `sf-draw/src/shapes.rs:134` | Eğme (bend/bezier) yok (`ArrowItem.BendPoint`, `ShapeItems.cs:147-227`); ok uç ölçeği (`HeadScale`) yok |
| Çizgi aracı | `InteractiveCanvas.cs:1063`, `ShapeItems.cs:79` | PORTED | `sf-draw/src/shapes.rs:35` | — |
| Serbest kalem | `InteractiveCanvas.cs:1069`, `ShapeItems.cs:272` | PARTIAL | `sf-draw/src/shapes.rs:53` | Chaikin yumuşatma yok; `sf-app` panelinde kalem yalnız 2 nokta kaydeder (`annotate.rs:429`) |
| Fosforlu kalem | `InteractiveCanvas.cs:1073`, `ShapeItems.cs:415` | PARTIAL | `sf-draw/src/highlight.rs:11`, `overlay` | Sabit genişlik 14 / alfa 110; C# genişlik×4 / alfa 120 |
| Blur / pikselleştirme | `InteractiveCanvas.cs:1078`, `ShapeItems.cs:104,208` | PARTIAL | `sf-draw/src/blur.rs:22` | Pikselleştirme UI'a bağlı değil (`MaskKind::Pixelate` tanımlı ama iki çağrı yeri `MaskKind::Blur` sabitliyor); overlay gücü sabit 8.0; canlı cam önizleme yok |
| Adım işareti | `InteractiveCanvas.cs:1176`, `StepBlurImageItems.cs:7,43` | PARTIAL | `sf-draw/src/shapes.rs:163` | Yalnız daire; C#'ta Circle/Square/Bubble; şekil seçimi UI'ı ve sayaç sıfırlama (`Scene.ResetStepCounter`) yok |
| Metin aracı | `InteractiveCanvas.cs:1087,1151`, `TextItem.cs:11` | PARTIAL | `sf-draw/src/text.rs:76` | Overlay'de metin girişi yok — sabit `"TEXT"` yazıyor (`editor_window.rs:428`); punto 2.0 sabit |
| Yerleştirilmiş metni düzenleme | `InteractiveCanvas.cs:615`, `CaptureOverlayWindow.xaml.cs:4030` (`BeginTextEdit`) | MISSING | — | Çift tıkla düzenleme yok |
| Interaktif kırpma (crop) aracı | `EditorTool.cs:16`, `InteractiveCanvas.cs:1488,1552`, `Scene.cs:245` | MISSING | yardımcı: `sf-draw/src/crop.rs:16` | Tutamaçlı interaktif kırpma yok; `crop_bgra` yalnız test çağrısı; `Tool` enum'unda `Crop` yok |
| Resim öğesini kırpma | `InteractiveCanvas.cs:1608,1618`, `ShapeItems.cs:280` | MISSING | — | — |
| Şekil dolgu rengi | `CaptureOverlayWindow.xaml.cs:3080` (`AddFillSwatch`) | PARTIAL | `sf-draw/src/shapes.rs:28` (`with_fill`) | Dolgu hiçbir render yolundan geçmiyor; `Hit::Fill(_)` no-op |
| Renk paleti | `CaptureOverlayWindow.xaml.cs:2593,2619` | PORTED | `sf-app/src/panels/annotate.rs:29`, `overlay_chrome.rs:66` | Palet içerikleri C#'tan farklı (6 vs 8 renk) |
| Özel renk seçici + damlalık | `CaptureOverlayWindow.xaml.cs:2638`, `Windows/EyedropperOverlay.cs` | MISSING | — | HSV karesi, hex girişi, canlı önizleme, damlalık yok |
| Çizgi kalınlığı ayarı | `CaptureOverlayWindow.xaml.cs:2657,2668` | PARTIAL | `annotate.rs:112`, `overlay_chrome.rs:658` | **Export'ta uygulanmıyor**: `sf-app` `model::render` (sabit `LINE_WIDTH`) kullanır (`tray.rs:134`); `apply_hit`'te `Hit::Width` önizlemeye geçiyor ama `apply_hit` select sonrası no-op; `build_shape(..., _w)` `_w`'yi yok sayar (`annotate.rs:411`) |
| Ok uç boyutu | `CaptureOverlayWindow.xaml.cs:1677`, `ShapeItems.cs:145` | MISSING | — | — |
| Opaklık (0-100) | `CaptureOverlayWindow.xaml.cs:2670,1782` | MISSING | — | `Rgba.a` var ama öğe opaklığı UI'a bağlı değil |
| Yazı tipi ailesi | `CaptureOverlayWindow.xaml.cs:2767,2769` | MISSING | — | Tek gömülü bitmap font |
| Kalın / İtalik | `CaptureOverlayWindow.xaml.cs:1692-1702` | MISSING | — | — |
| Metin gölge/kontur/hizalama | `CaptureOverlayWindow.xaml.cs:1708,2807` | PARTIAL | `sf-draw/src/text.rs:80` | Yalnız ribbon; gölge, kontur, hizalama yok |
| Öğe seçme (tek) | `InteractiveCanvas.cs:692`, `Scene.cs:167` (`HitTest`) | MISSING | — | `Tool::Select` overlay'de no-op (`editor_window.rs:453`); `sf-app` paneline kalem çiziyor (`annotate.rs:424`) |
| Çoklu seçim / kement | `InteractiveCanvas.cs:949,1449` | MISSING | — | — |
| Öğe taşıma | `InteractiveCanvas.cs:915` | MISSING | — | — |
| Öğe boyutlandırma (8 tutamaç) | `InteractiveCanvas.cs:535,590,1205,1334` | MISSING | — | — |
| Grup boyutlandırma | `InteractiveCanvas.cs:1286,1316` | MISSING | — | — |
| Döndürme + Shift 15° | `InteractiveCanvas.cs:486,885`, `SceneItem.cs:17` | MISSING | — | — |
| Köşe yarıçapı tutamacı | `InteractiveCanvas.cs:504,894` | MISSING | — | — |
| Z-sırası + sağ tık menüsü | `InteractiveCanvas.cs:1005,1033`, `Scene.cs:137` | MISSING | — | — |
| Öğe silme | `InteractiveCanvas.cs:1456`, `CaptureOverlayWindow.xaml.cs:4428` | MISSING | — | — |
| Ctrl+C/V/D öğe kopyala/yapıştır/çoğalt | `CaptureOverlayWindow.xaml.cs:3775,3799,3849`, `SceneClipboard.cs:9` | MISSING | — | — |
| Seçili öğeyi panoya PNG kopyala | `CaptureOverlayWindow.xaml.cs:3786`, `ImageExporter.cs:116` | MISSING | (pano altyapısı: `sf-capture/src/clipboard.rs:8`) | — |
| Undo / Redo | `Scene.cs:106-132`, `CaptureOverlayWindow.xaml.cs:4401-4402` | PARTIAL | `annotate.rs:135` (`shapes.pop()`) | **Redo yok**; işlem bazlı değil; `sf-edit/src/history.rs:16` (`EditHistory`) hiçbir yerden çağrılmıyor |
| Sahneyi temizle | `CaptureOverlayWindow.xaml.cs:4158,1841` | PARTIAL | `annotate.rs:132` | Overlay'de Temizle butonu yok |
| Undo sonrası seçimi geri koyma | `Scene.cs:97,121,131` | MISSING | — | — |
| Araç kısayolları (V/R/O/A/L/P/H/T/S/B) | `CaptureOverlayWindow.xaml.cs:4442-4449` | PARTIAL | `editor_window.rs:309` | Yalnız Tab ile sırayla geçiş; tek-tuş kısayolları yok |
| Shift ile kısıtlama (kare / 45°) | `InteractiveCanvas.cs:1106,1118` | MISSING | — | — |
| Ok tuşlarıyla nudge, Ctrl+Z/Y/C/S/V, Delete | `CaptureOverlayWindow.xaml.cs:4401-4439` | MISSING | — | Enter/sağ tık/Esc dışında kısayol yok |
| Zoom / pan / pencereye sığdır | `Editor/CanvasViewTransform.cs:11`, `InteractiveCanvas.cs:375` | PARTIAL | `sf-edit/src/view_transform.rs:27` | **Hiçbir uygulama yolu çağırmıyor** (yalnız test); panelde basit "sığdır" ölçeği var |
| Büyüteç / loupe | `Windows/EyedropperOverlay.cs:57` | MISSING | — | — |
| PNG/JPEG/WebP kodlama | `Editor/ImageExporter.cs:15,43,50` | PORTED | `annotate.rs:457-505` | JPEG saydamlığı beyaza düzleştirme yok |
| Kalite (0-100) ayarının export'ta kullanımı | `ImageExporter.cs:15`, `CaptureOverlayWindow.xaml.cs:4233` | PARTIAL | `annotate.rs:469-495` | `jpeg_quality` ayarı (`sf-settings/src/lib.rs:669`) export'ta okunmuyor |
| Dosyaya kaydet | `CaptureOverlayWindow.xaml.cs:4215`, `ImageExporter.cs:292` | PORTED | `annotate.rs:475` | — |
| Panoya kopyala (dışa aktarım) | `CaptureOverlayWindow.xaml.cs:4201,1842` | MISSING | (altyapı: `sf-capture/src/clipboard.rs:8`) | Annotate panelinden panoya kopyalama yok |
| Export'ta kırpma uygulama | `CaptureOverlayWindow.xaml.cs:4177-4193`, `SceneRenderer.cs:32` | MISSING | — | — |
| Boş sahne placeholder ("resim yapıştırın") | `CaptureOverlayWindow.xaml:116` | MISSING | — | — |
| Serbest mod arkaplan rengi | `CaptureOverlayWindow.xaml.cs:1527,2603` | MISSING | — | Kolaj export'u sabit `#12151E` (`collage.rs:339`) |

### 1.4 GIF kayıt + GIF editörü

| Özellik | C# kaynak | Rust durum | Rust kaynak | Eksik olan |
|---|---|---|---|---|
| GIF kaydı başlat | `Gif/GifRecorder.cs:257`, `Windows/RecordingLauncher.cs:17` | PORTED | `sf-app/src/panels/record.rs:200`, `panels/record_worker.rs:80` | — |
| GIF kaydını durdur | `Gif/GifRecorder.cs:316`, `RecordingOverlayWindow.cs:273` | PORTED | `record.rs:186` (İptal), `record_overlay.rs` (Durdur) | — |
| Duraklat / Devam et | `GifRecorder.cs:292,304` | PORTED | `sf-record/src/session.rs:70,74`, `record.rs:177` | — |
| Yeniden başlat | `GifRecorder.cs:348` | PORTED | `session.rs:84`, `record_worker.rs:139` | Kareler atılır, aynı dosyaya devam edilir (C# de öyle) |
| Kayıt durum overlay'i (süre/fps/bellek) | `RecordingOverlayWindow.cs:372,386` | PORTED | `record_overlay.rs:59` (ayrı egui viewport'u) | `WDA_EXCLUDEFROMCAPTURE` yok; çubuk kayıt bölgesinin üstüne konumlanır |
| FPS ayarı (kayıtta etkin) | `Settings/AppSettings.cs:198`, `GifRecorder.cs:102` | PORTED | `record.rs:229` → `settings.gif.fps` / `settings.video.fps` | — |
| Kayıt bölgesi | `GifRecorder.cs:172` | PORTED | `overlay_window.rs` (seçici) → `record_worker.rs:96` (`capture_region`) | — |
| İmleç yakalama (kayıtta) | `GifRecorder.cs:153`, `Gif/Input/CursorCapture.cs:73` | PORTED | `sf-capture/src/overlay_window.rs:draw_cursor`, `record_worker.rs:143` | GDI `DrawIconEx` ile; tıklama vurgusu/halka yok |
| Klavye/tuş overlay'i (GIF üstüne tuş etiketleri) | `GifRecorder.cs:159`, `Gif/Input/KeyLabels.cs:11`, `InputOverlayRenderer.cs:82` | MISSING | kısmi: `sf-capture/src/key_labels.rs:8` | Piksel render'ı ve `FrameInput` üreticisi yok; `key_labels` yalnız test çağrısı |
| Girdi olayı zamanlaması (kare başına girdi) | `GifRecorder.cs:193,215`, `Gif/Input/FrameInput.cs:7` | MISSING | yapı: `sf-edit/src/frame.rs:19` | Gerçek hook'tan `FrameInput` üreten yok |
| Kare deposu / bellek limiti | `Gif/FrameStore.cs:20`, `GifRecorder.cs:31,162` | PORTED | `sf-gif/src/frame_store.rs:11`, limit `record.rs:262` | GIF kareleri diske akar; limit yakalanan ham bayt üzerinden ölçülür |
| Nicemleyici seçimi (Neural/Octree) | `Gif/Encoder/QuantizerType.cs`, `NeuralQuantizer.cs:13` | PARTIAL | `sf-gif/src/quantize.rs:51,282` | Motor portlanmış ama GUI her yerde `QuantizerType::Octree` sabitliyor (`panels/gif.rs:352`) |
| Palet / renk sayısı / dither / global palet | `Gif/Encoder/GifFile.cs:18,27,30`, `GifEditorWindow.Panels.cs:154,182` | PARTIAL | `sf-gif/src/gif_file.rs:32,38,39` | `panels/gif.rs:351` 256'yı sabitliyor; dither/global/tolerance hiç set edilmiyor |
| Döngü / tekrar sayısı | `GifFile.cs:15`, `GifEditorWindow.xaml.cs:868` | PARTIAL | `sf-gif/src/gif_file.rs:29` | UI/CLI bayrağı yok; her zaman sonsuz döngü |
| GIF export / kaydet | `GifRecorder.cs:649,656`, `GifFile.cs:75` | PORTED | `sf-gif/src/gif_file.rs:97` | — |
| Kare listesi + küçük resimler | `GifEditorWindow.xaml:442`, `.xaml.cs:623`, `ThumbnailCache.cs:44` | MISSING | `panels/gif.rs:110` (tek statik önizleme) | Zaman çizelgesi ve küçük resim yok |
| Küçük resim önbelleği | `Gif/Editing/ThumbnailCache.cs:19,33,44` | MISSING | — | — |
| Kare sil / çoğalt | `GifEditorWindow.xaml.cs:518,534`, `FrameOperations.cs:44,121` | PARTIAL | `sf-edit/src/frame_ops.rs:42,140` | UI/CLI yolu yok; panelde yalnız "Kare azalt" |
| Kare başına gecikme/süre | `FrameOperations.cs:175,179` | PARTIAL | `sf-edit/src/frame_ops.rs:205,214` | UI çağırmıyor |
| Kareleri ters çevir | `FrameOperations.cs:141`, `GifEditorWindow.xaml.cs:232` | PORTED | `frame_ops.rs:162`, `panels/gif.rs:148,344` | Önizleme ters kareyi göstermiyor (yalnız yeniden kodlamada uygulanır) |
| Kare sırası değiştir | `FrameOperations.cs:149,161` | PARTIAL | `frame_ops.rs:169,186` | UI/CLI çağrısı yok |
| Aralık kırp (trim) | `FrameOperations.cs:106`, `GifEditorWindow.xaml.cs:587` | PARTIAL | `frame_ops.rs:127` | UI/CLI çağrısı yok |
| Kare azalt (keep:drop) | `FrameOperations.cs:208` | PORTED | `frame_ops.rs:245`, `panels/gif.rs:145` | — |
| Yinelenen kareleri kaldır | `FrameOperations.cs:246` | PARTIAL | `frame_ops.rs:281,358` | Benzerlik kaydırıcısı / "tık-kare koru" UI'ı yok |
| Döngüyü yumuşat | `FrameOperations.cs:287` | PARTIAL | `frame_ops.rs:330` | UI/CLI çağrısı yok |
| Hız değiştir | `FrameOperations.cs:186` | PARTIAL | `frame_ops.rs:223` | UI/CLI çağrısı yok |
| Tüm karelere FPS uygula | `FrameOperations.cs:196` | PARTIAL | `frame_ops.rs:237` | UI/CLI çağrısı yok |
| Kareleri kırp | `ImageOperations.cs:25,64,90` | PARTIAL | `sf-edit/src/image_ops.rs:40,102,144` | Interaktif kırpma UI'ı ve GIF CLI kırpma yok |
| Kareleri yeniden boyutlandır | `ImageOperations.cs:39,136` | PARTIAL | `sf-edit/src/image_ops.rs:71,194` | GIF panelinde boyut kontrolleri yok |
| Kareleri döndür (±90°) | `ImageOperations.cs:35,104` | PARTIAL | `sf-edit/src/image_ops.rs:58,159` | UI/CLI çağrısı yok |
| Oynatma önizlemesi (oynat/duraklat/döngü) | `GifEditorWindow.xaml.cs:827,833,855` | MISSING | `panels/gif.rs:110` | Yalnız statik ilk kare |
| Altyazı / metin overlay'i (export) | `OverlayOptions.cs:44`, `OverlayRenderer.cs:37` | MISSING | — | — |
| İlerleme/zaman damgası overlay'i | `OverlayOptions.cs:67`, `OverlayRenderer.cs:149` | MISSING | — | — |
| Kenarlık overlay'i | `OverlayOptions.cs:103` | MISSING | — | — |
| Filigran (metin veya logo) | `OverlayOptions.cs:115`, `OverlayRenderer.cs:253` | MISSING | — | — |
| Overlay set kompozisyonu | `OverlayOptions.cs:147`, `GifRecorder.cs:674` | MISSING | — | — |
| Editörde girdi overlay önizlemesi | `GifEditorWindow.Panels.cs:759` | MISSING | — | — |
| Karelere açıklama (rect/elips/ok/kalem/metin/adım/blur) | `GifEditorWindow.Annotations.cs:128,150`, `AnnotationTrack.cs:15`, `AnnotationCompositor.cs:29` | MISSING | temel: `sf-draw/src/model.rs:12,167` | Rust'ta `SceneItem`/`AnnotationTrack` karşılığı yok; kareye bağlı açıklama modeli ve export kompozisyonu yok |
| Nesne/seçim paneli | `GifEditorWindow.ObjectPanel.cs:33,279,335` | MISSING | — | Sahne modeli olmadığı için tamamen yok |
| Nesne keyframe klibi | `Gif/Editing/ObjectClip.cs:14,149,187` | MISSING | — | — |
| Açıklama undo/redo | `GifEditorWindow.Annotations.cs:376,391` | MISSING | — | — |
| Editör belgesi + geçmiş | `Gif/Editing/EditorDocument.cs:84,152`, `EditHistory.cs:22` | PARTIAL | `sf-edit/src/history.rs:16` | `EditHistory` hiç oluşturulmuyor; belge/snapshot konteyneri yok |
| Export (her şey kompozit) | `GifEditorWindow.Panels.cs:1019` | PARTIAL | `panels/gif.rs:334` | Yalnız azalt+ters; açıklama/girdi/overlay/boyut/palet/progress/iptal/aralık yok |
| Export boyut tahmini | `GifEditorWindow.Panels.cs:1178` | MISSING | — | — |
| LZW kodlayıcı | `Gif/Encoder/LzwEncoder.cs:50` | PORTED | `sf-gif/src/lzw.rs:64` | — |
| GIF89a yazıcı | `Gif/Encoder/GifFile.cs:47,75,368` | PORTED | `sf-gif/src/gif_file.rs:51-305` | — |
| Palet haritası | `Gif/Encoder/PaletteMap.cs:43,56` | PORTED | `sf-gif/src/palette.rs:55,68` | — |
| Değişmeyen piksel optimizasyonu (delta) | `GifRecorder.cs:867,936,1042` | MISSING | — | Tüm kareler tam tuval yazılıyor |
| Zamanlayıcı çözünürlüğü (timeBeginPeriod) | `Gif/TimerResolution.cs:31,46` | MISSING | — | `timeBeginPeriod` hiç yok |

### 1.5 Video kayıt + ses

| Özellik | C# kaynak | Rust durum | Rust kaynak | Eksik olan |
|---|---|---|---|---|
| Ekran video kaydı (bölge/tam ekran) | `Windows/RecordingLauncher.cs:34`, `Record/VideoRecorder.cs` | PARTIAL | `sf-app/src/panels/record.rs:211`, `sf-cli/src/main.rs:116` | Yalnız tüm sanal ekran; bölge sınırlı video yolu yok |
| Durdur / duraklat / devam | `VideoRecorder.cs:129,109,119` | MISSING | — | Sabit süreli bloklama döngüsü |
| Yeniden başlat | `VideoRecorder.cs:152` | MISSING | — | — |
| Kayıt overlay'i (kesikli çerçeve, süre, fps, bellek, ses metreleri, Esc/Space) | `RecordingOverlayWindow.cs:66,254,405` | MISSING | — | Pencere yok; panelde yalnız "kaydediliyor..." + İptal |
| Kayıt geri sayımı | `RecordingCountdown.cs:16` | MISSING | — | — |
| Kayıt sonrası sonuç penceresi (oynat/seek/kaydet/sil) | `VideoResultWindow.cs:31,97,387,414` | MISSING | — | Yol diyalogu + toast dışında bir şey yok |
| H.264/MP4 kodlama | `Record/MfH264Writer.cs:20,95,194` | PORTED | `sf-record/src/mf.rs:31,42,114,156` | — |
| MP4'e ses (AAC mux) | `MfH264Writer.cs:129,153`, `VideoRecorder.cs:346` | MISSING | — | `Mp4Writer` yalnız video akışı ekler; ses çıkış tipi yok |
| Kodlayıcı yedekleme (GPU/CPU) + kurtarma | `Record/EncoderFallback.cs:10,28,40`, `VideoRecorder.cs:445` | MISSING | — | Donanım dönüşüm öznitelikleri ve yeniden deneme yok |
| Bit hızı seçimi | `Record/VideoBitrate.cs:7` | PARTIAL | `sf-media/src/bitrate.rs:7` | Portlanmış ama her zaman `VideoQuality::High` sabit (`record.rs:223`) |
| Kare hızı seçimi | `RecorderPickerPopup.cs:268` (15/24/30/60) | PARTIAL | `record.rs:91` | 30 ile sınırlı; ayara (VideoSettings.fps) yazılmıyor |
| Video geometri/ölçekleme | `VideoGeometry.cs:5,16`, `VideoRecorder.cs:552` | PARTIAL | `sf-media/src/geometry.rs:7,18` | Ölçekleme nearest yerine bilineer; kare-başına kopya hızlı yolu yok |
| Mikrofon yakalama | `AudioDevices.cs:9,28`, `AudioMixer.cs:127` | PARTIAL | `sf-record/src/wasapi.rs:35` | Yalnız varsayılan aygıt; numaralandırma/cihaz listesi yok; uygulamadan çağrı yok |
| Sistem sesi / loopback | `Record/SystemLoopback.cs:10,22` | PARTIAL | `sf-record/src/wasapi.rs:30` | Rust render-loopback kullanır; C# süreç-loopback (`VAD\Process_Loopback`, Bluetooth takılmasını önler) yok; uygulamadan çağrı yok |
| Ses karıştırma (mic+system, peak meter) | `Record/AudioMixer.cs:7,211,178` | MISSING | — | Yalnız ham `read()` var |
| Ses dönüştürme / resample | `Record/AudioConvert.cs:88,9,36` | PORTED | `sf-media/src/audio.rs:20,89,125` | Modül sadık ama hiçbir yakalama yoluna bağlı değil |
| Ses aygıtı seçimi | `RecorderPickerPopup.cs:369`, `AudioMixer.cs:62` | MISSING | — | — |
| Kayıt modu seçici popup | `RecorderPickerPopup.cs:12,249,263` | MISSING | — | Kalite/imleç/tık vurgusu/geri sayım/ses anahtarları UI'da yok |

### 1.6 Sunum / mürekkep katmanı

| Özellik | C# kaynak | Rust durum | Rust kaynak | Eksik olan |
|---|---|---|---|---|
| Sunum katmanını aç | `Presenter/PresenterService.cs:139` | PORTED | `sf-capture/src/presenter_window.rs:79`, çağrı `sf-app/src/panels/capture.rs:228` | — |
| Kalemle çizim | `PresenterService.cs:227,238,262` | PORTED | `presenter_window.rs:120,111,126` | — |
| Lazer izi | `Presenter/LaserTrail.cs:5,17,36`, `PresenterRenderer.cs:311` | PARTIAL | `sf-overlay/src/presenter.rs:57`, `sf-draw/src/presenter.rs:14` | `MaxLength` trim yok; iki geçişli yumuşak parıltı yok |
| Eğmeli ok | `Presenter/ArrowBend.cs:9`, `PresenterService.cs:182` | PARTIAL | `sf-overlay/src/arrow.rs:12` | Matematik portlanmış+testli ama **uygulamadan hiç çağrılmıyor**; ok aracı ve eğme tuşu yok |
| Mürekkep güzelleştirme (şekle oturtma, el yazısı→metin) | `Presenter/InkBeautifier.cs:13,85,158`, `InkStrokeFit.cs:8,37,72` | MISSING | — | Hiç karşılığı yok |
| Mürekkebi temizle | `PresenterRenderer.cs:97`, `PresenterService.cs:142,344` | PARTIAL | `presenter_window.rs:126` | Açık "temizle" eylemi yok; aynı aracı yeniden seçmek temizlemiyor |
| Çizimi geri al (Ctrl+Z) | `PresenterRenderer.cs:108`, `PresenterService.cs:104` | MISSING | — | — |
| Araç değiştirme kısayolları | `App.xaml.cs:198-216` | PARTIAL | `presenter_window.rs:307-309` | Yalnız 1/2/3 sabit; rect/elips/çizgi/ok yok; `PresenterSettings` kısayolları okunmuyor |
| Renk değiştirme tuşları | `PresenterService.cs:73,95` | MISSING | — | Renk sabit `Rgba::rgb(0xEA6F12)` (`presenter_window.rs:105`) |
| Spot ışığı | `PresenterService.cs:161`, `PresenterRenderer.cs:281` | PARTIAL | `presenter_window.rs:39`, `sf-draw/src/presenter.rs:64` | `toggle_spotlight()` bool'u çeviriyor ama `paint` hiç okumuyor — spot hiç çizilmiyor; yarıçap animasyonu yok |
| Boşta otomatik kapanma | `PresenterService.cs:315` | MISSING | — | — |

### 1.7 Renk seçici / damlalık

| Özellik | C# kaynak | Rust durum | Rust kaynak | Eksik olan |
|---|---|---|---|---|
| Piksel rengi seç (damlalık) | `Windows/EyedropperOverlay.cs:62,200,235`, `ColorPickerOverlayWindow.cs:34` | MISSING | — | `GetPixel`/ekran örnekleme yok; tepsi "Renk Seçici" stub |
| Büyüteç + hex okuma | `EyedropperOverlay.cs:56,142` | MISSING | — | — |
| Hex/RGB/HSL kopyala | `ColorPickerOverlayWindow.cs:41,243,256` | MISSING | — | — |
| Renk sonuç paneli (yeniden seç/kapat) | `ColorPickerOverlayWindow.cs:94,229` | MISSING | — | — |
| HSV seçici popup (SV karesi + hue + hex girişi) | `Windows/ColorPickerPopup.cs:16,228,244,259` | PARTIAL | `overlay_chrome.rs:66`, `annotate.rs:29` | Yalnız sabit renk çipleri; HSV karesi, hue şeridi, hex girişi, damlalık yok |
| Sayısal sürükleme (Figma tarzı scrub) | `Windows/NumericDrag.cs:12,14,25` | MISSING | — | Win32 penceresinde scrub yok; egui `DragValue` farklı bir kavram |

### 1.8 Ayarlar / tepsi / güncelleme

| Özellik | C# kaynak | Rust durum | Rust kaynak | Eksik olan |
|---|---|---|---|---|
| Ayarlar penceresi | `Windows/SettingsWindow.xaml:31`, `.xaml.cs:210` | PARTIAL | `sf-app/src/panels/settings.rs:6`, `sf-capture/src/settings_window.rs:199` | İki yüzeysel yüz; ikisi de çeviri/presenter/altyazı/araç stili/mic cihazı/otomatik kapatma sunmuyor |
| Ayar modeli yükle/kaydet | `Settings/AppSettings.cs:75-178` | PORTED | `sf-settings/src/lib.rs:685-762` | — |
| İmleci göster | `AppSettings.cs:15` | PARTIAL | `settings.rs:9` | Düzenlenir+kaydedilir ama hiç okunmaz (`screen.rs:75` imleç bayrağı almıyor) |
| Yükleme sonrası bağlantı kopyala | `AppSettings.cs:16` | PORTED | `settings.rs:12`, okuma `panels/upload.rs:106` | — |
| Yükleme penceresini otomatik kapat | `AppSettings.cs:17` | MISSING | alan: `sf-settings/src/lib.rs:626` | UI yok, okuyucu yok |
| Windows açılışında başlat | `Settings/StartupManager.cs:14-51` | PORTED | `sf-settings/src/startup.rs:26`, `tray.rs:16` | — |
| Çıktı biçimi | `AppSettings.cs:29` | PORTED | `settings.rs:19` | — |
| Kalite | `AppSettings.cs:30` | PORTED | `settings.rs:28`, `capture.rs:248` | — |
| Kayıt klasörü + gözat | `AppSettings.cs:31`, `SettingsWindow.xaml.cs:1077` | PORTED | `settings.rs:34-39` | Win32 pencerede serbest metin, gözat butonu yok |
| Kısayol ayarları (5) | `AppSettings.cs:21-25` | PARTIAL | `settings.rs:98-117` | Yalnız `region_hotkey` kayıtlı/okunur; diğer 4'ü kalıcı+gösterilir ama hiç okunmaz |
| GIF ayarları (fps/renk/nicemleyici/dither/global) | `AppSettings.cs:196-212` | PARTIAL | `settings.rs:44-72` | Düzenlenir, **hiç okunmaz**; motor 256/Octree sabitliyor |
| GIF imleç/tık vurgusu/tuşlar/yarıçap | `AppSettings.cs:229-244` | MISSING | — | UI ve okuyucu yok |
| GIF optimizasyon/tolerans/max bellek | `AppSettings.cs:215-224` | MISSING | — | — |
| Video ayarları (fps/kalite) | `AppSettings.cs:473-476` | PARTIAL | `settings.rs:77-85` | Düzenlenir, **hiç okunmaz**; kayıt `VideoQuality::High` sabitliyor |
| Video imleç/tık/sistem sesi/mikrofon/geri sayım | `AppSettings.cs:477-482` | MISSING | — | UI ve okuyucu yok |
| Tepsi: Bölge Yakala | `Tray/TrayIconService.cs:59` | PORTED | `tray.rs:49-52` | — |
| Tepsi: Tam Ekran Yakala | `TrayIconService.cs:60` | PORTED | `tray.rs:54-58` | — |
| Tepsi: Serbest / Yerleştirme | `TrayIconService.cs:61` | MISSING | — | — |
| Tepsi: Renk Seçici | `TrayIconService.cs:62` | PARTIAL (stub) | `tray.rs:60-63` | Yalnız `eprintln!` |
| Tepsi: Yükleme Yap | `TrayIconService.cs:63` | MISSING | — | — |
| Tepsi: Hızlı Çeviri | `TrayIconService.cs:64` | PARTIAL (stub) | `tray.rs:64-67` | Yalnız `eprintln!` |
| Tepsi: Arka planını kaldır | `TrayIconService.cs:65` | MISSING | — | — |
| Tepsi: Sunum araçları toggle | `TrayIconService.cs:67,89-105` | MISSING | — | — |
| Tepsi: Canlı altyazı toggle | `TrayIconService.cs:68,107-123` | MISSING | — | — |
| Tepsi: Ayarlar | `TrayIconService.cs:70` | PORTED | `tray.rs:68-75` | — |
| Tepsi: Hakkında | `TrayIconService.cs:71` | MISSING | — | — |
| Tepsi: Çıkış | `TrayIconService.cs:73` | PORTED | `tray.rs:76-79` | — |
| Tepsi: güncelleme öğesi (dinamik) | `TrayIconService.cs:142-178` | MISSING | — | — |
| Tepsi: sol tık = bölge yakala | `TrayIconService.cs:44` | PORTED | `tray.rs:83-92` | — |
| Tepsi: balon mesajı | `TrayIconService.cs:216` | MISSING | — | `NIF_INFO`/`NIM_MODIFY` yok |
| İlk çalıştırma karşılaması | `App.xaml.cs:88-93` | MISSING | — | — |
| Otomatik güncelleme — kontrol | `Updates/AutoUpdateService.cs:71-121` | MISSING (uygulamada) | `sf-net/src/update.rs:47` | Yalnız `screenforge update` CLI; `sf-app` hiç çağırmıyor |
| Otomatik güncelleme — indir/kur | `AutoUpdateService.cs:33-69` | MISSING (uygulamada) | `update.rs:100,120` | CLI dışında çağrı yok |
| Hakkında penceresi | `Windows/AboutWindow.xaml.cs:7` | MISSING | — | — |
| Durum toast'ı (meşgul/OCR) | `Windows/StatusToastWindow.cs` | PARTIAL | `app.rs:236,325` | Yalnız egui toast; spinner/yüksek-pencere yok |
| Yükleme toast'ı (ilerleme + bağlantı + aç) | `UploadToastWindow.xaml.cs:19,37,49` | PARTIAL | `panels/upload.rs:61-96` | İlerleme çubuğu ve "Aç" butonu yok |
| Tek örnek koruması | `App.xaml.cs:41-54` | MISSING | — | Mutex/IPC yok |
| Çıktı klasörü seçimi | `SettingsWindow.xaml.cs:1077` | PORTED | `settings.rs:35-39` | — |
| Dosya adlandırma şablonu | `ImageExporter.cs:295` | PARTIAL | `tray.rs:147` | Zaman damgası yerine sayaç (`ScreenForge-{n}.png`); şablon yok |
| Koyu başlık çubuğu (Win11) | `Windows/DarkTitleBar.cs:12` | MISSING | — | DWM çağrısı yok |

### 1.9 Çeviri / ters görsel arama / buluta yükleme

| Özellik | C# kaynak | Rust durum | Rust kaynak | Eksik olan |
|---|---|---|---|---|
| Hızlı çeviri penceresi | `Windows/QuickTranslateWindow.xaml.cs:42,47` | PARTIAL | `sf-app/src/panels/translate.rs:29` | Ayrı üstte-kalan pencere yok; normal egui sekmesi, yalnız `--window` ile açılır; debounce yok |
| Ön plandaki seçimi oku (Ctrl+C zinciri) | `Translate/ForegroundSelectionReader.cs:43` | MISSING | — | Hiç portlanmadı |
| Çeviri çağrısı (4 uç noktalı zincir) | `Translate/GoogleTextTranslateClient.cs:47,83,250,289` | PARTIAL | `sf-net/src/translate.rs:24` | Yalnız `dict-chrome-ex` ucu; çok satırlı toplu çeviri ve sonuç önbelleği yok |
| Hedef dil seçimi | `QuickTranslateWindow.xaml.cs:255` | PORTED | `panels/translate.rs:52` | — |
| Dil değiştirme (ikinci geçiş) | `Translate/TranslateLanguageRouter.cs:10` | PARTIAL | `sf-net/src/lang_router.rs:6` | Router **uygulamadan çağrılmıyor** (yalnız test) |
| Sonucu kopyala | `QuickTranslateWindow.xaml.cs:214` | PARTIAL | `panels/translate.rs:135` | Ctrl+C klavye kısayolu yok |
| Pencere yerleşimi (imleç monitörü) | `QuickTranslateWindow.xaml.cs:145` | MISSING | — | — |
| Dil listesi (16 dil) | `Settings/TranslateLanguageDefaults.cs:8,56` | PORTED | `sf-settings/src/lang.rs:4,49` | — |
| Görsel/OCR çeviri (bölge seç + OCR + resim üstüne yaz) | `CaptureOverlayWindow.xaml.cs:2290,2340`, `TranslateOverlayRenderer.cs:10` | MISSING | — | Uygulamada hiç yok |
| Lens istek kurucu (protobuf) | `Translate/GoogleLensClient.cs:130,189`, `ProtoWriter.cs:13` | PARTIAL | `sf-net/src/lens.rs:56,73`, `proto.rs:8` | **Uygulamadan çağrılmıyor** (yalnız test) |
| Lens yanıt ayrıştırma | `ProtoResponseParser.cs:45` | MISSING | — | `sf-net` yalnız istek kurucu; yanıt çözücü yok |
| Görsel çeviri istemcisi (translate-pa) | `GoogleTranslateVisualClient.cs:107` | MISSING | — | — |
| Çevrilmiş metni resme bindirme | `TranslateOverlayRenderer.cs:10` | MISSING | — | — |
| OCR yalnız ("metin çıkarma") | `CaptureOverlayWindow.xaml.cs:2529` | MISSING | — | — |
| Son kullanılan diller menüsü | `VisualTranslatePickerPopup.cs:29,51,73`, `Settings/VisualTranslateRecents.cs:23` | PARTIAL | `sf-settings/src/recents.rs:12` | Model **uygulamadan çağrılmıyor** (yalnız test); popup yok |
| Ters görsel arama motorları (Yandex/Lens) | `Search/ReverseImageSearch.cs:11,46` | PARTIAL | `sf-net/src/reverse_http.rs:20,27` | Uygulama UI'ı yok; yalnız `sf-cli/src/main.rs:149` |
| Ters arama: bölgeyi girdi al | `CaptureOverlayWindow.xaml.cs:2437` | MISSING | — | — |
| Ters arama: sonucu tarayıcıda aç | `ReverseImageSearch.cs:59` | PARTIAL | `sf-cli/src/main.rs:158` | Rust URL yazdırır, açmaz |
| Lens HTML yükleme | `ReverseImageSearch.cs:72` | PORTED | `sf-net/src/reverse_search.rs:13` | Yalnız CLI çağrısı |
| Yandex CBIR ayrıştırma | `ReverseImageSearch.cs:94` | PORTED | `sf-net/src/reverse_search.rs:35` | Yalnız CLI çağrısı |
| Bulut sağlayıcı (Litterbox, 72s) | `Upload/LitterboxUploadProvider.cs:12,60` | PORTED | `sf-net/src/http_upload.rs:16` | `IUploadProvider` soyutlaması bırakılmış (sabit Litterbox) |
| Yüklenen bağlantıyı kopyala | `UploadToastWindow.xaml.cs:18`, `App.xaml.cs:391` | PORTED | `panels/upload.rs:82,106` | — |
| Yükleme sonrası boş-link kontrolü | `LitterboxUploadProvider.cs:119` | MISSING | `sf-net/src/upload.rs:14` | `check_allowed`/`upload_file`'ın app veya CLI çağrısı yok |
| Yükleme tetikleyicisi (tepsi/hotkey/dosya seç) | `TrayIconService.cs:63`, `App.xaml.cs:166,294` | PARTIAL | `panels/capture.rs:112` | Tepsi öğesi, yükleme kısayolu, dosya seçici yok |

### 1.10 Yapay zekâ arka plan + altyazı + kolaj

| Özellik | C# kaynak | Rust durum | Rust kaynak | Eksik olan |
|---|---|---|---|---|
| ONNX u2netp özne bölütleme | `Background/LocalSubjectSegmenter.cs:20,25,63` | PARTIAL | `sf-bg/src/model.rs:30,42` | Gerçek çıkarım yalnız `onnx` Cargo özelliği arkasında; sadece CLI'dan erişilebilir (`sf-cli/src/main.rs:202`); `sf-app`'in `sf-bg` bağımlılığı yok |
| Windows AI nesne çıkarıcı | `ScreenForge.WindowsAI/SubjectSegmenter.cs:24`, `.csproj:22` | MISSING | — | Windows App SDK AI bağlaması yok |
| Bölütleyici dağıtıcı (WinAI → u2netp yedek) | `Background/BackgroundSegmenter.cs:16,23-45` | MISSING | — | — |
| Arka planı kaldır → alfa (saydam) | `Background/BackgroundCompositor.cs:32` | MISSING | `sf-bg/src/lib.rs:20` | `composite_on_solid` her zaman `out[i+3]=255` (`lib.rs:35`); saydamlık çıkışı yok |
| Arka plan değiştir — düz renk | `BackgroundCompositor.cs:39-41` | PARTIAL | `sf-bg/src/lib.rs:20` | Var ama yalnız CLI (`sf-cli/src/main.rs:204`) |
| Arka plan değiştir — resim (cover) | `BackgroundCompositor.cs:43,161` | MISSING | — | — |
| Maskeyi yumuşatma (feather) | `BackgroundCompositor.cs:128-159` | PARTIAL | `sf-bg/src/lib.rs:27` | Karışım var ama CLI yolunda; maske yeniden boyutlandırma nearest (C# linear) |
| Canlı maske önizlemesi | `Windows/BackgroundEditWindow.xaml.cs:154` | MISSING | — | — |
| Arka plan seçim diyaloğu | `Windows/BackgroundChoiceDialog.xaml.cs:13`, `CaptureOverlayWindow.xaml.cs:4169` | MISSING | — | — |
| Arka plan düzenleme penceresi | `BackgroundEditWindow.xaml.cs:30,84,325` | MISSING | — | — |
| Kabuk/Explorer sağ tık menüsü | `Background/ShellBackgroundMenu.cs:20-48`, `App.xaml.cs:99` | MISSING | — | `rust/setup-rust.iss` içinde shell verb yok |
| Tepsi arka plan + IPC + `--edit-background` | `TrayIconService.cs:65`, `App.xaml.cs:40,135`, `BackgroundEditBridge.cs:11` | MISSING | — | — |
| Kesilen görüntüyü kaydet | `BackgroundFiles.cs:54,75` | PARTIAL | `sf-cli/src/main.rs:204` | Yalnız headless CLI; uygulama kaydı yok |
| Model dosyası sevk ediliyor mu | `ScreenForge.csproj:70`, `setup.iss:40` | PARTIAL | `sf-bg/u2netp.onnx` | Model `.gitignore:51`'de; build betiği/installer sevk etmiyor (`rust/setup-rust.iss:5` "ONNX/ort yok") |
| Altyazı bölgesi seçimi | `Subtitle/SubtitleRegionPicker.cs:34` | MISSING | — | — |
| Altyazı OCR döngüsü (Windows OCR + Lens, 600 ms) | `Subtitle/SubtitleService.cs:145`, `SubtitleOcr.cs:14` | MISSING | — | — |
| Altyazı overlay penceresi | `Subtitle/SubtitleOverlayWindow.cs:61,197,259` | PARTIAL | `sf-capture/src/subtitle_window.rs:65,74` | Statik: metin açılışta sabit, canlı güncelleme yok, taşıma/boyutlandırma yok, `WDA_EXCLUDEFROMCAPTURE` yok; yalnız CLI |
| Altyazı temaları (16) | `Subtitle/SubtitleThemes.cs:27-45` | PARTIAL | `subtitle_window.rs:13-60` | Tema kataloğu ve seçici UI yok |
| Altyazı stil penceresi | `Subtitle/SubtitleStyleWindow.cs:27` | MISSING | — | — |
| Altyazı ipucu penceresi | `Subtitle/SubtitleHintWindow.cs:23` | MISSING | — | — |
| Altyazı isabet hook'u (çift tık düzenle) | `Subtitle/SubtitleHitHook.cs:5`, `SubtitleService.cs:255` | PARTIAL | `subtitle_window.rs:185` | Tık-geçirgenlik var; global hook ve çift-tık düzenleme yok |
| Altyazı matematiği/geometrisi | `Subtitle/SubtitleMath.cs` | PORTED | `sf-overlay/src/subtitle.rs:11-258` | — |
| Altyazı servis yaşam döngüsü | `Subtitle/SubtitleService.cs:9-302` | MISSING | — | Servis nesnesi yok |
| Altyazı kısayolu / tepsi toggle | `TrayIconService.cs:107`, `App.xaml.cs:188` | MISSING | — | — |
| Kolaj serbest tuval | `CaptureOverlayWindow.xaml.cs:26,574,623` | PORTED | `sf-app/src/panels/collage.rs:30,80`, `app.rs:64` | — |
| Kolaj: resim yapıştır (Ctrl+V) | `CaptureOverlayWindow.xaml.cs:4427,3816` | PORTED | `collage.rs:56,224` | — |
| Kolaj: çoklu resim | `Scene.Items` | PORTED | `collage.rs:256`, `app.rs:164` | — |
| Kolaj: tuval arkaplan rengi | `CaptureOverlayWindow.xaml.cs:1382,1535,2603` | PARTIAL | `collage.rs:84,339` | Kullanıcı seçemiyor; export sabit `#12151E` |
| Kolaj: katman sırası | Sahne z-sırası + undo | PARTIAL | `collage.rs:144` | Katman listesi ve açık yeniden sıralama yok; undo/redo yok |
| Kolaj: export (kopyala/kaydet/yükle/saydam/kırp) | `CaptureOverlayWindow.xaml.cs:4295,4181,4169` | PARTIAL | `collage.rs:282-321` | Yalnız PNG, opak; pano/yükleme/saydamlık/kırpma seçimi yok |

---

## 2. Kullanıcı etkisine göre sıralı: MISSING + PARTIAL

Sıra "kullanıcının ilk fark edeceği" ölçütüne göredir.

| # | Özellik | Durum | Neden can yakar |
|---|---|---|---|
| 1 | Yerleştirilmiş öğeyi seçme / taşıma / boyutlandırma / döndürme / z-sıra / silme | MISSING | `Tool::Select` hem overlay'de (`editor_window.rs:453`) hem panelde (`annotate.rs:424`) no-op; yanlış yere kutu çizen kullanıcı hiçbir düzeltme yapamaz, tek çare Temizle |
| 2 | Overlay metin aracı | PARTIAL | Metin girişi yok, sabit `"TEXT"` yazıyor (`editor_window.rs:428`); overlay'de metin aracı işe yaramaz |
| 3 | Renk seçici / damlalık | MISSING | Tepsi öğesi stub (`tray.rs:62`); palet dışı renk seçilemez, ekrandan renk alınamaz |
| 4 | Çizgi kalınlığı ve dolgu ayarının çizime yansıması | PARTIAL | Kalınlık çipleri önizlemeyi değiştirir ama export sabit `LINE_WIDTH` kullanır (`tray.rs:134`); `build_shape(..., _w)` `_w`'yi yok sayar (`annotate.rs:411`); dolgu çipi tamamen no-op (`editor_window.rs:379`) |
| 5 | Undo / Redo | PARTIAL | Yalnız "son şekli at" var (`annotate.rs:135`); redo yok; `EditHistory` ölü kod |
| 6 | GIF kaydı bölgesel değil + duraklat/yeniden başlat yok | PARTIAL/MISSING | `record_gif` bölgeyi yok sayıp tüm sanal ekranı kaydeder (`record.rs:173`); yalnız sabit süre |
| 7 | Video kaydında ses ve durdur/duraklat yok | MISSING | `Mp4Writer` yalnız video akışı; `sf-record`/`sf-media` app'ten çağrılmıyor → MP4 her zaman sessiz, kayıt elle durdurulamaz |
| 8 | Görsel (OCR/Lens) çeviri | MISSING | Bölge seçip ekrandaki metni çevirme akışının tamamı yok |
| 9 | Arka planı kaldırma (AI) | MISSING/PARTIAL | Tepsi öğesi yok; `sf-app`'in `sf-bg` bağımlılığı yok; yalnız CLI; saydam çıktı üretilemiyor |
| 10 | Canlı altyazı | MISSING | Bölge seçimi, OCR döngüsü, servis, tepsi/kısayol — tamamı yok |
| 11 | Pencere algılama ile yakalama | MISSING | Pencereye tıklayıp yakalama yok |
| 12 | Kayıtlı kısayolların 4/5'i çalışmıyor | MISSING | Tam ekran/yükleme/kolaj/çeviri kısayolları ayarlarda görünür ama kayıtlı değil — ayar ekranı kullanıcıya yanlış bilgi veriyor |
| 13 | Ters görsel arama | PARTIAL | Motorlar yazılmış ama yalnız CLI; uygulamada özellik yok |
| 14 | GIF editöründe oynatma önizlemesi ve kare zaman çizelgesi | MISSING | Yalnız statik ilk kare; kare düzenleme yapılamaz |
| 15 | GIF editöründe karelere açıklama + overlay'ler (filigran/kenarlık/ilerleme) | MISSING | Export öncesi görsel zenginleştirme tamamen yok |
| 16 | Otomatik güncelleme | MISSING | Uygulamada kontrol/indirme/kurulum yok; yalnız CLI |
| 17 | Ayarların çoğu "yalan": düzenlenir, kaydedilir, hiç okunmaz | PARTIAL | `show_cursor`, `gif.*`, `video.*`, `presenter`, `subtitle`, `tool_styles`, 4 kısayol — hepsi ölü ayar |
| 18 | Çeviri: ön plan seçimini oku + çok uçlu istemci + ikinci geçiş | PARTIAL | Hızlı çeviri gerçek bir pencere değil; seçili metni kendiliğinden almıyor; router ölü |
| 19 | Buluta yükleme tetikleyicileri | PARTIAL | Tepsi öğesi, yükleme kısayolu, dosya seçici ve sürükle-bırak yok |
| 20 | Kalıcı öğe düzenlemesiz kırpma ve resim öğesi kırpma | MISSING | Interaktif crop aracı yok (`crop_bgra` ölü) |
| 21 | Kayıt overlay'i / geri sayım / sonuç penceresi | MISSING | Kayıt sırasında süre/fps/bellek göstergesi ve sonrasında oynatıcı yok |
| 22 | Sunum: spot ışığı, renk/araç kısayolları, eğmeli ok, mürekkep güzelleştirme, undo | PARTIAL/MISSING | Spot toggle çizilmiyor; ok eğme matematiği ölü; el yazısı→metin yok |
| 23 | Tepsi menüsünün 8 öğesi eksik | MISSING | Serbest/Yükleme/Arka plan/Sunum/Altyazı/Hakkında/güncelleme öğeleri yok |
| 24 | Dosya adlandırma | PARTIAL | Zaman damgası yerine sayaç; dosya adları çakışma/ayırt etme zayıf |
| 25 | Tek örnek koruması | MISSING | İkinci kopya açılınca ikinci tepsi+hotkey çakışması |
| 26 | Zoom/pan canvas | PARTIAL | `view_transform::compute` ölü; panel yalnız basit "sığdır" yapıyor |

---

## 3. Rust tarafında bulunan placeholder / ölü kod

Kesin satırlar:

**Stub işleyiciler (tepsi)**
- `rust/crates/sf-app/src/tray.rs:62` — `Box::new(|| eprintln!("renk seçici: yakında")),`
- `rust/crates/sf-app/src/tray.rs:66` — `Box::new(|| eprintln!("hızlı çeviri: pencereyi aç (--window)")),`

**Çizilmiş ama bağlanmamış UI (yalnız test çağrısı)**
- `rust/crates/sf-capture/src/overlay_chrome.rs:887` — `pub fn draw_size_badge(` (tek referans `sf-capture/tests/chrome_render.rs:166`) — W×H rozeti hiç çizilmiyor
- `rust/crates/sf-capture/src/overlay_chrome.rs:935` — `pub fn draw_toast(` (tek referans `sf-capture/tests/chrome_render.rs:179`) — toast hiç çizilmiyor

**Sessizce yutulan kullanıcı girdisi (no-op)**
- `rust/crates/sf-capture/src/editor_window.rs:373` — `Hit::Mode(ModeChoice::Region) => {}`
- `rust/crates/sf-capture/src/editor_window.rs:379` — `Hit::Fill(_) => {}`
- `rust/crates/sf-capture/src/editor_window.rs:453` — `Tool::Select => {}`
- `rust/crates/sf-app/src/panels/annotate.rs:424` — `Tool::Select => Shape::Pen { points: vec![from, to], color: app.color },`

**Sabit/yer tutucu değerler**
- `rust/crates/sf-capture/src/editor_window.rs:428` — `text: "TEXT".to_string(),` (metin girişi yok)
- `rust/crates/sf-app/src/panels/annotate.rs:411` — `fn build_shape(app: &ScreenForgeApp, from: (f32, f32), to: (f32, f32), _w: f32) -> Shape {` — `_w` kullanılmıyor
- `rust/crates/sf-app/src/tray.rs:134` — `sf_draw::model::render(&mut canvas, &shape.translate(-(x as f32), -(y as f32)));` — yeni `render_with` yerine sabit `LINE_WIDTH`'li `render`; seçilen kalınlık export'ta kaybolur
- `rust/crates/sf-app/src/panels/gif.rs:351` — `gif.maximum_number_color = 256;`
- `rust/crates/sf-app/src/panels/gif.rs:352` — `gif.quantizer_type = sf_gif::QuantizerType::Octree;`
- `rust/crates/sf-app/src/panels/record.rs:223` — `let bitrate = sf_media::bits_per_second(sf_settings::VideoQuality::High, w, h);`
- `rust/crates/sf-app/src/panels/settings.rs:114` — `egui::RichText::new("Kısayollar şimdilik salt okunur.")`

**Hiçbir üretim yolu çağırmıyor**
- `rust/crates/sf-edit/src/history.rs:16` — `pub struct EditHistory {` (yalnız `sf-edit/tests/`), undo/redo motoru ölü
- `rust/crates/sf-edit/src/view_transform.rs:27` — `pub fn compute(` (yalnız `sf-edit/tests/edit.rs`), zoom/pan ölü
- `rust/crates/sf-draw/src/crop.rs:16` — `pub fn crop_bgra(` (yalnız `sf-draw/tests/draw.rs:416`), interaktif crop ölü
- `rust/crates/sf-capture/src/reserved_hotkeys.rs:8` — `pub fn describe(` (test dışı çağrı yok), ayrılmış kısayol uyarısı ölü
- `rust/crates/sf-capture/src/key_labels.rs:8` — `pub fn describe(` (test dışı çağrı yok), tuş etiketi render'ı ölü
- `rust/crates/sf-capture/src/input_hook.rs:74` — `GestureState` (test dışı çağrı yok), girdi hook'u beslenmiyor
- `rust/crates/sf-overlay/src/arrow.rs:12` — `control_point` (yalnız `sf-overlay/tests/overlay.rs:4`), eğmeli ok ölü
- `rust/crates/sf-net/src/lang_router.rs:6` — router (yalnız `sf-net/tests/net.rs`), ikinci geçiş çeviri ölü
- `rust/crates/sf-settings/src/recents.rs:12` — `VisualTranslateRecents` (yalnız `sf-settings/tests/settings.rs`)

**Ayarlar: kalıcı ve düzenlenebilir ama hiç okunmayan alanlar**
- `rust/crates/sf-settings/src/lib.rs:624` — `show_cursor` (yakalama imleç bayrağı almıyor)
- `rust/crates/sf-settings/src/lib.rs:626` — `auto_close_upload_window` (okuyucu yok)
- `rust/crates/sf-settings/src/lib.rs:631-634` — `full_screen_hotkey`, `full_screen_upload_hotkey`, `collage_hotkey`, `quick_translate_hotkey` (kayıt yok)
- `rust/crates/sf-settings/src/lib.rs:642` — `tool_styles` (annotate paneli okumuyor)
- `rust/crates/sf-settings/src/lib.rs:643` — `gif` yapısı (256/Octree sabit)
- `rust/crates/sf-settings/src/lib.rs:644` — `video` yapısı (`VideoQuality::High` sabit)
- `rust/crates/sf-settings/src/lib.rs:645` — `presenter` yapısı (UI ve okuyucu yok)
- `rust/crates/sf-settings/src/lib.rs:646` — `subtitle` yapısı (yalnız CLI kısmen okur)
- `rust/crates/sf-settings/src/lib.rs:648-649` — `translate_source_language`, `translate_target_language` (okuyucu yok)

---

## 4. Denetlenemeyen alanlar

- **Çalıştırılmadı.** Tüm kararlar statik çağrı grafiği + okuma ile verildi; `cargo build`/`cargo test` ya da uygulama çalıştırılmadı. `sf-bg`'nin `onnx` özelliğiyle (`ort` crate) gerçekten çıkarım yaptığı doğrulanmadı; `sf-bg/tests/onnx.rs` yalnız okundu.
- **Çalışma ağacı denetim sırasında değişti.** `editor_window.rs` (581→625 satır), `overlay_chrome.rs` (+394 satır), `sf-draw/src/model.rs` ve `lib.rs` commit edilmemiş değişiklikler içeriyor. Bu belgedeki Rust satırları denetim sonundaki hâle göredir; `draw_option_bar`/`draw_action_bar`'ın kablolandığı bu değişiklikler commit edilmemiş durumda.
- **XAML olay bağlantıları.** `GifEditorWindow.xaml` (632 satır) ve `CaptureOverlayWindow.xaml` x:Name'li kontroller için işleyiciler code-behind'dan eşlendi; tam XAML okunmadı, bu yüzden işleyicisi hiç olmayan bir kontrol listeye düşmemiş olabilir.
- **Kısmi okumalar.** `GifRecorder.cs:384-568` (yakalama döngüsü GDI içleri), `OverlayRenderer.cs:37-315` (çizim içleri), `VideoResultWindow.cs` gövdesi, `RecorderPickerPopup.cs` gövdesi yalnız üye listesi/grep ile görüldü. `ScreenForge/obj/**` (üretilen kod) talimat gereği atlandı.
- **C# tarafında bulunamayanlar.** Otomatik bölge ("scissors") aracı ve overlay büyüteci C# denetlenen dosyalarda bulunamadı — ikisi de "MISSING" değil, "C#'ta yok" olarak işaretlendi. Overlay'de sağ tık iptali ve Enter onayı için kesin C# satırı bulunamadı ("satır belirsiz").
- **Setup/installer.** `setup.iss` (C#) ve `rust/setup-rust.iss` yalnız kısmen incelendi; kurulum kayıt defteri/shell verb paritesi ayrıntılı doğrulanmadı.
- **Runtime davranışı.** `presenter_window.rs` katmanlı pencere (UpdateLayeredWindow alfa) doğruluğu ve `hotkey.rs` kayıt davranışı çalıştırılarak doğrulanmadı.
