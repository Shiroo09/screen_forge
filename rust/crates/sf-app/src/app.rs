//! The application shell and shared state.
//!
//! Panels live in `panels/`; each is `pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui)`
//! and owns only the fields it declares here. All state lives on this struct so
//! panels never have to coordinate.

use crate::panels::collage::CollageItem;
use crate::theme;

/// Top-level navigation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Capture,
    Annotate,
    Record,
    Gif,
    Translate,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Capture,
        Tab::Annotate,
        Tab::Record,
        Tab::Gif,
        Tab::Translate,
        Tab::Settings,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Capture => "Yakala",
            Tab::Annotate => "Düzenle",
            Tab::Record => "Kayıt",
            Tab::Gif => "GIF",
            Tab::Translate => "Çeviri",
            Tab::Settings => "Ayarlar",
        }
    }

    /// The line icon drawn next to the tab title.
    pub fn icon_name(self) -> crate::icons::IconName {
        use crate::icons::IconName;
        match self {
            Tab::Capture => IconName::Camera,
            Tab::Annotate => IconName::Pen,
            Tab::Record => IconName::Record,
            Tab::Gif => IconName::Layers,
            Tab::Translate => IconName::Translate,
            Tab::Settings => IconName::Settings,
        }
    }

}

/// Capture mode, mirroring the old overlay's mode bar (Bölge · Tam ekran · Serbest).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CaptureMode {
    #[default]
    Region,
    FullScreen,
    Freeform,
}

impl CaptureMode {
    pub const ALL: [CaptureMode; 3] =
        [CaptureMode::Region, CaptureMode::FullScreen, CaptureMode::Freeform];

    pub fn title(self) -> &'static str {
        match self {
            CaptureMode::Region => "Bölge",
            CaptureMode::FullScreen => "Tam ekran",
            CaptureMode::Freeform => "Serbest",
        }
    }
}

/// A captured image held in memory, plus its GPU texture for display.
pub struct ImageBuf {
    pub width: i32,
    pub height: i32,
    pub bgra: Vec<u8>,
    pub texture: Option<egui::TextureHandle>,
    /// Bumped whenever `bgra` changes, so the texture is rebuilt lazily.
    pub dirty: bool,
    /// Identifies this capture. Panels that cache derived textures key on it, so
    /// a new capture of the same size never reuses the previous one's preview.
    pub seq: u64,
}

impl ImageBuf {
    pub fn from_captured(c: sf_capture::Captured) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_SEQ: AtomicU64 = AtomicU64::new(0);
        Self {
            width: c.width,
            height: c.height,
            bgra: c.bgra,
            texture: None,
            dirty: true,
            seq: NEXT_SEQ.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// The display texture, rebuilt on first use or after `dirty`.
    pub fn texture(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        if self.texture.is_none() || self.dirty {
            self.texture = Some(self.build_texture(ctx));
            self.dirty = false;
        }
        self.texture.clone().unwrap()
    }

    fn build_texture(&self, ctx: &egui::Context) -> egui::TextureHandle {
        let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
        for px in self.bgra.chunks_exact(4) {
            rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
        let img =
            egui::ColorImage::from_rgba_unmultiplied([self.width as usize, self.height as usize], &rgba);
        ctx.load_texture("capture", img, egui::TextureOptions::LINEAR)
    }
}

/// Recent action record shown on the Capture tab.
#[derive(Clone, Debug)]
pub struct RecentItem {
    pub label: String,
    pub path: String,
}

/// Annotate-tab options beyond the active tool.
#[derive(Clone, Debug)]
pub struct Options {
    pub line_width: f32,
    pub blur_strength: f32,
    pub text: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            line_width: 3.0,
            blur_strength: 8.0,
            text: "Metin".to_string(),
        }
    }
}

pub struct ScreenForgeApp {
    pub settings: sf_settings::AppSettings,
    pub tab: Tab,
    pub toast: Option<(String, f64)>,
    pub status: String,

    // Capture
    pub captured: Option<ImageBuf>,
    pub recents: Vec<RecentItem>,
    /// Capture mode selector (the old overlay's mode bar).
    pub capture_mode: CaptureMode,
    /// Serbest mod ("Serbest") tuvalindeki görsel öğeler.
    pub collage_items: Vec<CollageItem>,
    /// Share link of the last successful cloud upload.
    pub upload_url: Option<String>,
    /// True while an upload worker is running.
    pub uploading: bool,

