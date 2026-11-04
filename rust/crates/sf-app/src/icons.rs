//! 66 Lucide-tarzı çizgi ikonu (C# `ScreenForge/Resources/Icons.xaml` portu).
//!
//! Ikonlar 24×24 viewBox SVG path verisidir; `paint` bunları verilen `rect` içine
//! ölçekleyip stroke olarak çizer (fill yok).
#![allow(dead_code)]

use egui::{pos2, Color32, Painter, Pos2, Rect, Shape, Stroke};

/// Icons.xaml'deki 66 `<Geometry x:Key="IconXxx">` girdisi.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconName {
    Cursor,
    Crop,
    Square,
    Circle,
    Arrow,
    Line,
    Pen,
    Highlighter,
    Text,
    Step,
    Blur,
    Record,
    Copy,
    CopyFront,
    CopyBack,
    Save,
    Cloud,
    Close,
    Check,
    Trash,
    Translate,
    Search,
    Ocr,
    Lens,
    Yandex,
    Eyedropper,
    Region,
    Fullscreen,
    Layers,
    Ribbon,
    Shadow,
    Stroke,
    Bold,
    Italic,
    Palette,
    StepCircle,
    StepSquare,
    StepBubble,
    Settings,
    Info,
    Power,
    Camera,
    Undo,
    Redo,
    Duplicate,
    DeleteBefore,
    DeleteAfter,
    Reverse,
    MoveLeft,
    MoveRight,
    RotateLeft,
    RotateRight,
    Play,
    Pause,
    PrevFrame,
    NextFrame,
    FirstFrame,
    LastFrame,
    Stopwatch,
    Keyframe,
    PrevKey,
    NextKey,
    MarkIn,
    MarkOut,
    Reset,
    Fit,
}

impl IconName {
    /// Tüm ikonlar (galeri / test taraması için).
    pub const ALL: [IconName; 66] = [
        IconName::Cursor,
        IconName::Crop,
        IconName::Square,
        IconName::Circle,
        IconName::Arrow,
        IconName::Line,
        IconName::Pen,
        IconName::Highlighter,
        IconName::Text,
        IconName::Step,
        IconName::Blur,
        IconName::Record,
        IconName::Copy,
        IconName::CopyFront,
        IconName::CopyBack,
        IconName::Save,
        IconName::Cloud,
        IconName::Close,
        IconName::Check,
        IconName::Trash,
        IconName::Translate,
        IconName::Search,
        IconName::Ocr,
        IconName::Lens,
        IconName::Yandex,
        IconName::Eyedropper,
        IconName::Region,
        IconName::Fullscreen,
        IconName::Layers,
        IconName::Ribbon,
        IconName::Shadow,
        IconName::Stroke,
        IconName::Bold,
        IconName::Italic,
        IconName::Palette,
        IconName::StepCircle,
        IconName::StepSquare,
        IconName::StepBubble,
        IconName::Settings,
        IconName::Info,
        IconName::Power,
        IconName::Camera,
        IconName::Undo,
        IconName::Redo,
        IconName::Duplicate,
        IconName::DeleteBefore,
        IconName::DeleteAfter,
        IconName::Reverse,
        IconName::MoveLeft,
        IconName::MoveRight,
        IconName::RotateLeft,
        IconName::RotateRight,
        IconName::Play,
        IconName::Pause,
        IconName::PrevFrame,
        IconName::NextFrame,
        IconName::FirstFrame,
        IconName::LastFrame,
        IconName::Stopwatch,
        IconName::Keyframe,
        IconName::PrevKey,
        IconName::NextKey,
        IconName::MarkIn,
        IconName::MarkOut,
        IconName::Reset,
        IconName::Fit,
    ];
}

