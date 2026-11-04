//! The application shell and shared state.
//!
//! Panels live in `panels/`; each is `pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui)`
//! and owns only the fields it declares here. All state lives on this struct so
//! panels never have to coordinate.

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

    pub fn icon(self) -> &'static str {
        match self {
            Tab::Capture => "K",
            Tab::Annotate => "D",
            Tab::Record => "Y",
            Tab::Gif => "G",
            Tab::Translate => "C",
            Tab::Settings => "A",
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
}

impl ImageBuf {
    pub fn from_captured(c: sf_capture::Captured) -> Self {
        Self {
            width: c.width,
            height: c.height,
            bgra: c.bgra,
            texture: None,
            dirty: true,
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

    // Annotate
    pub annotate: Options,
    pub tool: sf_draw::model::Tool,
    pub color: sf_draw::Rgba,
    pub shapes: Vec<sf_draw::Shape>,
    pub drag_from: Option<(f32, f32)>,

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
        let (settings, _) = sf_settings::AppSettings::load(&sf_settings::settings_path());
        Self {
            settings,
            tab: Tab::default(),
            toast: None,
            status: "hazır".to_string(),
            captured: None,
            recents: Vec::new(),
            annotate: Options::default(),
            tool: sf_draw::model::Tool::Rect,
            color: sf_draw::Rgba::rgb(0xEA6F12),
            shapes: Vec::new(),
            drag_from: None,
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
                    let text = format!("{}  {}", tab.icon(), tab.title());
                    let btn = egui::Button::new(
                        egui::RichText::new(text)
                            .size(15.0)
                            .color(if selected { egui::Color32::WHITE } else { theme::TEXT }),
                    )
                    .fill(if selected { theme::ACCENT } else { theme::SURFACE })
                    .min_size(egui::vec2(150.0, 38.0));
                    if ui.add(btn).clicked() {
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