    // Annotate
    pub annotate: Options,
    pub tool: sf_draw::model::Tool,
    pub color: sf_draw::Rgba,
    pub shapes: Vec<sf_draw::Shape>,
    pub drag_from: Option<(f32, f32)>,
    /// `seq` of the capture `shapes` were drawn on; a different capture clears them.
    pub annotated_seq: u64,

    // Record
    pub recording: bool,
    pub record_gif: bool,
    pub record_seconds: f32,
    pub record_fps: i32,

    // GIF editor
    pub gif_info: Option<(i32, i32, usize)>,
    pub gif_keep: i32,
    pub gif_remove: i32,
    pub gif_reverse: bool,

    // Translate
    pub translate_input: String,
    pub translate_output: String,
}

impl ScreenForgeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        // egui picks up the OS scale factor, but the default is conservative on
        // high-DPI displays. Clamp to at least 1.0 so the UI is never tiny.
        let ppp = cc.egui_ctx.pixels_per_point();
        if ppp < 1.0 {
            cc.egui_ctx.set_pixels_per_point(1.0);
        }
        let (settings, _) = sf_settings::AppSettings::load(&sf_settings::settings_path());
        Self {
            settings,
            tab: Tab::default(),
            toast: None,
            status: "hazır".to_string(),
            captured: None,
            recents: Vec::new(),
            capture_mode: CaptureMode::default(),
            collage_items: Vec::new(),
            upload_url: None,
            uploading: false,
            annotate: Options::default(),
            tool: sf_draw::model::Tool::Rect,
            color: sf_draw::Rgba::rgb(0xEA6F12),
            shapes: Vec::new(),
            drag_from: None,
            annotated_seq: u64::MAX,
            recording: false,
            record_gif: true,
            record_seconds: 5.0,
            record_fps: 12,
            gif_info: None,
            gif_keep: 2,
            gif_remove: 1,
            gif_reverse: false,
            translate_input: String::new(),
            translate_output: String::new(),
        }
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 3.5));
    }

}

impl eframe::App for ScreenForgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.topbar(ui);
        self.sidebar(ui);
        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Capture => crate::panels::capture::ui(self, ui),
            Tab::Annotate => crate::panels::annotate::ui(self, ui),
            Tab::Record => crate::panels::record::ui(self, ui),
            Tab::Gif => crate::panels::gif::ui(self, ui),
            Tab::Translate => crate::panels::translate::ui(self, ui),
            Tab::Settings => crate::panels::settings::ui(self, ui),
        });
        self.toast_ui(ui.ctx());
    }
}

impl ScreenForgeApp {
    fn topbar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("topbar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("ScreenForge")
                        .color(theme::ACCENT)
                        .size(20.0)
                        .strong(),
                );
                ui.label(egui::RichText::new("ekran görüntüsü & kayıt").color(theme::TEXT_DIM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.status).color(theme::TEXT_DIM).size(12.0));
                });
            });
            ui.add_space(4.0);
        });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("sidebar")
            .resizable(false)
            .show(ui, |ui| {
                ui.add_space(6.0);
                for tab in Tab::ALL {
                    let selected = self.tab == tab;
                    let title = tab.title();
                    let icon_name = tab.icon_name();
                    let fg = if selected { egui::Color32::WHITE } else { theme::TEXT };
                    // Reserve the row, then paint an icon + label inside it so the
                    // line icon (not a letter) leads each tab, as in the old app.
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 38.0),
                        egui::Sense::click(),
                    );
                    let fill = if selected {
                        theme::ACCENT
                    } else if response.hovered() {
                        theme::SURFACE_HOVER
                    } else {
                        theme::SURFACE
                    };
                    ui.painter().rect_filled(rect, 6.0, fill);

                    let icon_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 12.0, rect.center().y - 9.0),
                        egui::vec2(18.0, 18.0),
                    );
                    crate::icons::paint(ui.painter(), icon_name, icon_rect, fg, 1.8);
                    ui.painter().text(
                        egui::pos2(icon_rect.right() + 10.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        title,
                        egui::FontId::proportional(15.0),
                        fg,
                    );

                    if response.clicked() {
                        self.tab = tab;
                    }
                    ui.add_space(2.0);
                }
            });
    }

    #[allow(deprecated)]
    fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some((msg, ttl)) = &mut self.toast else {
            return;
        };
        *ttl -= ctx.input(|i| i.stable_dt) as f64;
        if *ttl <= 0.0 {
            self.toast = None;
            return;
        }
        let msg = msg.clone();
        egui::Area::new(egui::Id::new("toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -24.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::SURFACE_HI)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER))
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .corner_radius(8.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&msg).color(theme::TEXT));
                    });
            });
        ctx.request_repaint();
    }
}