/// İkonun insan okur etiketi (Icons.xaml key adıyla aynı).
pub fn icon(name: IconName) -> &'static str {
    match name {
        IconName::Cursor => "Cursor",
        IconName::Crop => "Crop",
        IconName::Square => "Square",
        IconName::Circle => "Circle",
        IconName::Arrow => "Arrow",
        IconName::Line => "Line",
        IconName::Pen => "Pen",
        IconName::Highlighter => "Highlighter",
        IconName::Text => "Text",
        IconName::Step => "Step",
        IconName::Blur => "Blur",
        IconName::Record => "Record",
        IconName::Copy => "Copy",
        IconName::CopyFront => "CopyFront",
        IconName::CopyBack => "CopyBack",
        IconName::Save => "Save",
        IconName::Cloud => "Cloud",
        IconName::Close => "Close",
        IconName::Check => "Check",
        IconName::Trash => "Trash",
        IconName::Translate => "Translate",
        IconName::Search => "Search",
        IconName::Ocr => "Ocr",
        IconName::Lens => "Lens",
        IconName::Yandex => "Yandex",
        IconName::Eyedropper => "Eyedropper",
        IconName::Region => "Region",
        IconName::Fullscreen => "Fullscreen",
        IconName::Layers => "Layers",
        IconName::Ribbon => "Ribbon",
        IconName::Shadow => "Shadow",
        IconName::Stroke => "Stroke",
        IconName::Bold => "Bold",
        IconName::Italic => "Italic",
        IconName::Palette => "Palette",
        IconName::StepCircle => "StepCircle",
        IconName::StepSquare => "StepSquare",
        IconName::StepBubble => "StepBubble",
        IconName::Settings => "Settings",
        IconName::Info => "Info",
        IconName::Power => "Power",
        IconName::Camera => "Camera",
        IconName::Undo => "Undo",
        IconName::Redo => "Redo",
        IconName::Duplicate => "Duplicate",
        IconName::DeleteBefore => "DeleteBefore",
        IconName::DeleteAfter => "DeleteAfter",
        IconName::Reverse => "Reverse",
        IconName::MoveLeft => "MoveLeft",
        IconName::MoveRight => "MoveRight",
        IconName::RotateLeft => "RotateLeft",
        IconName::RotateRight => "RotateRight",
        IconName::Play => "Play",
        IconName::Pause => "Pause",
        IconName::PrevFrame => "PrevFrame",
        IconName::NextFrame => "NextFrame",
        IconName::FirstFrame => "FirstFrame",
        IconName::LastFrame => "LastFrame",
        IconName::Stopwatch => "Stopwatch",
        IconName::Keyframe => "Keyframe",
        IconName::PrevKey => "PrevKey",
        IconName::NextKey => "NextKey",
        IconName::MarkIn => "MarkIn",
        IconName::MarkOut => "MarkOut",
        IconName::Reset => "Reset",
        IconName::Fit => "Fit",
    }
}

/// Bir ikonu verilen rect içine, verilen renk ve kalınlıkla çizer.
///
/// `stroke_width` 24×24 ölçeğinde verilir; ikon rect'i küçüldükçe çizgi de
/// aynı oranda incelir (SVG `stroke-width` davranışı).
pub fn paint(painter: &Painter, name: IconName, rect: Rect, color: Color32, stroke_width: f32) {
    let scale = rect.width().min(rect.height()) / 24.0;
    let ox = rect.center().x - 12.0 * scale;
    let oy = rect.center().y - 12.0 * scale;
    let stroke = Stroke::new(stroke_width * scale, color);
    for (pts, closed) in parse_path(path_data(name)) {
        if pts.len() < 2 {
            continue;
        }
        let pts: Vec<Pos2> = pts
            .iter()
            .map(|&(x, y)| pos2(ox + x * scale, oy + y * scale))
            .collect();
        let shape = if closed {
            Shape::closed_line(pts, stroke)
        } else {
            Shape::line(pts, stroke)
        };
        painter.add(shape);
    }
}

