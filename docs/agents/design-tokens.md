# ScreenForge tasarım dili (eski C# → Rust eşleşmesi)

Kaynak: `ScreenForge/Resources/Theme.xaml` + `ScreenForge/Windows/*.xaml`.
Amaç: Rust/egui portu eski haliyle **aynı görsel dil** — aynı renkler, aynı
köşe yarıçapları, aynı boşluklar, aynı gölge hissi.

## Renkler (Theme.xaml birebir)

| Rol | Hex | theme.rs |
|---|---|---|
| Accent | `#EA6F12` | `ACCENT` |
| Accent hover | `#F28A3A` | `ACCENT_HOVER` |
| Accent pressed | `#D0620F` | `ACCENT_PRESSED` |
| Surface | `#1F2430` | `SURFACE` |
| Surface alt | `#272D3B` | `SURFACE_HI` |
| Surface hover | `#323A4C` | `SURFACE_HOVER` |
| Border | `#3A4254` | `BORDER` |
| Text | `#F2F4F8` | `TEXT` |
| Text muted | `#9AA4B8` | `TEXT_DIM` |
| Danger | `#E5484D` | `DANGER` |
| Editor canvas | `#12151E` | `BG` |
| Editor well | `#0E1118` | `WELL` |
| Editor chrome | `#171B26` | `CHROME` |
| Editor panel | `#1C2130` | `PANEL` |

## Geometri

- Mod çubuğu / araç kutusu: **6px** köşe yarıçapı, `#F21F2430` (yarı saydam), 1px `#3A4254` kenar, 3px iç boşluk.
- Kart: **8-10px** köşe yarıçapı, `SURFACE` dolgu, 1px `BORDER` kenar, 12-14px iç boşluk.
- Buton: **8px** köşe, 14×8 padding.
- Boyut rozeti: `#CC1F2430`, 4px köşe.
- Gölge: 12px blur, 2px offset, %45 opak — egui'de `Frame::shadow` ile yaklaşık.

## Yerleşim

- Capture overlay: ortada hap biçimli mod çubuğu (Bölge · Tam ekran · Serbest), seçili olan accent dolgulu.
- Ana pencere: üst çubuk (logo + başlık), sol kenar çubuğu (sekmeler), orta panel.
- Tipografi: başlık 20-22px bold, gövde 13-15px, etiket 11-12px.