/// Icons.xaml'deki ham SVG path verisi (24×24 viewBox, stroke).
fn path_data(name: IconName) -> &'static str {
    match name {
        IconName::Cursor => "M4 3 L19 11 L11.5 12.5 L9 20 Z",
        IconName::Crop => "M7 3 V17 H21 M3 7 H17 V21",
        IconName::Square => "M4 4 H20 V20 H4 Z",
        IconName::Circle => "M12 3 A9 9 0 1 0 12.01 3 Z",
        IconName::Arrow => "M5 19 L19 5 M11 5 H19 V13",
        IconName::Line => "M5 19 L19 5",
        IconName::Pen => "M3 21 L4 16.5 L16.5 4 A2.1 2.1 0 0 1 20 7 L7.5 19.5 Z M14 6.5 L17.5 10",
        IconName::Highlighter => "M9 11 L3 17 V21 H8 L12 17 M21 7 L17 3 L11 9 L15 13 Z",
        IconName::Text => "M5 7 V4 H19 V7 M9 20 H15 M12 4 V20",
        IconName::Step => "M12 3 A9 9 0 1 0 12.01 3 Z M10 9 L12 8 V16",
        IconName::Blur => "M12 3 C12 3 5 11 5 15 A7 7 0 0 0 19 15 C19 11 12 3 12 3 Z",
        IconName::Record => {
            "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2z"
        }
        IconName::Copy => "M9 9 H20 V20 H9 Z M5 15 H4 V4 H15 V5",
        IconName::CopyFront => {
            "M6 11C6 8.17 6 6.76 6.88 5.88C7.76 5 9.17 5 12 5H15C17.83 5 19.24 5 20.12 5.88C21 6.76 21 8.17 21 11V16C21 18.83 21 20.24 20.12 21.12C19.24 22 17.83 22 15 22H12C9.17 22 7.76 22 6.88 21.12C6 20.24 6 18.83 6 16Z"
        }
        IconName::CopyBack => {
            "M6 19C4.34 19 3 17.66 3 16V10C3 6.23 3 4.34 4.17 3.17C5.34 2 7.23 2 11 2H15C16.66 2 18 3.34 18 5"
        }
        IconName::Save => {
            "M21 15 V19 A2 2 0 0 1 19 21 H5 A2 2 0 0 1 3 19 V15 M7 10 L12 15 L17 10 M12 15 V3"
        }
        IconName::Cloud => {
            "M12 13 V21 M8 17 L12 13 L16 17 M20 16.6 A5 5 0 0 0 18 7 H16.7 A8 8 0 1 0 4 15.2"
        }
        IconName::Close => "M18 6 L6 18 M6 6 L18 18",
        IconName::Check => "M20 6 L9 17 L4 12",
        IconName::Trash => {
            "M3 6 H21 M8 6 V4 H16 V6 M5 6 V20 A1 1 0 0 0 6 21 H18 A1 1 0 0 0 19 20 V6 M10 10 V17 M14 10 V17"
        }
        IconName::Translate => {
            "M5 8 L10 13 M4 14 L10 8 L12 5 M2 5 H14 M7 2 H8 M12.913 17 H20.087 M12.913 17 L11 21 M12.913 17 L15.7783 11.009 C16.0092,10.5263 16.1246,10.2849 16.2826,10.2086 C16.4199,10.1423 16.5801,10.1423 16.7174,10.2086 C16.8754,10.2849 16.9908,10.5263 17.2217,11.009 L20.087 17 M20.087 17 L22 21"
        }
        IconName::Search => "M11 5 A6 6 0 1 0 11.01 5 Z M16 16 L21 21",
        IconName::Ocr => {
            "M3 7 V5 A2 2 0 0 1 5 3 H7 M17 3 H19 A2 2 0 0 1 21 5 V7 M21 17 V19 A2 2 0 0 1 19 21 H17 M7 21 H5 A2 2 0 0 1 3 19 V17 M7 8 H15 M7 12 H17 M7 16 H13"
        }
        IconName::Lens => "M12 3 A9 9 0 1 0 12.01 3 Z M9 12 H15 M12 9 V15",
        IconName::Yandex => "M7 4 L12 14 L17 4 M12 14 V20",
        IconName::Eyedropper => {
            "M6.34 17.73 A6.9 6.9 0 0 1 8.34 12.84 L14 7.23 L16.86 10.09 L11.23 15.7 A6.92 6.92 0 0 1 6.34 17.73 Z M17.32 10.57 L13.5 6.75 L18 2.29 A2.69 2.69 0 0 1 19.91 1.5 A2.7 2.7 0 0 1 22.61 4.2 A2.73 2.73 0 0 1 21.82 6.11 Z M12.07 5.32 L18.75 12 M6.34 21.55 A1 1 0 0 1 4.43 21.55 A6.27 6.27 0 0 1 5.43 19.64 A6.29 6.29 0 0 1 6.34 21.55 Z"
        }
        IconName::Region => {
            "M3 3 H7 M3 3 V7 M17 3 H21 M21 3 V7 M3 17 V21 M3 21 H7 M21 17 V21 M17 21 H21 M8 6 H16 M8 18 H16 M6 8 V16 M18 8 V16"
        }
        IconName::Fullscreen => {
            "M2 4 A2 2 0 0 1 4 2 H20 A2 2 0 0 1 22 4 V16 A2 2 0 0 1 20 18 H4 A2 2 0 0 1 2 16 Z M9 18 L8 22 H16 L15 18"
        }
        IconName::Layers => "M4 4 H14 V14 H4 Z M10 10 H20 V20 H10 Z M17 6 V4 M17 8 V6",
        IconName::Ribbon => "M3 3 H21 V21 H3 Z",
        IconName::Shadow => "M3 3 H21 V21 H3 Z M12 3 V21",
        IconName::Stroke => "M4 4 H20 V20 H4 Z",
        IconName::Bold => "M6 4 H14 A4 4 0 0 1 14 12 H6 Z M6 12 H15 A4 4 0 0 1 15 20 H6 Z",
        IconName::Italic => "M10 4 H20 M4 20 H14 M15 4 L9 20",
        IconName::Palette => {
            "M12 2 C12 2 4 9 4 14 A8 8 0 0 0 20 14 C20 9 12 2 12 2 Z M8 10 A1 1 0 1 1 8.01 10 Z M12 7 A1 1 0 1 1 12.01 7 Z M16 10 A1 1 0 1 1 16.01 10 Z M9 14 A1 1 0 1 1 9.01 14 Z M15 14 A1 1 0 1 1 15.01 14 Z"
        }
        IconName::StepCircle => "M12 3 A9 9 0 1 0 12.01 3 Z",
        IconName::StepSquare => {
            "M5 4 H19 A2 2 0 0 1 21 6 V18 A2 2 0 0 1 19 20 H5 A2 2 0 0 1 3 18 V6 A2 2 0 0 1 5 4 Z"
        }
        IconName::StepBubble => {
            "M21 15 A2 2 0 0 1 19 17 H7 L3 21 V5 A2 2 0 0 1 5 3 H19 A2 2 0 0 1 21 5 Z"
        }
        IconName::Settings => {
            "M12 8 A4 4 0 1 0 12.01 8 Z M19.4 15 A1.65 1.65 0 0 0 19.7 16.8 L19.8 16.9 A2 2 0 1 1 17 19.7 L16.9 19.6 A1.65 1.65 0 0 0 15 19.3 A1.65 1.65 0 0 0 14 20.8 V21 A2 2 0 1 1 10 21 V20.9 A1.65 1.65 0 0 0 9 19.4 A1.65 1.65 0 0 0 7.1 19.7 L7 19.8 A2 2 0 1 1 4.2 17 L4.3 16.9 A1.65 1.65 0 0 0 4.6 15 A1.65 1.65 0 0 0 3.1 14 H3 A2 2 0 1 1 3 10 H3.1 A1.65 1.65 0 0 0 4.6 9 A1.65 1.65 0 0 0 4.3 7.1 L4.2 7 A2 2 0 1 1 7 4.2 L7.1 4.3 A1.65 1.65 0 0 0 9 4.6 A1.65 1.65 0 0 0 10 3.1 V3 A2 2 0 1 1 14 3 V3.1 A1.65 1.65 0 0 0 15 4.6 A1.65 1.65 0 0 0 16.9 4.3 L17 4.2 A2 2 0 1 1 19.8 7 L19.7 7.1 A1.65 1.65 0 0 0 19.4 9 A1.65 1.65 0 0 0 20.9 10 H21 A2 2 0 1 1 21 14 H20.9 A1.65 1.65 0 0 0 19.4 15 Z"
        }
        IconName::Info => "M12 3 A9 9 0 1 0 12.01 3 Z M12 16 V12 M12 8 H12.01",
        IconName::Power => "M12 4 V12 M7.5 6.5 A8 8 0 1 0 16.5 6.5",
        IconName::Camera => {
            "M14.5 4 H9.5 L8 6 H5 A2 2 0 0 0 3 8 V18 A2 2 0 0 0 5 20 H19 A2 2 0 0 0 21 18 V8 A2 2 0 0 0 19 6 H16 Z M12 16 A3.5 3.5 0 1 0 12.01 16 Z"
        }
        IconName::Undo => "M3 8 H14 A5 5 0 0 1 14 18 H8 M3 8 L7 4 M3 8 L7 12",
        IconName::Redo => "M21 8 H10 A5 5 0 0 0 10 18 H16 M21 8 L17 4 M21 8 L17 12",
        IconName::Duplicate => {
            "M9 3 H19 A2 2 0 0 1 21 5 V15 M15 9 H5 A2 2 0 0 0 3 11 V19 A2 2 0 0 0 5 21 H15 A2 2 0 0 0 17 19 V11 A2 2 0 0 0 15 9 Z"
        }
        IconName::DeleteBefore => "M10 4 V20 M10 12 H20 M20 12 L16 8 M20 12 L16 16 M4 4 V20",
        IconName::DeleteAfter => "M14 4 V20 M14 12 H4 M4 12 L8 8 M4 12 L8 16 M20 4 V20",
        IconName::Reverse => {
            "M4 8 H18 M18 8 L14 4 M18 8 L14 12 M20 16 H6 M6 16 L10 12 M6 16 L10 20"
        }
        IconName::MoveLeft => "M14 5 L7 12 L14 19 M18 5 V19",
        IconName::MoveRight => "M10 5 L17 12 L10 19 M6 5 V19",
        IconName::RotateLeft => "M4 10 A8 8 0 1 1 6 16 M4 5 V10 H9",
        IconName::RotateRight => "M20 10 A8 8 0 1 0 18 16 M20 5 V10 H15",
        IconName::Play => "M7 4 L19 12 L7 20 Z",
        IconName::Pause => "M8 4 V20 M16 4 V20",
        IconName::PrevFrame => "M16 4 L8 12 L16 20 Z",
        IconName::NextFrame => "M8 4 L16 12 L8 20 Z",
        IconName::FirstFrame => "M18 4 L9 12 L18 20 Z M6 4 V20",
        IconName::LastFrame => "M6 4 L15 12 L6 20 Z M18 4 V20",
        IconName::Stopwatch => "M12 7 A7 7 0 1 0 12.01 7 Z M12 10 V14 M9 3 H15 M12 3 V6",
        IconName::Keyframe => "M12 5 L19 12 L12 19 L5 12 Z",
        IconName::PrevKey => "M13 5 L6 12 L13 19 Z M18 5 V19",
        IconName::NextKey => "M11 5 L18 12 L11 19 Z M6 5 V19",
        IconName::MarkIn => "M8 4 H5 V20 H8 M14 8 L18 12 L14 16",
        IconName::MarkOut => "M16 4 H19 V20 H16 M10 8 L6 12 L10 16",
        IconName::Reset => "M4 10 A8 8 0 1 1 5 15 M4 5 V10 H9",
        IconName::Fit => "M4 9 V4 H9 M15 4 H20 V9 M20 15 V20 H15 M9 20 H4 V15",
    }
}

// ---------------------------------------------------------------- parser ----

/// SVG path verisini nokta listelerine çevirir: `(noktalar, kapalı_mı)`.
///
/// Desteklenen komutlar: M/m, L/l, H/h, V/v, C/c, S/s, Q/q, T/t, A/a, Z/z.
/// Eğriler ve yaylar düz çizgi parçalarına bölünür (ikon ölçeğinde yeterli).
pub fn parse_path(d: &str) -> Vec<(Vec<(f32, f32)>, bool)> {
    let mut p = Scanner::new(d);
    let mut out: Vec<(Vec<(f32, f32)>, bool)> = Vec::new();
    let mut cur: Vec<(f32, f32)> = Vec::new();
    let (mut x, mut y) = (0.0f32, 0.0f32); // kalem konumu
    let (mut sx, mut sy) = (0.0f32, 0.0f32); // alt yol başlangıcı
    let (mut cx, mut cy) = (0.0f32, 0.0f32); // son kontrol noktası (S/T yansıması)
    let mut cmd: u8 = 0;
    let mut prev: u8 = 0; // son çalıştırılan komut (büyük harf)

    macro_rules! flush {
        ($closed:expr) => {
            if cur.len() >= 2 {
                out.push((std::mem::take(&mut cur), $closed));
            } else {
                cur.clear();
            }
        };
    }

    loop {
        p.ws();
        if p.eof() {
            break;
        }
        let b = p.byte();
        if b.is_ascii_alphabetic() {
            cmd = b;
            p.bump();
        } else if cmd == 0 {
            break; // baştaki sayı: geçersiz
        }
        let rel = cmd.is_ascii_lowercase();
        let (nx, ny, ctrl) = match cmd.to_ascii_uppercase() {
            b'M' => {
                let (Some(mut a), Some(mut b2)) = (p.num(), p.num()) else {
                    break;
                };
                flush!(false);
                if rel {
                    a += x;
                    b2 += y;
                }
                x = a;
                y = b2;
                sx = x;
                sy = y;
                cur.push((x, y));
                cx = x;
                cy = y;
                prev = b'M';
                cmd = if rel { b'l' } else { b'L' }; // ardışık sayılar L olur
                continue;
            }
            b'L' => {
                let (Some(mut a), Some(mut b2)) = (p.num(), p.num()) else {
                    break;
                };
                if rel {
                    a += x;
                    b2 += y;
                }
                (a, b2, None)
            }
            b'H' => {
                let Some(mut a) = p.num() else { break };
                if rel {
                    a += x;
                }
                (a, y, None)
            }
            b'V' => {
                let Some(mut a) = p.num() else { break };
                if rel {
                    a += y;
                }
                (x, a, None)
            }
            b'C' => {
                let n = [p.num(), p.num(), p.num(), p.num(), p.num(), p.num()];
                let [Some(x1), Some(y1), Some(x2), Some(y2), Some(a), Some(b2)] = n else {
                    break;
                };
                let (x1, y1, x2, y2, a, b2) = if rel {
                    (x + x1, y + y1, x + x2, y + y2, x + a, y + b2)
                } else {
                    (x1, y1, x2, y2, a, b2)
                };
                flatten_cubic(x, y, x1, y1, x2, y2, a, b2, &mut cur);
                (a, b2, Some((x2, y2)))
            }
            b'S' => {
                let n = [p.num(), p.num(), p.num(), p.num()];
                let [Some(x2), Some(y2), Some(a), Some(b2)] = n else {
                    break;
                };
                let (x2, y2, a, b2) = if rel {
                    (x + x2, y + y2, x + a, y + b2)
                } else {
                    (x2, y2, a, b2)
                };
                let (x1, y1) = if matches!(prev, b'C' | b'S') {
                    (2.0 * x - cx, 2.0 * y - cy)
                } else {
                    (x, y)
                };
                flatten_cubic(x, y, x1, y1, x2, y2, a, b2, &mut cur);
                (a, b2, Some((x2, y2)))
            }
            b'Q' => {
                let n = [p.num(), p.num(), p.num(), p.num()];
                let [Some(x1), Some(y1), Some(a), Some(b2)] = n else {
                    break;
                };
                let (x1, y1, a, b2) = if rel {
                    (x + x1, y + y1, x + a, y + b2)
                } else {
                    (x1, y1, a, b2)
                };
                flatten_quad(x, y, x1, y1, a, b2, &mut cur);
                (a, b2, Some((x1, y1)))
            }
            b'T' => {
                let (Some(mut a), Some(mut b2)) = (p.num(), p.num()) else {
                    break;
                };
                if rel {
                    a += x;
                    b2 += y;
                }
                let (x1, y1) = if matches!(prev, b'Q' | b'T') {
                    (2.0 * x - cx, 2.0 * y - cy)
                } else {
                    (x, y)
                };
                flatten_quad(x, y, x1, y1, a, b2, &mut cur);
                (a, b2, Some((x1, y1)))
            }
            b'A' => {
                let n = [
                    p.num(),
                    p.num(),
                    p.num(),
                    p.num(),
                    p.num(),
                    p.num(),
                    p.num(),
                ];
                let [Some(rx), Some(ry), Some(rot), Some(laf), Some(sf), Some(a), Some(b2)] = n
                else {
                    break;
                };
                let (a, b2) = if rel { (x + a, y + b2) } else { (a, b2) };
                arc_to_points(x, y, rx, ry, rot, laf != 0.0, sf != 0.0, a, b2, &mut cur);
                (a, b2, None)
            }
            b'Z' => {
                let zc = prev;
                flush!(true);
                x = sx;
                y = sy;
                cx = x;
                cy = y;
                prev = zc; // Z akışı bırakmaz: ardışık sayılar son komutu sürdürür
                continue;
            }
            _ => break, // bilinmeyen komut
        };

        x = nx;
        y = ny;
        match ctrl {
            Some((ccx, ccy)) => {
                cx = ccx;
                cy = ccy;
            }
            None => {
                cx = x;
                cy = y;
            }
        }
        cur.push((x, y));
        prev = cmd.to_ascii_uppercase();
    }
    flush!(false);
    out
}

/// Son nokta yinelenmesin diye t=0 atlanır.
#[allow(clippy::too_many_arguments)]
fn flatten_cubic(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32, out: &mut Vec<(f32, f32)>) {
    const N: usize = 12;
    for i in 1..=N {
        let t = i as f32 / N as f32;
        let m = 1.0 - t;
        let (a, b, c, d) = (m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
        out.push((
            a * x0 + b * x1 + c * x2 + d * x3,
            a * y0 + b * y1 + c * y2 + d * y3,
        ));
    }
}

fn flatten_quad(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, out: &mut Vec<(f32, f32)>) {
    const N: usize = 10;
    for i in 1..=N {
        let t = i as f32 / N as f32;
        let m = 1.0 - t;
        out.push((
            m * m * x0 + 2.0 * m * t * x1 + t * t * x2,
            m * m * y0 + 2.0 * m * t * y1 + t * t * y2,
        ));
    }
}

/// SVG uç-nokta parametreleştirmesini merkez parametreleştirmesine çevirip
/// yayı nokta dizisine böler (SVG 1.1 spec, bölüm F.6).
#[allow(clippy::too_many_arguments)]
fn arc_to_points(
    x1: f32,
    y1: f32,
    rx: f32,
    ry: f32,
    rot_deg: f32,
    large: bool,
    sweep: bool,
    x2: f32,
    y2: f32,
    out: &mut Vec<(f32, f32)>,
) {
    if (x1 - x2).abs() < 1e-6 && (y1 - y2).abs() < 1e-6 {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-6 || ry < 1e-6 {
        out.push((x2, y2));
        return;
    }
    let phi = rot_deg.to_radians();
    let (sp, cp) = phi.sin_cos();
    let dx = (x1 - x2) / 2.0;
    let dy = (y1 - y2) / 2.0;
    let x1p = cp * dx + sp * dy;
    let y1p = -sp * dx + cp * dy;

    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }

    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let co = if den > 0.0 {
        let s = (num / den).max(0.0).sqrt();
        if large != sweep {
            s
        } else {
            -s
        }
    } else {
        0.0
    };
    let cxp = co * rx * y1p / ry;
    let cyp = -co * ry * x1p / rx;
    let cx = cp * cxp - sp * cyp + (x1 + x2) / 2.0;
    let cy = sp * cxp + cp * cyp + (y1 + y2) / 2.0;

    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;
    let theta1 = uy.atan2(ux);
    let mut dtheta = (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    if !sweep && dtheta > 0.0 {
        dtheta -= std::f32::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f32::consts::TAU;
    }

    let steps = ((dtheta.abs() / (std::f32::consts::PI / 8.0)).ceil() as usize).max(2);
    for i in 1..=steps {
        let t = theta1 + dtheta * (i as f32 / steps as f32);
        let (st, ct) = t.sin_cos();
        out.push((
            cx + rx * ct * cp - ry * st * sp,
            cy + rx * ct * sp + ry * st * cp,
        ));
    }
}

/// SVG sayı/kontrol tarayıcısı.
struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Scanner<'a> {
    fn new(d: &'a str) -> Self {
        Self { s: d.as_bytes(), i: 0 }
    }
    fn ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i].is_ascii_whitespace() || self.s[self.i] == b',')
        {
            self.i += 1;
        }
    }
    fn eof(&self) -> bool {
        self.i >= self.s.len()
    }
    fn byte(&self) -> u8 {
        self.s[self.i]
    }
    fn bump(&mut self) {
        self.i += 1;
    }
    fn num(&mut self) -> Option<f32> {
        self.ws();
        let start = self.i;
        if self.i < self.s.len() && (self.s[self.i] == b'-' || self.s[self.i] == b'+') {
            self.i += 1;
        }
        let mut digits = false;
        while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
            self.i += 1;
            digits = true;
        }
        if self.i < self.s.len() && self.s[self.i] == b'.' {
            self.i += 1;
            while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                self.i += 1;
                digits = true;
            }
        }
        if !digits {
            self.i = start;
            return None;
        }
        // üs kısmı (nadir; yoksa geri sar)
        if self.i < self.s.len() && (self.s[self.i] == b'e' || self.s[self.i] == b'E') {
            let save = self.i;
            self.i += 1;
            if self.i < self.s.len() && (self.s[self.i] == b'-' || self.s[self.i] == b'+') {
                self.i += 1;
            }
            let mut exp_digits = false;
            while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                self.i += 1;
                exp_digits = true;
            }
            if !exp_digits {
                self.i = save;
            }
        }
        std::str::from_utf8(&self.s[start..self.i])
            .ok()
            .and_then(|t| t.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basit_path_iki_nokta_uretir() {
        let subs = parse_path("M5 19 L19 5");
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].0, vec![(5.0, 19.0), (19.0, 5.0)]);
        assert!(!subs[0].1);
    }

    #[test]
    fn kapali_yol_ve_alt_yollar() {
        let subs = parse_path("M4 3 L19 11 L11.5 12.5 L9 20 Z");
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].0.len(), 4);
        assert!(subs[0].1, "Z ile kapanan alt yol kapalı işaretlenmeli");
    }

    #[test]
    fn arc_ve_bezier_nokta_uretir() {
        let subs = parse_path("M12 3 A9 9 0 1 0 12.01 3 Z");
        assert!(subs[0].0.len() > 8, "yay noktalara bölünmeli");
        let subs = parse_path("M12 2C6.48 2 2 6.48 2 12z");
        assert!(subs[0].0.len() > 4, "C eğrisi noktalara bölünmeli");
    }

    #[test]
    fn tum_ikonlar_gecerli_nokta_uretir() {
        for name in IconName::ALL {
            let data = path_data(name);
            let subs = parse_path(data);
            let pts: usize = subs.iter().map(|(p, _)| p.len()).sum();
            assert!(pts >= 2, "{} boş/eksik path: {data}", icon(name));
            for (poly, _) in &subs {
                for (x, y) in poly {
                    assert!(x.is_finite() && y.is_finite(), "{} NaN üretti", icon(name));
                    assert!(
                        (-8.0..=32.0).contains(x) && (-8.0..=32.0).contains(y),
                        "{} 24×24 viewBox dışına taştı: {x},{y}",
                        icon(name)
                    );
                }
            }
        }
    }
}
